import os
import re

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
        data.setdefault('schema', {'filename': ['schema.sql']})
        data.setdefault('migrations', {'filename': ['migrations/*.sql']})
        data.setdefault('migra', {'safe': False, 'privileges': False})
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


def replace_from_env_var(matchobj):
    env_variable = matchobj.group(1)
    try:
        return os.environ[env_variable]
    except KeyError:
        raise ConfigError.missing_env(env_variable)


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

        if isinstance(value, str):
            # Replace any environment variables
            value = re.sub(r"\${([a-zA-Z_][a-zA-Z_0-9]*)}", replace_from_env_var, value)

        if not isinstance(value, type):
            raise ConfigError.invalid(name, 'Not of type {}'.format(type))
        return value

    def get_list(self, name, required=False, default=None):
        value = self.get(name, (str, list), required, default)
        if isinstance(value, str):
            value = [value]
        else:
            if value and not all(isinstance(x, str) for x in value):
                raise ConfigError.invalid(name, 'Not a list of strings {}'.format(value))
        return value


class SchemaConfig:

    def __init__(self, data):
        data = ConfigReader(data, 'schema')
        self.filename = data.get_list('filename', default=['schema.sql'])

    def __str__(self):
        return 'SchemaConfig({!r})'.format(self.__dict__)


class MigrationsConfig:

    def __init__(self, data):
        data = ConfigReader(data, 'migrations')
        directory = data.get('directory', str, False)
        if directory:
            import warnings
            warnings.warn(
                'The "migrations.directory" configuration option is '
                'deprecated and support for this option will be removed '
                'in the next version of tusker. Please replace this by '
                'the "migrations.filename" option which does support '
                'globbing patterns.',
                DeprecationWarning,
                stacklevel=2
            )
            filename = data.get_list('filename')
            if filename:
                raise ConfigError.invalid(
                    'migrations directory and filename parameters '
                    'are mutually exclusive',
                )
            else:
                self.filename = ['{}/*.sql'.format(directory)]
        else:
            self.filename = data.get_list('filename', default=['migrations/*.sql'])

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
    def missing_env(cls, env_variable):
        return cls('Missing environment variable: {}'.format(env_variable))

    @classmethod
    def missing(cls, name):
        return cls('Missing configuration: {}'.format(name))

    @classmethod
    def invalid(cls, name, reason):
        return cls('Invalid configuration: {}, {}'.format(name, reason))
