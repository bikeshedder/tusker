# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `migrator` module: the runner as a library that returns data instead of
  printing – `status`, `check`, `log`, `run` and `fix`. Every function takes a
  connected `tokio_postgres::Client`, so the caller chooses TLS and may use a
  pooled connection. The returned futures are `Send`.
- `MigrationState`, `MigrationStatus`, `AppliedMigration` and
  `MigrationLogEntry` are public.
- `Migration::hash()` and `Migration::no_transaction()`.
- A migration whose first line is `-- tusker:no-transaction` runs outside a
  transaction, e.g. for `CREATE INDEX CONCURRENTLY`.
- `migration run --number N` applies outstanding migrations only up to and
  including `N` (`RunOptions::up_to`).
- `Error::NotClean` with the migrations that are out of sync.

### Changed

- **Breaking:** each migration is applied in one transaction together with its
  log entry. A failing migration leaves neither its changes nor a log entry
  behind. Migrations must not contain their own `BEGIN`/`COMMIT`.
- `run` and `fix` hold a session-level advisory lock (`migrator::LOCK_KEY`), so
  concurrent runners wait for each other and apply each migration once.
- **Breaking:** `run` refuses to apply anything while a migration is
  mismatched or its file is missing, instead of stopping on the first one it
  reaches.
- **Breaking:** `check` returns `Error::NotClean` instead of calling
  `std::process::exit(1)`; the `tusker` binary still exits with status 1.
- Errors in a migration's SQL point at the failing line.
- `RunArgs` implements `Default` and its `number` field is public.
- `cli::Subcommands`, `cli::FixArgs` and `Command::subcommand` are public, so
  an embedding CLI can flatten the subcommands into an enum of its own and add
  commands next to them.

### Fixed

- `fix` on a mismatched migration failed with an exclusion constraint
  violation: the log update could insert the new entry before closing the old
  one.

## [0.2.1] - 2026-10-02

### Changed

- `migration log` now prints timestamps as `YYYY-MM-DD HH:MM:SS +HH:MM`
  fixing the occasional misalignments.
- Update tusker-query to 0.4

## [0.2.0] - 2026-07-06

### Added

- `MigrationSource` trait decoupling migration loading from the runner, with a
  filesystem `GlobSource` and an embedded `RustEmbedSource` (behind the new opt-in
  `rust-embed` feature).

### Changed

- **Breaking:** `cli::cmd` and `cli::run` now take a `&dyn MigrationSource`
  argument; the source is chosen in code rather than via CLI flags.
- **Breaking:** removed the `--migrations-dir` command-line option. Embedding
  applications supply a `MigrationSource`; the `tusker` CLI now loads migrations
  from the `migrations.filename` glob in its config.
- **Breaking:** renamed the public migration types: The source-loaded item is
  now `Migration` (was `MigrationFile`), the database row is `AppliedMigration`
  (was `DbMigration`), and the reconciled pair is `MigrationState`.

## [0.1.1] - 2026-05-23

### Changed

- Improve crate metadata with repository, keywords, and categories

## [0.1.0] - 2026-05-22

### Added

- Initial release

[unreleased]: https://github.com/bikeshedder/tusker/compare/tusker-migration-v0.2.1...HEAD
[0.2.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-migration-v0.2.1
[0.2.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-migration-v0.2.0
[0.1.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-migration-v0.1.1
[0.1.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-migration-v0.1.0
