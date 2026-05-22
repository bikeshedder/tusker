use std::error::Error as StdError;

use time::OffsetDateTime;
use tokio_postgres::{
    error::{DbError, ErrorPosition as PgErrorPosition},
    Error as PgError,
};
use tusker_query::query;

use crate::error::Error;
use crate::file::MigrationFile;
use crate::queries;

#[derive(Debug)]
pub(crate) struct Database {
    pub(crate) client: tokio_postgres::Client,
}

impl Database {
    pub(crate) async fn connect(pg_config: &tokio_postgres::Config) -> Result<Database, Error> {
        let (client, connection) = pg_config
            .connect(tokio_postgres::NoTls)
            .await
            .map_err(|e| Error::Pg("Unable to connect to database".into(), e))?;
        drop(tokio::spawn(connection));
        Ok(Database { client })
    }
    pub(crate) async fn migration_table_exists(&self) -> Result<bool, PgError> {
        let stmt = self
            .client
            .prepare("SELECT to_regclass('migration')::bigint")
            .await?;
        let result = self.client.query(&stmt, &[]).await?;
        Ok(match result.first() {
            Some(row) => row.get::<_, Option<i64>>(0).is_some(),
            None => false,
        })
    }
    pub(crate) async fn init(&self) -> Result<(), PgError> {
        let sql = include_str!("../db/schema.sql");
        self.client.simple_query(sql).await.map(|_| ())
    }
    pub(crate) async fn get_migrations(&self) -> Result<Vec<DbMigration>, PgError> {
        Ok(query(&self.client, queries::MigrationCurrent {})
            .await?
            .iter()
            .map(|row| DbMigration {
                number: row.number,
                name: row.name.clone(),
                hash: row.hash.clone(),
            })
            .collect())
    }
    pub(crate) async fn get_migration_log(&self) -> Result<Vec<DbMigrationLog>, PgError> {
        Ok(query(&self.client, queries::MigrationLog {})
            .await?
            .iter()
            .map(|row| DbMigrationLog {
                number: row.number,
                name: row.name.clone(),
                timestamp: row.timestamp,
                operation: row.operation.clone(),
            })
            .collect())
    }
    pub(crate) async fn update_migration(
        &self,
        migration_file: &MigrationFile,
    ) -> Result<(), PgError> {
        let _ = query(
            &self.client,
            queries::MigrationUpdate {
                number: migration_file.number,
                name: &migration_file.name,
                hash: &migration_file.hash,
            },
        )
        .await?;
        Ok(())
    }
    pub(crate) async fn apply_migration(
        &self,
        migration_file: &MigrationFile,
        sql: &str,
    ) -> Result<(), PgError> {
        self.client.simple_query(sql).await.map(|_| ())?;
        // log that migration has been run
        query(
            &self.client,
            queries::MigrationInsert {
                number: migration_file.number,
                name: &migration_file.name,
                hash: &migration_file.hash,
            },
        )
        .await
        .map(|_| ())
    }
    pub(crate) async fn fake_migration(
        &self,
        migration_file: &MigrationFile,
    ) -> Result<(), PgError> {
        query(
            &self.client,
            queries::MigrationFake {
                number: migration_file.number,
                name: &migration_file.name,
                hash: &migration_file.hash,
            },
        )
        .await
        .map(|_| ())
    }
    pub(crate) async fn remove_migration(&self, number: i32) -> Result<(), PgError> {
        let _ = query(&self.client, queries::MigrationDelete { number }).await?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DbMigration {
    pub(crate) number: i32,
    pub(crate) name: String,
    pub(crate) hash: Vec<u8>,
    //applied: std::time::
}

#[derive(Clone, Debug)]
pub(crate) struct DbMigrationLog {
    pub(crate) number: i32,
    pub(crate) name: String,
    pub(crate) timestamp: OffsetDateTime,
    pub(crate) operation: String,
}

pub(crate) fn to_sql_error(error: PgError, sql: &str) -> Error {
    // FIXME This function is really ugly and only works if the newline
    // is only '\n'.
    match error.source().unwrap().downcast_ref::<DbError>() {
        Some(db_error) => {
            println!("{}: {}", db_error.severity(), db_error.message());
            let position = match db_error.position() {
                Some(PgErrorPosition::Original(position)) => Some(*position),
                Some(PgErrorPosition::Internal { position, query: _ }) => Some(*position),
                None => None,
            };
            //let position = db_error.line();
            if let Some(position) = position {
                let position = position as usize;
                let line_begin = sql[..position].rfind('\n').map(|p| p + 1).unwrap_or(0);
                let line_end = sql[position..].find('\n').unwrap_or(sql.len()) + position;
                let line_position = position - line_begin;
                let mut remaining = position;
                let mut line_number = 1;
                for line in sql.lines() {
                    if remaining <= line.len() {
                        break;
                    } else {
                        remaining -= line.len() + 1; // FIXME assuming \n and no \r\n
                        line_number += 1;
                    }
                }
                let prefix = format!("LINE {}: ", line_number);
                let mut msg = format!("{}{}", prefix, &sql[line_begin..line_end]);
                // The position is 1 indexed. Thus 1 needs to be subtracted.
                for _ in 0..(prefix.len() + line_position - 1) {
                    msg += " ";
                }
                msg += "^";
                Error::Sql(msg)
            } else {
                Error::Sql(format!("SQL error: {}", error))
            }
        }
        None => Error::Pg("Unknown error".into(), error),
    }
}
