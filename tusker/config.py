import os

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

    def __str__(self):
        return f'Config(schema={self.schema}, migrations={self.migrations}, database={self.database})'


class ConfigReader:

    def __init__(self, data):
        self.data = data

    def get(self, name, type, required=False):
        if name not in self.data:
            if required:
                raise ConfigError.missing(f'database.{name}')
            else:
                return None
        value = self.data[name]
        if not isinstance(value, type):
            raise ConfigError.invalid(name, f"Not of type {type}")
        return value


class SchemaConfig:

    def __init__(self, data):
        data = ConfigReader(data)
        self.filename = data.get('filename', str) or 'schema.sql'

    def __str__(self):
        return f'SchemaConfig({self.__dict__!r})'


class MigrationsConfig:

    def __init__(self, data):
        data = ConfigReader(data)
        self.directory = data.get('directory', str, True) or 'migrations'

    def __str__(self):
        return f'MigrationsConfig({self.__dict__!r})'


class DatabaseConfig:

    def __init__(self, data):
        data = ConfigReader(data)
        self.url = data.get('url', str)
        self.host = data.get('host', str)
        self.port = data.get('port', int)
        self.dbname = data.get('dbname', str) or 'tusker'
        self.user = data.get('user', str)
        self.password = data.get('password', str)
        self.schema = data.get('schema', str)

    def __str__(self):
        return f'DatabaseConfig({self.__dict__!r})'

    def args(self, **override):
        args = {
            'host': self.host,
            'port': self.port,
            'user': self.user,
            'password': self.password,
            'dbname': self.dbname,
        }
        args.update(override)
        return args


class ConfigError(RuntimeError):

    @classmethod
    def missing(cls, name):
        return cls(f'Missing configuration: {name}')

    @classmethod
    def invalid(cls, name, reason):
        return cls(f'Invalid configuration: {name}, {reason}')
