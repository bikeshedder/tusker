use time::OffsetDateTime;
use tusker_query::{FromRow, Query};

#[derive(FromRow)]
pub(crate) struct NoRow {}

#[derive(Query)]
#[query(sql = "migration_current", row = MigrationCurrentRow)]
pub(crate) struct MigrationCurrent {}

#[derive(FromRow)]
pub(crate) struct MigrationCurrentRow {
    pub(crate) number: i32,
    pub(crate) name: String,
    pub(crate) hash: Vec<u8>,
}

#[derive(Query)]
#[query(sql = "migration_log", row = MigrationLogRow)]
pub(crate) struct MigrationLog {}

#[derive(FromRow)]
pub(crate) struct MigrationLogRow {
    pub(crate) number: i32,
    pub(crate) name: String,
    pub(crate) timestamp: OffsetDateTime,
    pub(crate) operation: String,
}

#[derive(Query)]
#[query(sql = "migration_insert", row = NoRow)]
pub(crate) struct MigrationInsert<'a> {
    pub(crate) number: i32,
    pub(crate) name: &'a str,
    pub(crate) hash: &'a [u8],
}

#[derive(Query)]
#[query(sql = "migration_fake", row = NoRow)]
pub(crate) struct MigrationFake<'a> {
    pub(crate) number: i32,
    pub(crate) name: &'a str,
    pub(crate) hash: &'a [u8],
}

#[derive(Query)]
#[query(sql = "migration_update", row = NoRow)]
pub(crate) struct MigrationUpdate<'a> {
    pub(crate) number: i32,
    pub(crate) name: &'a str,
    pub(crate) hash: &'a [u8],
}

#[derive(Query)]
#[query(sql = "migration_delete", row = NoRow)]
pub(crate) struct MigrationDelete {
    pub(crate) number: i32,
}
