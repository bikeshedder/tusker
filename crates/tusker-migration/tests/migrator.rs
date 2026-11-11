//! Tests for [`tusker_migration::migrator`] against a real database.
//!
//! They need `PG_URL` (a libpq-style string or URL of a role that may create
//! databases). Every test creates its own database and drops it afterwards.

use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio_postgres::{Client, Config, NoTls};
use tusker_migration::error::Error;
use tusker_migration::migrator::{self, FixAction, RunOptions};
use tusker_migration::{Migration, MigrationSource, MigrationStatus};

static NEXT_DB_ID: AtomicUsize = AtomicUsize::new(0);

/// A source made of migrations defined in the test.
struct Source(Vec<Migration>);

impl Source {
    fn new(migrations: &[(i32, &str, &str)]) -> Self {
        Self(
            migrations
                .iter()
                .map(|(number, name, sql)| Migration::new(*number, *name, *sql))
                .collect(),
        )
    }
}

impl MigrationSource for Source {
    fn load(&self) -> Result<Vec<Migration>, Error> {
        Ok(self.0.clone())
    }
}

struct TestDatabase {
    admin: Client,
    config: Config,
    dbname: String,
}

impl TestDatabase {
    async fn new() -> Self {
        let url = env::var("PG_URL").expect("Missing environment variable: PG_URL");
        let mut admin_config: Config = url.parse().unwrap();
        let _ = admin_config.dbname("postgres");
        let (admin, connection) = admin_config.connect(NoTls).await.unwrap();
        drop(tokio::spawn(connection));
        let id = NEXT_DB_ID.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dbname = format!("tusker_migration_test_{}_{}", timestamp, id);
        let _ = admin
            .simple_query(&format!("CREATE DATABASE {}", dbname))
            .await
            .unwrap();
        let mut config: Config = url.parse().unwrap();
        let _ = config.dbname(&dbname);
        Self {
            admin,
            config,
            dbname,
        }
    }

    async fn connect(&self) -> (Client, JoinHandle<()>) {
        let (client, connection) = self.config.connect(NoTls).await.unwrap();
        let handle = tokio::spawn(async move {
            let _ = connection.await;
        });
        (client, handle)
    }

    async fn cleanup(self) {
        let _ = self
            .admin
            .simple_query(&format!("DROP DATABASE {} WITH (FORCE)", self.dbname))
            .await
            .unwrap();
    }
}

async fn tables(client: &Client) -> Vec<String> {
    client
        .query(
            "SELECT table_name::text FROM information_schema.tables \
             WHERE table_schema = 'public' ORDER BY 1",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect()
}

fn statuses(states: &[tusker_migration::MigrationState]) -> Vec<(i32, MigrationStatus)> {
    states.iter().map(|s| (s.number, s.status())).collect()
}

const FIRST: (i32, &str, &str) = (1, "first", "CREATE TABLE a (id int);");
const SECOND: (i32, &str, &str) = (2, "second", "CREATE TABLE b (id int);");

#[tokio::test]
async fn run_applies_outstanding_migrations_once() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let source = Source::new(&[SECOND, FIRST]);

    assert_eq!(
        statuses(&migrator::status(&client, &source).await.unwrap()),
        vec![(1, MigrationStatus::New), (2, MigrationStatus::New)]
    );
    assert!(migrator::check(&client, &source).await.is_err());
    assert!(migrator::log(&client).await.unwrap().is_empty());

    let applied = migrator::run(&mut client, &source, &RunOptions::new())
        .await
        .unwrap();
    assert_eq!(
        applied.iter().map(|m| m.number).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(tables(&client).await, vec!["a", "b", "migration"]);
    migrator::check(&client, &source).await.unwrap();

    let again = migrator::run(&mut client, &source, &RunOptions::new())
        .await
        .unwrap();
    assert!(again.is_empty());
    let log = migrator::log(&client).await.unwrap();
    assert_eq!(log.len(), 2);
    assert!(log.iter().all(|entry| entry.operation == "apply"));

    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn run_without_migrations_creates_the_table() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let applied = migrator::run(&mut client, &Source::new(&[]), &RunOptions::new())
        .await
        .unwrap();
    assert!(applied.is_empty());
    assert_eq!(tables(&client).await, vec!["migration"]);
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn failing_migration_leaves_nothing_behind() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let source = Source::new(&[
        FIRST,
        (
            2,
            "broken",
            "CREATE TABLE b (id int);\nCREATE TABLE c (id innt);",
        ),
    ]);
    let error = migrator::run(&mut client, &source, &RunOptions::new())
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("Applying migration 2 (broken) failed"),
        "{message}"
    );
    assert!(
        message.contains("LINE 2: CREATE TABLE c (id innt);"),
        "{message}"
    );

    // The first migration stays applied, the second left neither table b
    // nor a log entry.
    assert_eq!(tables(&client).await, vec!["a", "migration"]);
    assert_eq!(
        statuses(&migrator::status(&client, &source).await.unwrap()),
        vec![(1, MigrationStatus::Ok), (2, MigrationStatus::New)]
    );
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn failing_first_migration_rolls_back_the_migration_table() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let source = Source::new(&[(1, "broken", "CREATE TABLE a (id innt);")]);
    assert!(migrator::run(&mut client, &source, &RunOptions::new())
        .await
        .is_err());
    assert!(tables(&client).await.is_empty());
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn no_transaction_migration() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let source = Source::new(&[
        FIRST,
        (
            2,
            "concurrent_index",
            "-- tusker:no-transaction\nCREATE INDEX CONCURRENTLY a_id_idx ON a (id);",
        ),
    ]);
    assert!(source.0[1].no_transaction());
    assert!(!source.0[0].no_transaction());
    let applied = migrator::run(&mut client, &source, &RunOptions::new())
        .await
        .unwrap();
    assert_eq!(applied.len(), 2);
    let index = client
        .query_one("SELECT to_regclass('a_id_idx') IS NOT NULL", &[])
        .await
        .unwrap();
    assert!(index.get::<_, bool>(0));
    migrator::check(&client, &source).await.unwrap();
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn up_to_applies_a_prefix() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let source = Source::new(&[FIRST, SECOND]);
    let applied = migrator::run(&mut client, &source, &RunOptions::new().up_to(1))
        .await
        .unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(
        statuses(&migrator::status(&client, &source).await.unwrap()),
        vec![(1, MigrationStatus::Ok), (2, MigrationStatus::New)]
    );
    let unknown = migrator::run(&mut client, &source, &RunOptions::new().up_to(7)).await;
    assert!(unknown.is_err());
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn mismatch_blocks_run_until_fixed() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let _ = migrator::run(&mut client, &Source::new(&[FIRST]), &RunOptions::new())
        .await
        .unwrap();

    let changed = Source::new(&[(1, "first", "CREATE TABLE a (id bigint);"), SECOND]);
    let error = migrator::run(&mut client, &changed, &RunOptions::new())
        .await
        .unwrap_err();
    let states = error.not_clean().expect("NotClean");
    assert_eq!(
        statuses(states),
        vec![(
            1,
            MigrationStatus::Mismatch {
                name: false,
                hash: true
            }
        )]
    );
    // Nothing was applied: migration 2 is still outstanding.
    assert_eq!(tables(&client).await, vec!["a", "migration"]);

    let check = migrator::check(&client, &changed).await.unwrap_err();
    assert_eq!(statuses(check.not_clean().unwrap()).len(), 2);

    assert_eq!(
        migrator::fix(&mut client, &changed, 1).await.unwrap(),
        FixAction::Updated
    );
    let _ = migrator::run(&mut client, &changed, &RunOptions::new())
        .await
        .unwrap();
    migrator::check(&client, &changed).await.unwrap();
    assert!(migrator::fix(&mut client, &changed, 1).await.is_err());
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn fix_fakes_new_and_removes_missing() {
    let db = TestDatabase::new().await;
    let (mut client, _conn) = db.connect().await;
    let source = Source::new(&[FIRST]);
    assert_eq!(
        migrator::fix(&mut client, &source, 1).await.unwrap(),
        FixAction::Faked
    );
    // Faked: recorded, but the table was never created.
    assert_eq!(tables(&client).await, vec!["migration"]);
    migrator::check(&client, &source).await.unwrap();

    assert_eq!(
        migrator::fix(&mut client, &Source::new(&[]), 1)
            .await
            .unwrap(),
        FixAction::Removed
    );
    assert_eq!(
        statuses(&migrator::status(&client, &source).await.unwrap()),
        vec![(1, MigrationStatus::New)]
    );
    let operations: Vec<String> = migrator::log(&client)
        .await
        .unwrap()
        .into_iter()
        .map(|entry| entry.operation)
        .collect();
    assert_eq!(operations, vec!["fake", "delete"]);
    drop(client);
    db.cleanup().await;
}

#[tokio::test]
async fn concurrent_runs_apply_each_migration_once() {
    let db = TestDatabase::new().await;
    // Slow enough that the runners overlap.
    let source = || {
        Source::new(&[
            (1, "slow", "SELECT pg_sleep(0.3); CREATE TABLE a (id int);"),
            SECOND,
        ])
    };
    let mut runners = Vec::new();
    for _ in 0..4 {
        let (mut client, conn) = db.connect().await;
        runners.push(tokio::spawn(async move {
            let applied = migrator::run(&mut client, &source(), &RunOptions::new())
                .await
                .unwrap();
            drop(client);
            let _ = conn.await;
            applied.len()
        }));
    }
    let mut total = 0;
    for runner in runners {
        total += runner.await.unwrap();
    }
    assert_eq!(total, 2);
    let (client, _conn) = db.connect().await;
    assert_eq!(migrator::log(&client).await.unwrap().len(), 2);
    drop(client);
    db.cleanup().await;
}
