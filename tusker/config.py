import os

from psycopg2.extensions import parse_dsn
from tomlkit.toml_file import TOMLFile


class Config:

    def __init__(self, filename=None):
        env = os.environ
        filename = filename or 'tusker.toml'
        toml = TOMLFile(filename)
        try:
            data = toml.read()
        except FileNotFoundError:
            data = {}
        # time to validate some configuration variables
        data.setdefault('database', {'dbname': 'tusker'})
        data.setdefault('schema', {'filename': 'schema.sql'})
        data.setdefault('migrations', {'directory': 'migrations'})
        self.schema = SchemaConfig(data['schema'])
        self.migrations = MigrationsConfig(data['migrations'])
        self.database = DatabaseConfig(data['database'])
        self.migra = MigraConfig(data['migra'])

    def __str__(self):
        return 'Config(schema={}, migrations={}, database={}, migra={})'.format(
            self.schema,
            self.migrations,
            self.database,
            self.migra
        )


class ConfigReader:

    def __init__(self, data, path):
        self.data = data
        self.path = path

    def get(self, name, type, required=False, default=None):
        if name not in self.data:
            if required:
                raise ConfigError.missing('{}.{}'.format(self.path, name))
            else:
                return default
        value = self.data[name]
        if not isinstance(value, type):
            raise ConfigError.invalid(name, 'Not of type {}'.format(type))
        return value


class SchemaConfig:

    def __init__(self, data):
        data = ConfigReader(data, 'schema')
        self.filename = data.get('filename', str) or 'schema.sql'

    def __str__(self):
        return 'SchemaConfig({!r})'.format(self.__dict__)


class MigrationsConfig:

    def __init__(self, data):
        data = ConfigReader(data, 'migrations')
        self.directory = data.get('directory', str, False)
        self.filename = data.get('filename', str, False)
        if not self.directory and not self.filename:
            self.directory = 'migrations'
        elif self.directory and self.filename:
            raise ConfigError.invalid(
                'migrations',
                'directory and filename parameters are mutually exclusive',
            )

    def __str__(self):
        return 'MigrationsConfig({!r})'.format(self.__dict__)


class DatabaseConfig:

    def __init__(self, data):
        data = ConfigReader(data, 'database')
        self.url = data.get('url', str)
        self.host = data.get('host', str)
        self.port = data.get('port', int)
        self.dbname = data.get('dbname', str)
        self.user = data.get('user', str)
        self.password = data.get('password', str)
        self.schema = data.get('schema', str)

    def __str__(self):
        return 'DatabaseConfig({!r})'.format(self.__dict__)

    def args(self, **override):
        if self.url:
            args = parse_dsn(self.url)
        else:
            args = {}
        for k in ['host', 'port', 'dbname', 'user', 'password']:
            v = getattr(self, k)
            if v is not None:
                args[k] = v
        if not args['dbname']:
            args['dbname'] = 'tusker'
        args.update(override)
        return args


class MigraConfig:
    def __init__(self, data):
        data = ConfigReader(data, 'migra')
        self.safe = data.get('safe', bool, default=False)
        self.privileges = data.get('privileges', bool, default=False)


class ConfigError(RuntimeError):

    @classmethod
    def missing(cls, name):
        return cls('Missing configuration: {}'.format(name))

    @classmethod
    def invalid(cls, name, reason):
        return cls('Invalid configuration: {}, {}'.format(name, reason))
