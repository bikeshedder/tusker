use std::{
    env,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Result;
use tokio_postgres::{Client, NoTls};

use super::inspect_query_sql;

static NEXT_SCHEMA_ID: AtomicU64 = AtomicU64::new(0);

/// A schema with test tables, created on the database at `PG_URL`.
struct TestSchema {
    client: Client,
    name: String,
}

impl TestSchema {
    async fn new(tables: &str) -> Result<Self> {
        let url = env::var("PG_URL").expect("Missing environment variable: PG_URL");
        let (client, connection) = tokio_postgres::connect(&url, NoTls).await?;
        drop(tokio::spawn(connection));
        let name = format!(
            "tusker_query_test_{}_{}",
            std::process::id(),
            NEXT_SCHEMA_ID.fetch_add(1, Ordering::Relaxed)
        );
        client
            .batch_execute(&format!(
                "CREATE SCHEMA {name}; SET search_path = {name}; {tables}"
            ))
            .await?;
        Ok(Self { client, name })
    }

    async fn notnull(&self, sql: &str) -> Result<Vec<(String, Option<bool>)>> {
        let query = inspect_query_sql(&self.client, sql).await?;
        Ok(query
            .columns
            .into_iter()
            .map(|c| (c.name, c.notnull))
            .collect())
    }

    async fn drop(self) -> Result<()> {
        self.client
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.name))
            .await?;
        Ok(())
    }
}

fn columns(expected: &[(&str, Option<bool>)]) -> Vec<(String, Option<bool>)> {
    expected
        .iter()
        .map(|(name, notnull)| ((*name).to_owned(), *notnull))
        .collect()
}

/// A query and the expected nullability of its columns.
type Case<'a> = (&'a str, &'a [(&'a str, Option<bool>)]);

/// Create `tables` and check the nullability of the columns of each query.
async fn check(tables: &str, cases: &[Case<'_>]) -> Result<()> {
    let schema = TestSchema::new(tables).await?;
    for (sql, expected) in cases {
        assert_eq!(schema.notnull(sql).await?, columns(expected), "{sql}");
    }
    schema.drop().await
}

const T: Option<bool> = Some(true);
const F: Option<bool> = Some(false);
const U: Option<bool> = None;

/// `parent` has no primary key, so that the planner runs some left joins as
/// right joins with swapped inputs.
const PARENT_CHILD: &str = "
    CREATE TABLE parent (id BIGINT NOT NULL, value TEXT NOT NULL);
    CREATE TABLE child (id BIGINT NOT NULL, parent_id BIGINT);
";

#[tokio::test]
async fn left_join() -> Result<()> {
    check(
        PARENT_CHILD,
        &[
            (
                "SELECT c.id, p.value FROM child c JOIN parent p ON p.id = c.parent_id",
                &[("id", T), ("value", T)],
            ),
            // The planner runs this as a right join with the inputs swapped.
            (
                "SELECT c.id, p.value FROM child c LEFT JOIN parent p ON p.id = c.parent_id",
                &[("id", T), ("value", F)],
            ),
            (
                "SELECT c.id, p.value FROM parent p RIGHT JOIN child c ON p.id = c.parent_id",
                &[("id", T), ("value", F)],
            ),
            (
                "SELECT c.id, p.value FROM parent p FULL JOIN child c ON p.id = c.parent_id",
                &[("id", F), ("value", F)],
            ),
            // The planner drops `parent` from the plan.
            (
                "SELECT c.id, p.value FROM child c LEFT JOIN parent p ON false",
                &[("id", T), ("value", F)],
            ),
            (
                "SELECT c.id, p.value FROM child c \
                LEFT JOIN LATERAL (SELECT value FROM parent WHERE id = c.parent_id LIMIT 1) p ON true",
                &[("id", T), ("value", F)],
            ),
            // The same table on both sides of the join.
            (
                "SELECT a.value AS a, b.value AS b FROM parent a LEFT JOIN parent b ON b.id = a.id + 1",
                &[("a", F), ("b", F)],
            ),
            // A strict condition on the nullable side turns it into an inner join.
            (
                "SELECT p.value FROM child c LEFT JOIN parent p ON p.id = c.parent_id \
                WHERE p.value = $1",
                &[("value", T)],
            ),
            // Semi and anti joins do not NULL-extend.
            (
                "SELECT p.value FROM parent p WHERE EXISTS (SELECT FROM child c WHERE c.parent_id = p.id)",
                &[("value", T)],
            ),
            (
                "SELECT p.value FROM parent p WHERE NOT EXISTS (SELECT FROM child c WHERE c.parent_id = p.id)",
                &[("value", T)],
            ),
        ],
    )
    .await
}

/// The plan scans the table inside the CTE, not the CTE itself.
#[tokio::test]
async fn left_join_cte() -> Result<()> {
    check(
        PARENT_CHILD,
        &[
            (
                "WITH x AS MATERIALIZED (SELECT * FROM parent) SELECT x.value FROM x",
                &[("value", T)],
            ),
            (
                "WITH x AS MATERIALIZED (SELECT * FROM parent) \
                SELECT c.id, x.value FROM child c LEFT JOIN x ON x.id = c.parent_id",
                &[("id", T), ("value", F)],
            ),
            (
                "WITH x AS MATERIALIZED (SELECT * FROM parent), \
                    y AS MATERIALIZED (SELECT * FROM x) \
                SELECT c.id, y.value FROM child c LEFT JOIN y ON y.id = c.parent_id",
                &[("id", T), ("value", F)],
            ),
        ],
    )
    .await
}

/// The plan scans the partitions of a partitioned table, not the table itself.
#[tokio::test]
async fn left_join_partitioned() -> Result<()> {
    check(
        "
        CREATE TABLE parent (id BIGINT NOT NULL, value TEXT NOT NULL)
            PARTITION BY LIST (id);
        CREATE TABLE parent_default PARTITION OF parent DEFAULT;
        CREATE TABLE child (id BIGINT NOT NULL, parent_id BIGINT);
        ",
        &[
            ("SELECT value FROM parent", &[("value", T)]),
            (
                "SELECT c.id, p.value FROM child c LEFT JOIN parent p ON p.id = c.parent_id",
                &[("id", T), ("value", F)],
            ),
        ],
    )
    .await
}

/// Grouping sets put NULL into the grouping columns that a set leaves out.
#[tokio::test]
async fn grouping_sets() -> Result<()> {
    check(
        "CREATE TABLE item (value TEXT NOT NULL);",
        &[
            (
                "SELECT value, count(*) FROM item GROUP BY value",
                &[("value", T), ("count", U)],
            ),
            (
                "SELECT value, count(*) FROM item GROUP BY ROLLUP (value)",
                &[("value", F), ("count", U)],
            ),
        ],
    )
    .await
}
