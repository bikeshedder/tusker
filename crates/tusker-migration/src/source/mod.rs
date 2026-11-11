//! Pluggable sources of migration files.
//!
//! A [`MigrationSource`] decouples *where* migrations come from (a directory on
//! disk, files embedded into the binary, or something else entirely) from the
//! runner logic that inspects and applies them. The crate ships two
//! implementations:
//!
//! - [`GlobSource`] loads migrations from the filesystem using a glob pattern.
//! - [`RustEmbedSource`] loads migrations embedded into the binary via
//!   [`rust-embed`](https://docs.rs/rust-embed) (requires the `rust-embed`
//!   feature).
//!
//! Applications can also implement [`MigrationSource`] themselves and construct
//! [`Migration`] values directly with [`Migration::new`].

use std::collections::HashSet;

use sha2::{Digest, Sha512};

use crate::error::Error;

mod fs;
pub use fs::GlobSource;

#[cfg(feature = "rust-embed")]
mod rust_embed;
#[cfg(feature = "rust-embed")]
pub use rust_embed::RustEmbedSource;

/// A source of migrations.
///
/// Implementors return the full set of migrations they know about. The order is
/// irrelevant: migrations are keyed and sorted by their number further down the
/// pipeline.
pub trait MigrationSource {
    /// Load all migrations from this source.
    fn load(&self) -> Result<Vec<Migration>, Error>;
}

/// A single migration as provided by a [`MigrationSource`].
///
/// A migration is identified by its `number`, carries a human readable `name`,
/// and holds the `sql` that is executed when it is applied. The SHA-512 hash of
/// the SQL is used to detect migrations that were modified after they had
/// already been applied.
#[derive(Clone, Debug)]
pub struct Migration {
    /// Sequential migration number parsed from the `<number>_<name>` file stem.
    pub number: i32,
    /// Human readable migration name parsed from the `<number>_<name>` file stem.
    pub name: String,
    /// The SQL executed when this migration is applied.
    pub sql: String,
    hash: Vec<u8>,
}

impl Migration {
    /// Build a migration from its parts.
    ///
    /// The hash is computed from the SQL bytes, so custom [`MigrationSource`]
    /// implementations only need to provide the number, name, and SQL.
    pub fn new(number: i32, name: impl Into<String>, sql: impl Into<String>) -> Self {
        let sql = sql.into();
        let hash = hash_sql(&sql);
        Self {
            number,
            name: name.into(),
            sql,
            hash,
        }
    }

    /// SHA-512 hash of the SQL bytes, used to detect migrations that were
    /// modified after they had been applied.
    pub fn hash(&self) -> &[u8] {
        &self.hash
    }

    /// Whether the migration must run outside a transaction: its first line
    /// is `-- tusker:no-transaction`. Needed for statements PostgreSQL refuses
    /// inside a transaction block, such as `CREATE INDEX CONCURRENTLY`.
    pub fn no_transaction(&self) -> bool {
        self.sql
            .lines()
            .next()
            .is_some_and(|line| line.trim() == NO_TRANSACTION_MARKER)
    }

    /// Build a migration from a `<number>_<name>` file stem and its SQL.
    pub(crate) fn from_stem(stem: &str, sql: String) -> Result<Self, Error> {
        let (number, name) = parse_stem(stem)?;
        Ok(Self::new(number, name, sql))
    }
}

/// First line of a migration that must run outside a transaction.
pub const NO_TRANSACTION_MARKER: &str = "-- tusker:no-transaction";

/// Compute the SHA-512 hash of the given SQL.
///
/// This matches the hash previously computed over the raw migration file bytes:
/// for valid UTF-8 files `sql.as_bytes()` equals the on-disk bytes, so hashes
/// stored in existing databases keep validating.
pub(crate) fn hash_sql(sql: &str) -> Vec<u8> {
    let mut hasher = Sha512::new();
    hasher.update(sql.as_bytes());
    hasher.finalize().to_vec()
}

/// Parse a `<number>_<name>` file stem into its number and name.
pub(crate) fn parse_stem(stem: &str) -> Result<(i32, String), Error> {
    let v: Vec<&str> = stem.splitn(2, '_').collect();
    let number = v
        .first()
        .ok_or_else(|| Error::Misc(format!("Expected format <number>_<name>: {:?}", stem)))?;
    let number = number
        .parse::<i32>()
        .map_err(|e| Error::Misc(format!("Invalid number in {:?}: {}", stem, e)))?;
    let name = v.get(1).unwrap_or(&"");
    Ok((number, String::from(*name)))
}

/// Collect migrations, rejecting duplicate migration numbers.
pub(crate) fn collect_unique(
    migrations: impl IntoIterator<Item = Result<Migration, Error>>,
) -> Result<Vec<Migration>, Error> {
    let mut result = Vec::new();
    let mut numbers = HashSet::new();
    for migration in migrations {
        let migration = migration?;
        if !numbers.insert(migration.number) {
            return Err(Error::Misc(format!(
                "Migration source contains multiple files for number {}",
                migration.number
            )));
        }
        result.push(migration);
    }
    Ok(result)
}
