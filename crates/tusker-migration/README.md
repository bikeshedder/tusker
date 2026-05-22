# tusker-migration

`tusker-migration` is a small PostgreSQL migration runner for Rust
applications.

It manages SQL migration files from a directory, keeps a migration history in
the database, and provides the logic needed to inspect, apply, and reconcile
migration state.

The crate is used by `tusker`, but it is also intended to be embedded directly
in applications that want Tusker's migration runner without depending on the
full top-level CLI.

This crate provides:

- a migration status table schema for PostgreSQL
- a SQL-file-based migration loader
- embeddable `clap` command types for status, log, check, run, and fix
- hash-based detection of renamed or modified migration files
- migration runner logic that can be called directly from Rust code

## What it manages

`tusker-migration` expects migration files in a directory such as:

```text
db/migrations/
  0001_initial.sql
  0002_add_users.sql
  0003_fix_indexes.sql
```

Each migration file name is parsed as:

```text
<number>_<name>.sql
```

The crate computes a SHA-512 hash of each file and compares it with the hash
stored in the database migration log.

## Migration table

On first run, `tusker-migration` creates a PostgreSQL table called
`migration`. The schema lives in:

- [db/schema.sql](db/schema.sql)

The table stores:

- migration number
- migration name
- migration file hash
- a validity range
- the operation (`apply`, `fake`, `update`, `delete`)

Instead of mutating rows in place, the migration history is tracked as a log of
entries with time ranges. The current migration state is reconstructed from the
entries whose validity range contains `now()`.

## Commands

The crate exports its own `clap` command types from:

- [src/cli.rs](src/cli.rs)

In particular:

- `tusker_migration::cli::Command`
- `tusker_migration::cli::cmd(...)`
- `tusker_migration::cli::run(...)`

That makes it easy to either:

- reuse the built-in migration CLI shape in your own binary
- call the migration functions directly from application code

### Embedding example

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Migrate(tusker_migration::cli::Command),
}

async fn run(pg_config: &tokio_postgres::Config) -> Result<(), tusker_migration::error::Error> {
    let args = Args::parse();
    match args.command {
        Command::Migrate(command) => tusker_migration::cli::cmd(pg_config, &command).await,
    }
}
```

### Built-in subcommands

### `status`

Lists migration files and compares them with the current database state.

Possible states:

- `Ok`: migration file and database entry match
- `Mismatch`: same migration number exists, but name and/or hash differ
- `New`: migration file exists but has not been applied
- `Migration file missing`: database entry exists but the file is gone

### `log`

Shows the migration history from the database log, including timestamp and
operation.

### `check`

Fails if migration files and database state are not in sync.

This is useful in CI or before applying new migrations.

### `run`

Applies all outstanding migrations in order.

If the migration table does not exist yet, it is created automatically before
running the first migration.

### `fix`

Reconciles migration state for one migration number:

- `Mismatch` -> updates the stored migration hash/name entry
- `New` -> marks the migration as applied without running the SQL (`fake`)
- `Migration file missing` -> removes the current migration entry

This is a repair tool and should be used carefully.

## Using It From Your App

In an embedded setup, the usual flow is:

1. expose `tusker_migration::cli::Command` from your own binary
2. pass your application's PostgreSQL config into `tusker_migration::cli::cmd`
3. let the crate handle migration status, running, checking, or repair

The migration files themselves still live in a normal directory such as
`db/migrations/`, and the built-in command types already expose the
`--migrations-dir` option when you need to override that location.

## How it works

At a high level:

1. Read migration files from a directory
2. Parse the migration number and name from the file name
3. Hash the SQL file contents
4. Load the current migration state from PostgreSQL
5. Compare filesystem and database state
6. Apply or repair as requested

The comparison logic is implemented in:

- [src/models.rs](src/models.rs)

The database access and migration log operations live in:

- [src/db.rs](src/db.rs)

The file loading and hashing logic lives in:

- [src/file.rs](src/file.rs)

## PostgreSQL only

This crate is PostgreSQL-specific.

It depends on:

- `tokio-postgres` for database access
- PostgreSQL range types for migration validity tracking
- PostgreSQL SQL syntax and system behavior

## Limitations

Current behavior is intentionally small and conservative:

- only `.sql` files are considered migration files
- duplicate migration numbers are rejected
- `run --number` is parsed but not fully implemented yet
- `fix` is operational but still fairly blunt as a repair tool
- this crate is focused on running and reconciling migrations, not generating
  them

## Relationship to the main `tusker` crate

`tusker-migration` is the migration runner behind the `tusker migration` and
`tusker migrate` commands, but that is not its only purpose.

If you are looking for schema diffing or migration generation, that lives in
the main `tusker` crate and the schema inspection/diff crates around it.

This crate is the execution and bookkeeping layer for already-written SQL
migration files, whether you use it through `tusker` or embed it directly in
your own program.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
