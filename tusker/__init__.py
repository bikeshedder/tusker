import argparse
from contextlib import contextmanager, ExitStack
import datetime
import os
import re
import string
import sys
import time

import migra
import psycopg2
import sqlbag
import schemainspect
import sqlalchemy

from .config import Config

TUSKER_COMMENT = 'CREATED BY TUSKER - If this table is left behind tusker probably crashed and was not able to clean up after itself. Either try running `tusker clean` or remove this database manually.'


class Tusker:

    def __init__(self, config: Config, verbose=False):
        self.config = config
        self.verbose = verbose
        self.conn = self._connect('template1')
        self.conn.autocommit = True

    def _connect(self, name):
        if self.config.database.url:
            return psycopg2.connect(self.config.database.url, dbname='template1')
        else:
            return psycopg2.connect(**self.config.database.args(dbname='template1'))

    def log(self, text):
        if self.verbose:
            print(text, file=sys.stderr)

    @contextmanager
    def createengine(self, dbname=None):
        if self.config.database.url:
            engine = sqlalchemy.create_engine(
                self.config.database.url,
                connect_args={'dbname': dbname} if dbname else {},
            )
        else:
            engine = sqlalchemy.create_engine(
                'postgresql://',
                connect_args=self.config.database.args(dbname=dbname)
            )
        try:
            yield engine
        finally:
            engine.dispose()

    @contextmanager
    def createdb(self, suffix):
        cursor = self.conn.cursor()
        now = int(time.time())
        dbname = f'{self.config.database.dbname}_{now}_{suffix}'
        cursor.execute(f'CREATE DATABASE "{dbname}"')
        cursor.execute(f'COMMENT ON DATABASE "{dbname}" IS \'{TUSKER_COMMENT}\'')
        try:
            with self.createengine(dbname) as engine:
                yield engine
        finally:
            cursor.execute(f'DROP DATABASE {dbname}')

    @contextmanager
    def mgr_schema(self):
        with self.createdb('schema') as schema_engine:
            with schema_engine.connect() as schema_cursor:
                self.log('Creating original schema...')
                with open(self.config.schema.filename) as fh:
                    sql = fh.read()
                    sql = sql.strip()
                    if sql:
                        sql = sqlalchemy.text(sql)
                        schema_cursor.execute(sql)
            yield schema_engine

    @contextmanager
    def mgr_migrations(self):
        with self.createdb('migrations') as migrations_engine:
            with migrations_engine.connect() as migrations_cursor:
                self.log('Creating migrated schema...')
                for filename in sorted(os.listdir(self.config.migrations.directory)):
                    if not filename.endswith('.sql'):
                        continue
                    self.log(f"- {filename}")
                    filename = os.path.join(self.config.migrations.directory, filename)
                    with open(filename) as fh:
                        sql = fh.read()
                        sql = sql.strip()
                        if sql:
                            sql = sqlalchemy.text(sql)
                            migrations_cursor.execute(sql)
            yield migrations_engine

    @contextmanager
    def mgr_database(self):
        with self.createengine(self.config.database.dbname) as database_engine:
            with database_engine.connect() as database_cursor:
                self.log('Observing database schema...')
            yield database_engine

    def mgr(self, name):
        return getattr(self, f'mgr_{name}')()

    def diff(self, source, target, with_privileges=False):
        self.log('Creating databases...')
        with self.mgr(source) as source, self.mgr(target) as target:
            self.log(f'Diffing...')
            migration = migra.Migration(source, target, self.config.database.schema)
            migration.set_safety(False)
            migration.add_all_changes(privileges=with_privileges)
            return migration.sql

    def check(self, backends, with_privileges=False):
        diff_found = True
        with ExitStack() as stack:
            managers = [(name, stack.enter_context(self.mgr(name))) for name in backends]
            for i in range(len(managers)-1):
                source, target = (managers[i], managers[i+1])
                self.log(f'Diffing {source[0]} against {target[0]}...')
                migration = migra.Migration(source[1], target[1], schema=self.config.database.schema)
                migration.set_safety(False)
                migration.add_all_changes(privileges=with_privileges)
                if migration.sql:
                    return (source[0], target[0])
                    diff_found = False
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
                if '"' in dbname:
                    raise RuntimeError('Database with an " in its name found. Please fix that manually.')
                self.log(f'Dropping {dbname} ...')
                cursor.execute(f'DROP DATABASE "{dbname}"')
        finally:
            cursor.close()


def cmd_diff(args, cfg: Config):
    tusker = Tusker(cfg, args.verbose)
    source = args.source
    target = args.target
    if args.reverse:
        source, target = target, source
    sql = tusker.diff(source, target, with_privileges=args.with_privileges)
    print(sql, end='')


def cmd_check(args, cfg: Config):
    backends = args.backends
    if 'all' in backends:
        backends = ['migrations', 'schema', 'database']
    tusker = Tusker(cfg, args.verbose)
    diff = tusker.check(backends, with_privileges=args.with_privileges)
    if diff:
        print(f'Schemas differ: {diff[0]} != {diff[1]}')
        print(f'Run `tusker diff` to see the differences')
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
                raise argparse.ArgumentError(self, f'at least two backends are required to perform the check (choose from {choices}) or pass \'all\' on its own.')
            for value in values:
                if value not in BACKEND_CHOICES:
                    choices = ', '.join(map(repr, BACKEND_CHOICES + ['all']))
                    msg = f'invalid choice: {value!r} (choose from {choices})'
                    raise argparse.ArgumentError(self, msg)
        setattr(args, self.dest, values)


def main():
    parser = argparse.ArgumentParser(
        description='Generate a database migration.')
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
        nargs="?",
        help=f'from-backend for the diff operation. Default: migrations',
        choices=BACKEND_CHOICES,
        default='migrations')
    parser_diff.add_argument(
        'target',
        metavar='to',
        nargs="?",
        help=f'to-backend for the diff operation. Default: schema',
        choices=BACKEND_CHOICES,
        default='schema')
    parser_diff.add_argument(
        '--reverse', '-r',
        help='swaps the "from" and "to" arguments creating a reverse diff',
        action='store_true')
    parser_diff.add_argument(
        '--with-privileges',
        help='also output privilege differences (ie. grant/revoke statements)',
        action='store_true')
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
        help='at least two backends are required to diff against each other (choose from {}). You can also pass \'all\' on its own to diff all backends against each other.'.format(', '.join(map(repr, BACKEND_CHOICES))),
        metavar='backend',
        nargs='*',
        default=['migrations', 'schema'],
        action=ValidateBackends
    )
    parser_check.add_argument(
        '--with-privileges',
        help='also output privilege differences (ie. grant/revoke statements)',
        action='store_true')
    parser_clean = subparsers.add_parser(
        'clean',
        help='clean up left over *_migrations or *_schema tables')
    parser_clean.set_defaults(func=cmd_clean)
    args = parser.parse_args()
    if hasattr(args, 'from') and hasattr(args, 'target') and args.source == args.target:
        parser.error("source and target must not be identical")
    cfg = Config(args.config)
    args.func(args, cfg)
