use time::OffsetDateTime;
use tusker_query::{FromRow, Query};

#[derive(FromRow)]
pub struct NoRow {}

#[derive(Query)]
#[query(sql = "migration_current", row = MigrationCurrentRow)]
pub struct MigrationCurrent {}

#[derive(FromRow)]
pub struct MigrationCurrentRow {
    pub number: i32,
    pub name: String,
    pub hash: Vec<u8>,
}

#[derive(Query)]
#[query(sql = "migration_log", row = MigrationLogRow)]
pub struct MigrationLog {}

#[derive(FromRow)]
pub struct MigrationLogRow {
    pub number: i32,
    pub name: String,
    pub timestamp: OffsetDateTime,
    pub operation: String,
}

#[derive(Query)]
#[query(sql = "migration_insert", row = NoRow)]
pub struct MigrationInsert<'a> {
    pub number: i32,
    pub name: &'a str,
    pub hash: &'a [u8],
}

#[derive(Query)]
#[query(sql = "migration_fake", row = NoRow)]
pub struct MigrationFake<'a> {
    pub number: i32,
    pub name: &'a str,
    pub hash: &'a [u8],
}

#[derive(Query)]
#[query(sql = "migration_update", row = NoRow)]
pub struct MigrationUpdate<'a> {
    pub number: i32,
    pub name: &'a str,
    pub hash: &'a [u8],
}

#[derive(Query)]
#[query(sql = "migration_delete", row = NoRow)]
pub struct MigrationDelete {
    pub number: i32,
}
