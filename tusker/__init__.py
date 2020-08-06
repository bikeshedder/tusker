from contextlib import contextmanager
import argparse
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

    def diff(self, source, target):
        self.log('Creating databases...')
        with self.mgr(source) as source, self.mgr(target) as target:
            self.log(f'Diffing...')
            migration = migra.Migration(source, target)
            migration.set_safety(False)
            migration.add_all_changes()
            print(migration.sql, end='')

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
    tusker.diff(source, target)


def cmd_clean(args, cfg: Config):
    tusker = Tusker(cfg, args.verbose)
    tusker.clean()


def main():
    parser = argparse.ArgumentParser(
        description='Generate a database migration.')
    parser.add_argument(
        '--verbose',
        help='enable verbose output',
        action='store_true',
        default=False)
    parser.add_argument(
        '--config',
        help='the configuration file. Default: tusker.toml',
        default='tusker.toml')
    subparsers = parser.add_subparsers(
        dest='command',
        required=True)
    parser_diff = subparsers.add_parser(
        'diff',
        help='show differences of target schema and migrations')
    parser_diff.add_argument(
        '--from', '--source',
        help='the actual schema version to compare from. Default: migrations',
        dest='source',
        choices=['migrations', 'schema', 'database'],
        default='migrations')
    parser_diff.add_argument(
        '--to', '--target',
        help='the future schema version to compare to. Default: schema',
        dest='target',
        choices=['migrations', 'schema', 'database'],
        default='schema')
    parser_diff.add_argument(
        '--reverse',
        help='inverts the from/source and to/target parameter',
        action='store_true')
    parser_diff.set_defaults(func=cmd_diff)
    parser_clean = subparsers.add_parser(
        'clean',
        help='clean up left over *_migrations or *_schema tables')
    parser_clean.set_defaults(func=cmd_clean)
    args = parser.parse_args()
    if hasattr(args, 'source') and hasattr(args, 'target') and args.source == args.target:
        parser.error("source and target must not be identical")
    cfg = Config(args.config)
    args.func(args, cfg)
