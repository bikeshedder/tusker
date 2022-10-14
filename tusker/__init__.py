import argparse
from contextlib import contextmanager, ExitStack
from glob import glob
import sys
import time
import warnings

import migra
import psycopg2
from psycopg2 import sql
import sqlalchemy

from .config import Config

TUSKER_COMMENT = (
    'CREATED BY TUSKER - If this table is left behind tusker probably '
    'crashed and was not able to clean up after itself. Either try '
    'running `tusker clean` or remove this database manually.'
)


try:
    import importlib.metadata as importlib_metadata
except ModuleNotFoundError:
    import importlib_metadata
try:
    __version__ = importlib_metadata.version(__name__)
except:
    __version__ = 'unknown'


class ExecuteSqlError(Exception):
    pass


def execute_sql_file(cursor, filename):
    with open(filename) as fh:
        sql = fh.read()
    sql = sql.strip()
    if not sql:
        return
    try:
        cursor.exec_driver_sql(sql.replace('%', '%%'))
    except sqlalchemy.exc.SQLAlchemyError as e:
        # https://github.com/sqlalchemy/sqlalchemy/blob/9e7c068d669b209713da62da5748579f92d98129/lib/sqlalchemy/exc.py#L699-L709
        # To provide more detail on the underlying error, but without printing the original SQL.
        if e.orig:
            orig = e.orig
            error_text = "(%s.%s) %s" % (orig.__class__.__module__, orig.__class__.__name__, str(orig))
        else:
            error_text = str(e)
        raise ExecuteSqlError('Error executing SQL file {}: {}'.format(filename, error_text))


class Tusker:

    def __init__(self, config: Config, verbose=False):
        self.config = config
        self.verbose = verbose
        self.conn = self._connect('template1')
        self.conn.autocommit = True

    def _connect(self, name):
        args = self.config.database.args(dbname='template1')
        return psycopg2.connect(**args)

    def log(self, text):
        if self.verbose:
            print(text, file=sys.stderr)

    @contextmanager
    def createengine(self, dbname=None):
        override = {'dbname': dbname} if dbname else {}
        engine = sqlalchemy.create_engine(
            'postgresql://',
            connect_args=self.config.database.args(**override)
        )
        try:
            yield engine
        finally:
            engine.dispose()

    @contextmanager
    def createdb(self, suffix):
        cursor = self.conn.cursor()
        now = int(time.time())
        dbname = '{}_{}_{}'.format(
            self.config.database.args()['dbname'],
            now,
            suffix
        )
        cursor.execute(sql.SQL('CREATE DATABASE {}').format(
            sql.Identifier(dbname)
        ))
        cursor.execute(sql.SQL('COMMENT ON DATABASE {} IS {}').format(
            sql.Identifier(dbname),
            sql.Literal(TUSKER_COMMENT)
        ))
        try:
            with self.createengine(dbname) as engine:
                yield engine
        finally:
            cursor.execute(sql.SQL('DROP DATABASE {}').format(
                sql.Identifier(dbname)
            ))

    @contextmanager
    def mgr_schema(self):
        with self.createdb('schema') as schema_engine:
            with schema_engine.begin() as schema_cursor:
                self.log('Creating original schema...')
                for filename in self._get_schema_files():
                    self.log('- {}'.format(filename))
                    execute_sql_file(schema_cursor, filename)
            yield schema_engine

    @contextmanager
    def mgr_migrations(self):
        with self.createdb('migrations') as migrations_engine:
            with migrations_engine.begin() as migrations_cursor:
                self.log('Creating migrated schema...')
                for filename in self._get_migration_files():
                    self.log('- {}'.format(filename))
                    execute_sql_file(migrations_cursor, filename)
            yield migrations_engine

    @contextmanager
    def mgr_database(self):
        with self.createengine() as database_engine:
            with database_engine.begin() as database_cursor:
                self.log('Observing database schema...')
            yield database_engine

    def mgr(self, name):
        return getattr(self, 'mgr_{}'.format(name))()

    def diff(self, source, target):
        self.log('Creating databases...')
        with self.mgr(source) as source, self.mgr(target) as target:
            self.log('Diffing...')
            migration = migra.Migration(
                source,
                target,
                self.config.database.schema,
            )
            migration.set_safety(self.config.migra.safe)
            migration.add_all_changes(privileges=self.config.migra.privileges)
            return migration.sql

    def check(self, backends):
        with ExitStack() as stack:
            managers = [(name, stack.enter_context(self.mgr(name)))
                        for name in backends]
            for i in range(len(managers)-1):
                source, target = (managers[i], managers[i+1])
                self.log('Diffing {} against {}...'.format(
                    source[0],
                    target[0]
                ))
                migration = migra.Migration(
                    source[1],
                    target[1],
                    schema=self.config.database.schema
                )
                migration.set_safety(self.config.migra.safe)
                migration.add_all_changes(privileges=self.config.migra.privileges)
                if migration.sql:
                    return (source[0], target[0])
        return None

    def clean(self):
        cursor = self.conn.cursor()
        try:
            cursor.execute('''
                SELECT db.datname
                FROM pg_database db
                JOIN pg_shdescription dsc ON dsc.objoid = db.oid
                WHERE dsc.description = %s;
            ''', (TUSKER_COMMENT,))
            rows = cursor.fetchall()
            for row in rows:
                dbname = row[0]
                self.log('Dropping {} ...'.format(dbname))
                cursor.execute(sql.SQL('DROP DATABASE {}').format(
                    sql.Identifier(dbname)
                ))
        finally:
            cursor.close()

    def _get_schema_files(self):
        for pattern in self.config.schema.filename:
            yield from sorted(glob(pattern, recursive=True))


    def _get_migration_files(self):
        for pattern in self.config.migrations.filename:
            yield from sorted(glob(pattern, recursive=True))

def cmd_diff(args, cfg: Config):
    tusker = Tusker(cfg, args.verbose)
    source = args.source
    target = args.target
    if args.reverse:
        source, target = target, source
    try:
        sql = tusker.diff(source, target)
        print(sql, end='')
    except ExecuteSqlError as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)


def cmd_check(args, cfg: Config):
    backends = args.backends
    if 'all' in backends:
        backends = ['migrations', 'schema', 'database']
    tusker = Tusker(cfg, args.verbose)
    try:
        diff = tusker.check(backends)
    except ExecuteSqlError as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)
    if diff:
        print('Schemas differ: {} != {}'.format(diff[0], diff[1]))
        print('Run `tusker diff` to see the differences')
        sys.exit(1)
    else:
        print('Schemas are identical')
        sys.exit(0)


def cmd_clean(args, cfg: Config):
    tusker = Tusker(cfg, args.verbose)
    tusker.clean()


BACKEND_CHOICES = ['migrations', 'schema', 'database']


class ValidateBackends(argparse.Action):
    def __call__(self, parser, args, values, option_string=None):
        if 'all' in values:
            values = BACKEND_CHOICES
        else:
            if len(values) <= 1:
                choices = ', '.join(map(repr, BACKEND_CHOICES))
                raise argparse.ArgumentError(
                    self,
                    (
                        'at least two backends are required to perform '
                        'the check (choose from {choices}) or pass \'all\' '
                        'on its own.'
                    )
                )
            for value in values:
                if value not in BACKEND_CHOICES:
                    choices = ', '.join(map(repr, BACKEND_CHOICES + ['all']))
                    msg = 'invalid choice: {!r} (choose from {})'.format(
                        value,
                        choices
                    )
                    raise argparse.ArgumentError(self, msg)
        setattr(args, self.dest, values)


def add_migra_args(parser):
    g = parser.add_mutually_exclusive_group()
    g.add_argument(
        '--safe',
        help='throw an exception if drop-statements are generated.',
        action='store_const',
        dest='safe',
        const=True,
    )
    g.add_argument(
        '--unsafe',
        help='don\'t throw an exception if drop-statements are generated.',
        action='store_const',
        dest='safe',
        const=False,
    )
    g = parser.add_mutually_exclusive_group()
    g.add_argument(
        '--with-privileges',
        help='output privilege differences (ie. grant/revoke statements).',
        action='store_const',
        dest='privileges',
        const=True,
    )
    g.add_argument(
        '--without-privileges',
        help='don\'t output privilege differences.',
        action='store_const',
        dest='privileges',
        const=False,
    )


def main():
    if not sys.warnoptions:
        warnings.simplefilter("default")
    parser = argparse.ArgumentParser(
        description='Generate a database migration.')
    parser.add_argument(
        '--version',
        action='version',
        version='%(prog)s {}'.format(__version__))
    parser.add_argument(
        '--verbose',
        help='enable verbose output',
        action='store_true',
        default=False)
    parser.add_argument(
        '--config', '-c',
        help='the configuration file. Default: tusker.toml',
        default='tusker.toml')
    subparsers = parser.add_subparsers(
        dest='command',
        required=True)
    parser_diff = subparsers.add_parser(
        'diff',
        help='show differences between two schemas',
        description='''
            This command calculates the difference between two database schemas.
            The from- and to-parameter accept one of the following backends:
            migrations, schema, database
        ''')
    parser_diff.add_argument(
        'source',
        metavar='from',
        nargs='?',
        help='from-backend for the diff operation. Default: migrations',
        choices=BACKEND_CHOICES,
        default='migrations')
    parser_diff.add_argument(
        'target',
        metavar='to',
        nargs='?',
        help='to-backend for the diff operation. Default: schema',
        choices=BACKEND_CHOICES,
        default='schema')
    parser_diff.add_argument(
        '--reverse', '-r',
        help='swaps the "from" and "to" arguments creating a reverse diff',
        action='store_true')
    parser_diff.add_argument(
        '--create-extensions-only',
        help='Only output create extension statements, nothing else. ',
        action='store_true',
    )
    add_migra_args(parser_diff)
    parser_diff.set_defaults(func=cmd_diff)
    parser_check = subparsers.add_parser(
        'check',
        help='check for differences between schemas',
        description='''
            This command checks for differences between two or more schemas.
            Exit code 0 means that the schemas are all in sync. Otherwise the
            exit code 1 is used. This is useful for continuous integration checks.
        ''')
    parser_check.set_defaults(func=cmd_check)
    parser_check.add_argument(
        'backends',
        help=(
            'at least two backends are required to diff against each other '
            '(choose from {}). You can also pass \'all\' on its own to diff '
            'all backends against each other.'
        ).format(
            ', '.join(map(repr, BACKEND_CHOICES))
        ),
        metavar='backend',
        nargs='*',
        default=['migrations', 'schema'],
        action=ValidateBackends
    )
    add_migra_args(parser_check)
    parser_clean = subparsers.add_parser(
        'clean',
        help='clean up left over *_migrations or *_schema tables')
    parser_clean.set_defaults(func=cmd_clean)
    args = parser.parse_args()
    if hasattr(args, 'from') and hasattr(args, 'target') and args.source == args.target:
        parser.error('source and target must not be identical')
    cfg = Config(args.config)
    if getattr(args, 'safe') is not None:
        cfg.migra.safe = args.safe
    if getattr(args, 'privileges') is not None:
        cfg.migra.privileges = args.privileges
    args.func(args, cfg)
