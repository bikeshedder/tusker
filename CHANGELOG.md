# Change Log

## v0.4.0 (unreleased)

* Update `migra` to version `3.0`
* Update `tomlkit` to version `0.7`
* Update `sqlalchemy` to version `1.4`
* Add `migrations.filename` setting which supports a `glob` pattern

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
