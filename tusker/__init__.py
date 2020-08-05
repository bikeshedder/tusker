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


class DatabaseAdmin:

    def __init__(self, cfg: Config):
        self.config = cfg.database
        self.conn = self._connect('template1')
        self.conn.autocommit = True

    def _connect(self, name):
        if self.config.url:
            return psycopg2.connect(self.config.url, dbname='template1')
        else:
            return psycopg2.connect(**self.config.args(dbname='template1'))

    @contextmanager
    def createengine(self, dbname=None):
        if self.config.url:
            engine = sqlalchemy.create_engine(
                self.config.url,
                connect_args={'dbname': dbname} if dbname else {},
            )
        else:
            engine = sqlalchemy.create_engine(
                'postgresql://',
                connect_args=self.config.args(dbname=dbname)
            )
        try:
            yield engine
        finally:
            engine.dispose()

    @contextmanager
    def createdb(self, suffix):
        cursor = self.conn.cursor()
        now = int(time.time())
        dbname = f'{self.config.dbname}_{now}_{suffix}'
        cursor.execute(f'CREATE DATABASE "{dbname}"')
        cursor.execute(f'COMMENT ON DATABASE "{dbname}" IS \'{TUSKER_COMMENT}\'')
        try:
            with self.createengine(dbname) as engine:
                yield engine
        finally:
            cursor.execute(f'DROP DATABASE {dbname}')

def cmd_diff(args, cfg: Config):
    dba = DatabaseAdmin(cfg)
    if args.verbose:
        print('Creating databases...', file=sys.stderr)
    with dba.createdb('schema') as schema_engine, dba.createdb('migrations') as migrations_engine, dba.createengine(cfg.database.dbname) as database_engine:
        if 'schema' in [args.source, args.target]:
            with schema_engine.connect() as schema_cursor:
                if args.verbose:
                    print('Creating original schema...', file=sys.stderr)
                #with schema_engine as schema_cursor:
                with open(cfg.schema.filename) as fh:
                    sql = fh.read()
                    sql = sql.strip()
                    if sql:
                        sql = sqlalchemy.text(sql)
                        schema_cursor.execute(sql)
        if 'migrations' in [args.source, args.target]:
            with migrations_engine.connect() as migrations_cursor:
                if args.verbose:
                    print('Creating migrated schema...', file=sys.stderr)
                for filename in sorted(os.listdir(cfg.migrations.directory)):
                    if not filename.endswith('.sql'):
                        continue
                    if args.verbose:
                        print(f"- {filename}", file=sys.stderr)
                    filename = os.path.join(cfg.migrations.directory, filename)
                    with open(filename) as fh:
                        sql = fh.read()
                        sql = sql.strip()
                        if sql:
                            sql = sqlalchemy.text(sql)
                            migrations_cursor.execute(sql)
        if 'database' in [args.source, args.target]:
            with database_engine.connect() as database_cursor:
                if args.verbose:
                    print('Observing database schema...', file=sys.stderr)
        if args.verbose:
            print('Selecting source and target', file=sys.stderr)
        source = (migrations_engine if args.source == 'migrations'
                 else schema_engine if args.source == 'schema'
                 else database_engine if args.source == 'database'
                 else migrations_engine)
        target = (migrations_engine if args.target == 'migrations'
                 else schema_engine if args.target == 'schema'
                 else database_engine if args.target == 'database'
                 else schema_engine)
        if args.reverse:
            source, target = target, source
        if args.verbose:
            print(f'Diffing...', file=sys.stderr)
        migration = migra.Migration(source, target)
        migration.set_safety(False)
        migration.add_all_changes()
        print(migration.sql, end='')


def cmd_clean(args, cfg: Config):
    dba = DatabaseAdmin(cfg)
    cursor = dba.conn.cursor()
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
            if args.verbose:
                print(f'Dropping {dbname} ...', file=sys.stderr)
            cursor.execute(f'DROP DATABASE "{dbname}"')
    finally:
        cursor.close()



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
        help='clean up left ofter *_migrations or *_schema tables')
    parser_clean.set_defaults(func=cmd_clean)
    args = parser.parse_args()
    cfg = Config(args.config)
    args.func(args, cfg)
