# Change Log

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
