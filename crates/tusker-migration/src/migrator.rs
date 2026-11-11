//! The migration runner as a library: inspect, check, apply and repair
//! migrations without printing anything.
//!
//! Every function takes a connected [`tokio_postgres::Client`], so the caller
//! decides how to connect (TLS or not) and may take the client from a pool.
//! The source is loaded when the function is called; the returned futures
//! don't borrow it and are `Send`.
//! With `deadpool-postgres`, `&**client` and `&mut **client` give the
//! underlying `tokio_postgres::Client`.
//!
//! [`run`] and [`fix`] hold a session-level advisory lock ([`LOCK_KEY`]) while
//! they work, so several processes migrating the same database at once wait
//! for each other and apply each migration exactly once. Each migration runs
//! in its own transaction together with its log entry: a failing migration
//! leaves neither its changes nor a log entry behind. A migration whose first
//! line is `-- tusker:no-transaction` runs outside a transaction instead (for
//! statements such as `CREATE INDEX CONCURRENTLY`); it is logged after it
//! succeeded.
//!
//! ```no_run
//! # async fn example(client: &mut tokio_postgres::Client) -> Result<(), tusker_migration::error::Error> {
//! use tusker_migration::{migrator, GlobSource};
//!
//! let source = GlobSource::new("db/migrations/**/*.sql");
//! for migration in migrator::run(client, &source, &migrator::RunOptions::new()).await? {
//!     println!("applied {:04}_{}", migration.number, migration.name);
//! }
//! migrator::check(client, &source).await?;
//! # Ok(())
//! # }
//! ```

use std::future::Future;

use tokio_postgres::Client;
use tusker_query::__private::QueryClient;

use crate::db;
use crate::error::Error;
use crate::models::{
    combine_migrations, AppliedMigration, MigrationLogEntry, MigrationState, MigrationStatus,
};
use crate::source::{Migration, MigrationSource};

/// Key of the session-level advisory lock held by [`run`] and [`fix`]
/// (`pg_advisory_lock(LOCK_KEY)`): the bytes of `"tuskermi"`.
pub const LOCK_KEY: i64 = 0x7475_736b_6572_6d69;

/// Options for [`run`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RunOptions {
    /// Apply outstanding migrations only up to and including this number.
    /// `None` applies all of them.
    pub up_to: Option<i32>,
}

impl RunOptions {
    /// Options that apply all outstanding migrations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply outstanding migrations only up to and including `number`.
    pub fn up_to(mut self, number: i32) -> Self {
        self.up_to = Some(number);
        self
    }
}

/// What [`fix`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixAction {
    /// The migration's name and hash in the database were updated to match
    /// the source ([`MigrationStatus::Mismatch`]).
    Updated,
    /// The migration was recorded as applied without running it
    /// ([`MigrationStatus::New`]).
    Faked,
    /// The database entry was removed because the source has no file for it
    /// ([`MigrationStatus::FileMissing`]).
    Removed,
}

/// Compares the migrations of `source` with the database, ordered by number.
///
/// A database without the migration table counts as having no migrations
/// applied.
pub fn status<'a>(
    client: &'a Client,
    source: &dyn MigrationSource,
) -> impl Future<Output = Result<Vec<MigrationState>, Error>> + Send + 'a {
    let loaded = source.load();
    async move {
        let migrations = loaded?;
        let applied = load_applied(client).await?;
        Ok(combine_migrations(&migrations, &applied))
    }
}

/// Succeeds if every migration of `source` is applied and unchanged, and the
/// database knows no other migrations. Otherwise returns
/// [`Error::NotClean`] with the migrations that are not
/// [`MigrationStatus::Ok`].
pub fn check<'a>(
    client: &'a Client,
    source: &dyn MigrationSource,
) -> impl Future<Output = Result<(), Error>> + Send + 'a {
    let status = status(client, source);
    async move {
        let not_ok: Vec<MigrationState> = status
            .await?
            .into_iter()
            .filter(|state| state.status() != MigrationStatus::Ok)
            .collect();
        if not_ok.is_empty() {
            Ok(())
        } else {
            Err(Error::NotClean(not_ok))
        }
    }
}

/// The migration log, oldest entry first. Empty if the migration table does
/// not exist.
pub async fn log(client: &Client) -> Result<Vec<MigrationLogEntry>, Error> {
    if !table_exists(client).await? {
        return Ok(Vec::new());
    }
    db::get_migration_log(client)
        .await
        .map_err(|e| Error::Pg("Unable to load the migration log".into(), e))
}

/// Applies the outstanding migrations of `source` in order and returns the
/// ones it applied.
///
/// Creates the migration table first if needed. Refuses to apply anything
/// ([`Error::NotClean`]) while a migration is [`MigrationStatus::Mismatch`] or
/// [`MigrationStatus::FileMissing`]; repair those with [`fix`] first.
pub fn run<'a>(
    client: &'a mut Client,
    source: &dyn MigrationSource,
    options: &RunOptions,
) -> impl Future<Output = Result<Vec<Migration>, Error>> + Send + 'a {
    let loaded = source.load();
    let options = *options;
    async move {
        let migrations = loaded?;
        if let Some(number) = options.up_to {
            if !migrations.iter().any(|m| m.number == number) {
                return Err(Error::Misc(format!(
                    "Migration number does not exist: {}",
                    number
                )));
            }
        }
        with_lock(client, |client| {
            Box::pin(run_locked(client, migrations, options))
        })
        .await
    }
}

/// Repairs the state of one migration, see [`FixAction`]. Fails if the
/// migration is [`MigrationStatus::Ok`] or the number is unknown.
pub fn fix<'a>(
    client: &'a mut Client,
    source: &dyn MigrationSource,
    number: i32,
) -> impl Future<Output = Result<FixAction, Error>> + Send + 'a {
    let loaded = source.load();
    async move {
        let migrations = loaded?;
        with_lock(client, |client| {
            Box::pin(fix_locked(client, migrations, number))
        })
        .await
    }
}

type LockedFuture<'a, T> = std::pin::Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// Runs `f` while holding the advisory lock, and releases the lock whether
/// `f` succeeded or not.
async fn with_lock<T, F>(client: &mut Client, f: F) -> Result<T, Error>
where
    F: for<'a> FnOnce(&'a mut Client) -> LockedFuture<'a, T>,
{
    let _ = client
        .execute("SELECT pg_advisory_lock($1)", &[&LOCK_KEY])
        .await
        .map_err(|e| Error::Pg("Unable to acquire the migration lock".into(), e))?;
    let result = f(client).await;
    let unlocked = client
        .execute("SELECT pg_advisory_unlock($1)", &[&LOCK_KEY])
        .await
        .map_err(|e| Error::Pg("Unable to release the migration lock".into(), e));
    let value = result?;
    let _ = unlocked?;
    Ok(value)
}

async fn run_locked(
    client: &mut Client,
    migrations: Vec<Migration>,
    options: RunOptions,
) -> Result<Vec<Migration>, Error> {
    // Loaded under the lock: another runner may just have applied some.
    let applied = load_applied(&*client).await?;
    let states = combine_migrations(&migrations, &applied);
    let blocking: Vec<MigrationState> = states
        .iter()
        .filter(|state| {
            matches!(
                state.status(),
                MigrationStatus::Mismatch { .. } | MigrationStatus::FileMissing
            )
        })
        .cloned()
        .collect();
    if !blocking.is_empty() {
        return Err(Error::NotClean(blocking));
    }
    let pending = states
        .into_iter()
        .filter(|state| state.status() == MigrationStatus::New)
        .filter_map(|state| state.migration)
        .filter(|migration| options.up_to.is_none_or(|up_to| migration.number <= up_to));

    let mut table_exists = table_exists(&*client).await?;
    let mut done = Vec::new();
    for migration in pending {
        if migration.no_transaction() {
            if !table_exists {
                create_table(client).await?;
                table_exists = true;
            }
            client
                .batch_execute(&migration.sql)
                .await
                .map_err(|e| db::to_sql_error(e, &migration))?;
            db::record_apply(&*client, &migration)
                .await
                .map_err(|e| log_error(&migration, e))?;
        } else {
            let tx = client
                .transaction()
                .await
                .map_err(|e| Error::Pg("Unable to start a transaction".into(), e))?;
            if !table_exists {
                db::init(&tx)
                    .await
                    .map_err(|e| Error::Pg("Unable to create the migration table".into(), e))?;
            }
            tx.batch_execute(&migration.sql)
                .await
                .map_err(|e| db::to_sql_error(e, &migration))?;
            db::record_apply(&tx, &migration)
                .await
                .map_err(|e| log_error(&migration, e))?;
            tx.commit().await.map_err(|e| {
                Error::Pg(
                    format!(
                        "Committing migration {} ({}) failed",
                        migration.number, migration.name
                    ),
                    e,
                )
            })?;
            table_exists = true;
        }
        done.push(migration);
    }
    if !table_exists {
        create_table(client).await?;
    }
    Ok(done)
}

async fn fix_locked(
    client: &mut Client,
    migrations: Vec<Migration>,
    number: i32,
) -> Result<FixAction, Error> {
    let applied = load_applied(&*client).await?;
    let state = combine_migrations(&migrations, &applied)
        .into_iter()
        .find(|state| state.number == number)
        .ok_or_else(|| Error::Misc(format!("Migration number does not exist: {}", number)))?;
    let tx = client
        .transaction()
        .await
        .map_err(|e| Error::Pg("Unable to start a transaction".into(), e))?;
    if !table_exists(&tx).await? {
        db::init(&tx)
            .await
            .map_err(|e| Error::Pg("Unable to create the migration table".into(), e))?;
    }
    let fixing = |e| Error::Pg("Fixing migration failed".into(), e);
    let action = match (state.status(), &state.migration) {
        (MigrationStatus::Ok, _) => {
            return Err(Error::Misc(format!(
                "Migration does not require fixing: {}",
                number
            )));
        }
        (MigrationStatus::Mismatch { .. }, Some(migration)) => {
            db::update_migration(&tx, migration).await.map_err(fixing)?;
            FixAction::Updated
        }
        (MigrationStatus::New, Some(migration)) => {
            db::fake_migration(&tx, migration).await.map_err(fixing)?;
            FixAction::Faked
        }
        (MigrationStatus::FileMissing, _) => {
            db::remove_migration(&tx, number).await.map_err(fixing)?;
            FixAction::Removed
        }
        (_, None) => unreachable!("mismatched or new migration without a source file"),
    };
    tx.commit()
        .await
        .map_err(|e| Error::Pg("Committing the fix failed".into(), e))?;
    Ok(action)
}

async fn create_table(client: &mut Client) -> Result<(), Error> {
    let tx = client
        .transaction()
        .await
        .map_err(|e| Error::Pg("Unable to start a transaction".into(), e))?;
    db::init(&tx)
        .await
        .map_err(|e| Error::Pg("Unable to create the migration table".into(), e))?;
    tx.commit()
        .await
        .map_err(|e| Error::Pg("Unable to create the migration table".into(), e))
}

async fn table_exists<C>(client: &C) -> Result<bool, Error>
where
    C: QueryClient + ?Sized,
{
    db::migration_table_exists(client)
        .await
        .map_err(|e| Error::Pg("Checking the migration table failed".into(), e))
}

async fn load_applied<C>(client: &C) -> Result<Vec<AppliedMigration>, Error>
where
    C: QueryClient + ?Sized,
{
    if !table_exists(client).await? {
        return Ok(Vec::new());
    }
    db::get_migrations(client)
        .await
        .map_err(|e| Error::Pg("Unable to load already applied migrations".into(), e))
}

fn log_error(migration: &Migration, error: tokio_postgres::Error) -> Error {
    Error::Pg(
        format!(
            "Recording migration {} ({}) failed",
            migration.number, migration.name
        ),
        error,
    )
}
