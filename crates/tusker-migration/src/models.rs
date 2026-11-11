use std::collections::BTreeMap;
use std::fmt;

use time::OffsetDateTime;

use crate::source::Migration;

/// A migration as currently recorded in the database's migration log.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AppliedMigration {
    /// The migration number.
    pub number: i32,
    /// The migration name recorded when it was applied, updated or faked.
    pub name: String,
    /// SHA-512 hash of the SQL recorded when it was applied, updated or faked.
    pub hash: Vec<u8>,
}

/// One entry of the migration log, as returned by [`crate::migrator::log`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MigrationLogEntry {
    /// The migration number.
    pub number: i32,
    /// The migration name.
    pub name: String,
    /// When the operation took place.
    pub timestamp: OffsetDateTime,
    /// The operation: `apply`, `fake`, `update` or `delete`.
    pub operation: String,
}

/// How a migration from the source compares with the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationStatus {
    /// The migration has been applied and name and hash match.
    Ok,
    /// The migration has been applied, but the name and/or the hash differ.
    Mismatch {
        /// The name differs.
        name: bool,
        /// The hash, i.e. the SQL, differs.
        hash: bool,
    },
    /// The migration exists in the source but has not been applied yet.
    New,
    /// The database knows the migration, but the source has no file for it.
    FileMissing,
}

impl fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationStatus::Ok => write!(f, "ok"),
            MigrationStatus::Mismatch { name, hash } => {
                write!(f, "mismatch:")?;
                if *name {
                    write!(f, " name")?;
                }
                if *hash {
                    write!(f, " hash")?;
                }
                Ok(())
            }
            MigrationStatus::New => write!(f, "new"),
            MigrationStatus::FileMissing => write!(f, "file missing"),
        }
    }
}

/// A migration number with its source file and its database entry, if any.
///
/// At least one of [`migration`](Self::migration) and
/// [`applied`](Self::applied) is always set.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MigrationState {
    /// The migration number.
    pub number: i32,
    /// The migration from the source, if the source has one.
    pub migration: Option<Migration>,
    /// The database entry, if the migration has been applied (or faked).
    pub applied: Option<AppliedMigration>,
}

impl MigrationState {
    /// Compares the source with the database.
    pub fn status(&self) -> MigrationStatus {
        match (&self.migration, &self.applied) {
            (Some(migration), Some(applied)) => {
                let name = migration.name != applied.name;
                let hash = migration.hash() != applied.hash.as_slice();
                if name || hash {
                    MigrationStatus::Mismatch { name, hash }
                } else {
                    MigrationStatus::Ok
                }
            }
            (Some(_), None) => MigrationStatus::New,
            (None, Some(_)) => MigrationStatus::FileMissing,
            (None, None) => unreachable!("MigrationState without migration and database entry"),
        }
    }

    /// The migration name: from the source if present, otherwise from the
    /// database.
    pub fn name(&self) -> &str {
        match (&self.migration, &self.applied) {
            (Some(migration), _) => &migration.name,
            (None, Some(applied)) => &applied.name,
            (None, None) => "",
        }
    }
}

pub(crate) fn combine_migrations(
    migrations: &[Migration],
    applied_migrations: &[AppliedMigration],
) -> Vec<MigrationState> {
    let mut map: BTreeMap<i32, MigrationState> = BTreeMap::new();
    for migration in migrations {
        let _ = map.insert(
            migration.number,
            MigrationState {
                number: migration.number,
                migration: Some(migration.clone()),
                applied: None,
            },
        );
    }
    for applied in applied_migrations {
        map.entry(applied.number)
            .or_insert_with(|| MigrationState {
                number: applied.number,
                migration: None,
                applied: None,
            })
            .applied = Some(applied.clone());
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(number: i32, name: &str, sql: &str) -> AppliedMigration {
        AppliedMigration {
            number,
            name: name.into(),
            hash: Migration::new(number, name, sql).hash().to_vec(),
        }
    }

    #[test]
    fn combine_sorts_by_number_and_reports_status() {
        let source = vec![
            Migration::new(3, "c", "SELECT 3;"),
            Migration::new(1, "a", "SELECT 1;"),
            Migration::new(2, "b", "SELECT 2;"),
        ];
        let db = vec![
            applied(1, "a", "SELECT 1;"),
            applied(2, "b", "SELECT 2 -- changed;"),
            applied(4, "d", "SELECT 4;"),
        ];
        let states = combine_migrations(&source, &db);
        let summary: Vec<_> = states.iter().map(|s| (s.number, s.status())).collect();
        assert_eq!(
            summary,
            vec![
                (1, MigrationStatus::Ok),
                (
                    2,
                    MigrationStatus::Mismatch {
                        name: false,
                        hash: true
                    }
                ),
                (3, MigrationStatus::New),
                (4, MigrationStatus::FileMissing),
            ]
        );
        assert_eq!(states[3].name(), "d");
    }

    #[test]
    fn renamed_migration_is_a_name_mismatch() {
        let states = combine_migrations(
            &[Migration::new(1, "new_name", "SELECT 1;")],
            &[applied(1, "old_name", "SELECT 1;")],
        );
        assert_eq!(
            states[0].status(),
            MigrationStatus::Mismatch {
                name: true,
                hash: false
            }
        );
    }

    #[test]
    fn status_display() {
        assert_eq!(
            MigrationStatus::Mismatch {
                name: true,
                hash: true
            }
            .to_string(),
            "mismatch: name hash"
        );
        assert_eq!(MigrationStatus::FileMissing.to_string(), "file missing");
    }
}
