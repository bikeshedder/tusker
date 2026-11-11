use std::error::Error as StdError;

use tokio_postgres::{
    error::{DbError, ErrorPosition as PgErrorPosition},
    Error as PgError,
};
use tusker_query::__private::QueryClient;
use tusker_query::query;

use crate::error::Error;
use crate::models::{AppliedMigration, MigrationLogEntry};
use crate::queries;
use crate::source::Migration;

/// The SQL that creates the `migration` table. It contains no transaction
/// control, so it can run inside the runner's transaction.
const SCHEMA: &str = include_str!("../db/schema.sql");

pub(crate) async fn migration_table_exists<C>(client: &C) -> Result<bool, PgError>
where
    C: QueryClient + ?Sized,
{
    let rows = query(client, queries::MigrationTableExists {}).await?;
    Ok(rows.first().is_some_and(|row| row.exists))
}

pub(crate) async fn init(client: &tokio_postgres::Transaction<'_>) -> Result<(), PgError> {
    client.batch_execute(SCHEMA).await
}

pub(crate) async fn get_migrations<C>(client: &C) -> Result<Vec<AppliedMigration>, PgError>
where
    C: QueryClient + ?Sized,
{
    Ok(query(client, queries::MigrationCurrent {})
        .await?
        .into_iter()
        .map(|row| AppliedMigration {
            number: row.number,
            name: row.name,
            hash: row.hash,
        })
        .collect())
}

pub(crate) async fn get_migration_log<C>(client: &C) -> Result<Vec<MigrationLogEntry>, PgError>
where
    C: QueryClient + ?Sized,
{
    Ok(query(client, queries::MigrationLog {})
        .await?
        .into_iter()
        .map(|row| MigrationLogEntry {
            number: row.number,
            name: row.name,
            timestamp: row.timestamp,
            operation: row.operation,
        })
        .collect())
}

pub(crate) async fn record_apply<C>(client: &C, migration: &Migration) -> Result<(), PgError>
where
    C: QueryClient + ?Sized,
{
    query(
        client,
        queries::MigrationInsert {
            number: migration.number,
            name: &migration.name,
            hash: migration.hash(),
        },
    )
    .await
    .map(|_| ())
}

pub(crate) async fn update_migration<C>(client: &C, migration: &Migration) -> Result<(), PgError>
where
    C: QueryClient + ?Sized,
{
    query(
        client,
        queries::MigrationUpdate {
            number: migration.number,
            name: &migration.name,
            hash: migration.hash(),
        },
    )
    .await
    .map(|_| ())
}

pub(crate) async fn fake_migration<C>(client: &C, migration: &Migration) -> Result<(), PgError>
where
    C: QueryClient + ?Sized,
{
    query(
        client,
        queries::MigrationFake {
            number: migration.number,
            name: &migration.name,
            hash: migration.hash(),
        },
    )
    .await
    .map(|_| ())
}

pub(crate) async fn remove_migration<C>(client: &C, number: i32) -> Result<(), PgError>
where
    C: QueryClient + ?Sized,
{
    query(client, queries::MigrationDelete { number })
        .await
        .map(|_| ())
}

/// Turns a database error raised by a migration's SQL into an error that
/// points at the failing line, if PostgreSQL reported a position.
pub(crate) fn to_sql_error(error: PgError, migration: &Migration) -> Error {
    let context = format!(
        "Applying migration {} ({}) failed",
        migration.number, migration.name
    );
    let Some(db_error) = error.source().and_then(|e| e.downcast_ref::<DbError>()) else {
        return Error::Pg(context, error);
    };
    let position = match db_error.position() {
        Some(PgErrorPosition::Original(position)) => Some(*position as usize),
        _ => None,
    };
    let message = format!("{}: {}", db_error.severity(), db_error.message());
    match position.and_then(|position| point_at(&migration.sql, position)) {
        Some(pointer) => Error::Sql(format!("{}\n{}\n{}", context, message, pointer)),
        None => Error::Sql(format!("{}\n{}", context, message)),
    }
}

/// Shows the line containing the 1-based character `position` of `sql` with a
/// caret below the character.
fn point_at(sql: &str, position: usize) -> Option<String> {
    let offset = sql.char_indices().nth(position.checked_sub(1)?)?.0;
    let line_begin = sql[..offset].rfind('\n').map_or(0, |p| p + 1);
    let line_end = sql[offset..].find('\n').map_or(sql.len(), |p| p + offset);
    let line = sql[line_begin..line_end].trim_end_matches('\r');
    let line_number = sql[..offset].matches('\n').count() + 1;
    let prefix = format!("LINE {}: ", line_number);
    let column = sql[line_begin..offset].chars().count();
    Some(format!(
        "{}{}\n{}^",
        prefix,
        line,
        " ".repeat(prefix.len() + column)
    ))
}

#[cfg(test)]
mod tests {
    use super::point_at;

    #[test]
    fn points_at_the_character() {
        let sql = "CREATE TABLE a (id int);\r\nCREATE TABLE b (id innt);\n";
        let position = sql.find("innt").unwrap() + 1;
        let pointed = point_at(sql, position).unwrap();
        let (line, caret) = pointed.split_once('\n').unwrap();
        assert_eq!(line, "LINE 2: CREATE TABLE b (id innt);");
        assert_eq!(caret.find('^'), line.find("innt"));
        assert_eq!(caret.trim(), "^");
    }

    #[test]
    fn first_character_and_out_of_range() {
        assert_eq!(
            point_at("SELEC 1", 1).unwrap(),
            "LINE 1: SELEC 1\n        ^"
        );
        assert_eq!(point_at("SELECT", 99), None);
        assert_eq!(point_at("SELECT", 0), None);
    }
}
