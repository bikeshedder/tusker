use std::collections::BTreeMap;

use crate::db::DbMigration;
use crate::file::MigrationFile;

pub enum MigrationStatus<'a> {
    Ok(&'a MigrationFile, &'a DbMigration),
    Mismatch(&'a MigrationFile, &'a DbMigration),
    NotApplied(&'a MigrationFile),
    FileMissing(&'a DbMigration),
}

pub struct Migration {
    pub number: i32,
    pub file: Option<MigrationFile>,
    pub db: Option<DbMigration>,
}

impl Migration {
    pub fn get_status(&self) -> MigrationStatus<'_> {
        match (&self.file, &self.db) {
            (Some(file), Some(db)) => {
                if file.name == db.name && file.hash == db.hash {
                    MigrationStatus::Ok(file, db)
                } else {
                    MigrationStatus::Mismatch(file, db)
                }
            }
            (Some(file), None) => MigrationStatus::NotApplied(file),
            (None, Some(db)) => MigrationStatus::FileMissing(db),
            (None, None) => {
                panic!("Neither 'file' nor 'db' set. This should never happen.");
            }
        }
    }
}

pub fn combine_migrations(
    migration_files: &[MigrationFile],
    db_migrations: &[DbMigration],
) -> Vec<Migration> {
    let mut map: BTreeMap<i32, Migration> = BTreeMap::new();
    for migration_file in migration_files {
        map.insert(
            migration_file.number,
            Migration {
                number: migration_file.number,
                file: Some(migration_file.clone()),
                db: None,
            },
        );
    }
    for db_migration in db_migrations {
        if let Some(migration) = map.get_mut(&db_migration.number) {
            migration.db.replace(db_migration.clone());
        } else {
            map.insert(
                db_migration.number,
                Migration {
                    number: db_migration.number,
                    file: None,
                    db: Some(db_migration.clone()),
                },
            );
        }
    }
    map.into_values().collect()
}
