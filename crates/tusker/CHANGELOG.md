# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `tusker query sync` and `tusker query inspect` record PostgreSQL enum types including their labels

## [0.7.0] - 2026-07-06

### Added

- Add support for array and composite parameter and result column types in `tusker query` metadata.
- Add the `[diff] removed_enum_value` configuration option (`"unsafe"` — the
  default and previous behavior, `"warn"`, or `"ignore"`) controlling how the
  removal of an enum value is handled by `tusker diff` and `tusker check`.

### Changed

- **Breaking:** the `tusker migration` and `tusker migrate` commands no longer
  accept `--migrations-dir`. Migrations are loaded from the `migrations.filename`
  glob in the configuration file (default `db/migrations/**/*.sql`).
- `tusker check` is now defined as an empty `tusker diff` and defaults to the
  same direction (migrations → schema), so the two commands can never disagree.

### Fixed

- `tusker diff` now reconciles drifted `NOT NULL` constraint names on
  PostgreSQL 18+. A column renamed with `ALTER TABLE ... RENAME COLUMN` kept the
  constraint name derived from its former name, so `tusker diff` reported no
  changes while `pg_dump` still showed a difference; it now emits a
  `RENAME CONSTRAINT` statement to reconcile the two.

## [0.6.1] - 2026-05-23

### Fixed

- Improve the README for the Rust CLI release

### Changed

- Improve crate metadata with repository, keywords, and categories

## [0.6.0] - 2026-05-22

This is the first release of the Rust implementation of Tusker. It starts at
`0.6.0` to avoid overlapping with version numbers used by the archived Python
implementation on [PyPI](https://pypi.org/project/tusker/).

### Added

- Initial release

[unreleased]: https://github.com/bikeshedder/tusker/compare/tusker-v0.7.0...HEAD
[0.7.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-v0.7.0
[0.6.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-v0.6.1
[0.6.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-v0.6.0
