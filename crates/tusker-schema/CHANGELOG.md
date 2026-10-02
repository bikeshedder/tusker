# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-10-02

### Changed

- Update tusker-query to 0.4

## [0.2.0] - 2026-07-06

### Added

- Add `RemovedEnumValue` and `DiffOptions` to configure how the removal of an
  enum value is handled. `DiffSql::sql` now takes a `&DiffOptions` argument, and
  `Inspection::equivalent` performs the same options-aware comparison that
  `tusker check` relies on.

### Fixed

- Reconcile drifted `NOT NULL` constraint names (PostgreSQL 18+). A column
  renamed with `ALTER TABLE ... RENAME COLUMN` keeps the constraint name derived
  from its former name, causing `tusker diff` to report no changes while
  `pg_dump` still showed a difference. The diff now emits a `RENAME CONSTRAINT`
  statement to reconcile the two.

## [0.1.1] - 2026-05-23

### Changed

- Improve crate metadata with repository, keywords, and categories

## [0.1.0] - 2026-05-22

### Added

- Initial release

[unreleased]: https://github.com/bikeshedder/tusker/compare/tusker-schema-v0.2.1...HEAD
[0.2.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-schema-v0.2.1
[0.2.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-schema-v0.2.0
[0.1.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-schema-v0.1.1
[0.1.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-schema-v0.1.0
