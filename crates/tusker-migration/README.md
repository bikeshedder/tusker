# tusker-migration

`tusker-migration` is a small PostgreSQL migration runner for Rust
applications.

It loads SQL migrations from a pluggable source — a directory on disk or files
embedded directly into your binary — keeps a migration history in the database,
and provides the logic needed to inspect, apply, and reconcile migration state.

The crate is used by `tusker`, but it is also intended to be embedded directly
in applications that want Tusker's migration runner without depending on the
full top-level CLI.

This crate provides:

- a migration status table schema for PostgreSQL
- a pluggable `MigrationSource` trait, with a filesystem (`GlobSource`) and an
  embedded (`RustEmbedSource`, via [`rust-embed`](https://docs.rs/rust-embed))
  implementation
- a library API (`migrator`) for status, check, log, run, and fix that returns
  data and works on a connection you provide
- embeddable `clap` command types for status, log, check, run, and fix
- hash-based detection of renamed or modified migration files
- transactional, concurrency-safe application of migrations

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

The crate computes a SHA-512 hash of each migration's SQL and compares it with
the hash stored in the database migration log.

## Migration sources

Where migrations come from is abstracted behind the `MigrationSource` trait.
Two implementations ship with the crate:

- `GlobSource` loads migrations from the filesystem using a glob pattern such as
  `db/migrations/**/*.sql`.
- `RustEmbedSource` loads migrations embedded into the binary via
  [`rust-embed`](https://docs.rs/rust-embed). This requires the `rust-embed`
  feature:

  ```toml
  tusker-migration = { version = "0.3", features = ["rust-embed"] }
  ```

  ```ignore
  use rust_embed::RustEmbed;
  use tusker_migration::RustEmbedSource;

  #[derive(RustEmbed)]
  #[folder = "db/migrations"]
  struct Migrations;

  let source = RustEmbedSource::<Migrations>::new();
  ```

Applications with other needs can implement `MigrationSource` themselves and
construct `Migration` values with `Migration::new`.

## Migration table

On first run, `tusker-migration` creates a PostgreSQL table called
`migration`. The schema lives in:

- [db/schema.sql](db/schema.sql)

It needs the `btree_gist` extension and creates it if missing, so the role
running the first migration must be allowed to, or the extension must exist.

The table stores:

- migration number
- migration name
- migration file hash
- a validity range
- the operation (`apply`, `fake`, `update`, `delete`)

Instead of mutating rows in place, the migration history is tracked as a log of
entries with time ranges. The current migration state is reconstructed from the
entries whose validity range contains `now()`.

## Applying migrations

`run` applies the outstanding migrations in order of their numbers:

- Each migration runs in one transaction together with its log entry. A
  failing migration leaves neither its changes nor a log entry behind, so
  migrations must not contain their own `BEGIN`/`COMMIT`.
- A migration whose first line is `-- tusker:no-transaction` runs outside a
  transaction, for statements PostgreSQL refuses inside one such as
  `CREATE INDEX CONCURRENTLY`. It is logged after it succeeded.
- `run` and `fix` hold a session-level advisory lock
  (`pg_advisory_lock(migrator::LOCK_KEY)`). Several processes migrating the same
  database at once wait for each other and apply each migration exactly once,
  e.g. replicas of an application that migrates at startup.
- Nothing is applied while a migration is mismatched or its file is missing;
  repair those with `fix` first.

## Library API

The `migrator` module returns data and never prints. Every function takes a
connected `tokio_postgres::Client`, so you choose how to connect (with or
without TLS) and may use a pooled connection (`&**client` / `&mut **client`
for a `deadpool_postgres::Client`):

```rust,no_run
use tusker_migration::{migrator, GlobSource, MigrationStatus};

async fn migrate(client: &mut tokio_postgres::Client) -> Result<(), tusker_migration::error::Error> {
    let source = GlobSource::new("db/migrations/**/*.sql");

    // Inspect without changing anything.
    for state in migrator::status(client, &source).await? {
        if state.status() != MigrationStatus::Ok {
            println!("{:04}_{}: {}", state.number, state.name(), state.status());
        }
    }

    // Apply everything outstanding (or `RunOptions::new().up_to(n)`).
    let applied = migrator::run(client, &source, &migrator::RunOptions::new()).await?;
    println!("applied {} migrations", applied.len());

    // Fails with `Error::NotClean` listing what is out of sync.
    migrator::check(client, &source).await
}
```

- `status` compares source and database (`MigrationState`, `MigrationStatus`).
- `check` succeeds only if everything is cleanly applied.
- `log` returns the migration history.
- `run` applies outstanding migrations and returns them.
- `fix` repairs one migration and returns what it did (`FixAction`).

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
use tusker_migration::GlobSource;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Migration(tusker_migration::cli::Command),
}

async fn run(pg_config: &tokio_postgres::Config) -> Result<(), tusker_migration::error::Error> {
    let args = Args::parse();
    // Pick any `MigrationSource` — a filesystem glob here, or `RustEmbedSource` for
    // migrations baked into the binary.
    let source = GlobSource::new("db/migrations/**/*.sql");
    match args.command {
        Command::Migration(command) => {
            tusker_migration::cli::cmd(pg_config, &source, &command).await
        }
    }
}
```

The built-in commands connect without TLS. To connect differently, call the
`migrator` functions with a client of your own.

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

Fails if migration files and database state are not in sync, listing the
migrations that are not.

This is useful in CI or before applying new migrations.

### `run`

Applies all outstanding migrations in order (see
[Applying migrations](#applying-migrations)), or with `--number N` only those
up to and including `N`.

If the migration table does not exist yet, it is created automatically.

### `fix`

Reconciles migration state for one migration number:

- `Mismatch` -> updates the stored migration hash/name entry
- `New` -> marks the migration as applied without running the SQL (`fake`)
- `Migration file missing` -> removes the current migration entry

This is a repair tool and should be used carefully.

## Using It From Your App

Call the `migrator` functions directly, e.g. `migrator::run` at startup or
`migrator::status` to refuse starting with outstanding migrations. To offer
the migration commands in your own CLI instead:

1. expose `tusker_migration::cli::Command` from your own binary
2. choose a `MigrationSource` (`GlobSource` for the filesystem, `RustEmbedSource`
   for embedded migrations, or your own implementation)
3. pass your application's PostgreSQL config and the source into
   `tusker_migration::cli::cmd`
4. let the crate handle migration status, running, checking, or repair

## How it works

At a high level:

1. Load migrations from a `MigrationSource`
2. Parse the migration number and name from each file name
3. Hash the SQL contents
4. Load the current migration state from PostgreSQL
5. Compare source and database state
6. Apply or repair as requested

The comparison logic is implemented in:

- [src/models.rs](src/models.rs)

The database access and migration log operations live in:

- [src/db.rs](src/db.rs)

The migration sources, loading, and hashing logic live in:

- [src/source/mod.rs](src/source/mod.rs)

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
