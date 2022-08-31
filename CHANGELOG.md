# Change Log

## v0.4.8

* Fix "`ypeError: dict is not a sequence" error when
  the schema or migration files contain percent characters (`%`).

## v0.4.7

* Fix "A value is required for bind parameter ..." error caused
  by SQL files containing code looking like SQLAlchemy parameters
  (`:<params>`).

## v0.4.6 [YANKED]

## v0.4.5

* Add support for `**` in glob pattern
* Improve output of SQL errors

## v0.4.4

* Add default config for `migra` config section

## v0.4.3

* Fix `privileges` configuration option

## v0.4.2

* Add `migra.safe` and `migra.permission` to `tusker.toml`
* Add `--safe` and `--unsafe` arguments
* Add `--without-privileges` argument
* Update `tomlkit` to version `0.10`
* Update locked dependency versions

## v0.4.1

* Do not filter by `.sql` extension when using the `migrations.filename`
  setting.

## v0.4.0

* Add `migrations.filename` setting which supports a `glob` pattern
* Fix error messages for invalid configurations
* Increase minimum `python` version to `3.6`
* Update `migra` to version `3.0`
* Update `tomlkit` to version `0.7`
* Update `sqlalchemy` to version `1.4`
* Update `psycopg2` to version `2.9`

## v0.3.4

* Fix quoting of database names

## v0.3.3

* Add support for mixing url with other database settings

## v0.3.2

* Fix transaction handling

## v0.3.1

* Execute files specified by `glob` pattern in sorted order

## v0.3.0

* Add `--version` argument
* Add `glob` pattern support for `schema.filename` setting

## v0.2.3

* Replace f-Strings by .format() calls. This fixes Python 3.5 support.

## v0.2.2

* Add support for `database.schema` config option

## v0.2.1

* Add `--with-privileges` option to `diff` and `check` commands.

## v0.2.0

* Add `from` and `to` argument to `diff` command which makes it possible
  to compare a schema file, migration files and an existing database.
* Add `--reverse` option to `diff` command.
* Add `check` command

## v0.1.2

* Fix closing of DB connections

## v0.1.1

* Escape schema and migration SQL before execution

## v0.1.0

* First release
