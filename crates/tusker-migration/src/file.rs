use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{read_dir, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha512};

use crate::error::Error;

#[derive(Clone)]
pub struct MigrationFile {
    pub path: PathBuf,
    pub number: i32,
    pub name: String,
    pub hash: Vec<u8>,
}

impl MigrationFile {
    pub fn from_path(path: &Path) -> Result<MigrationFile, Error> {
        let file_stem = path
            .file_stem()
            .map(|name| name.to_string_lossy().into())
            .ok_or_else(|| Error::Misc(format!("Invalid filename: {}", path.display())))?;
        let (number, name) = parse_filename(file_stem)
            .map_err(|m| Error::Misc(format!("Invalid filename {:?}: {}", path.display(), m)))?;
        Ok(MigrationFile {
            path: PathBuf::from(&path),
            number,
            name,
            hash: calculate_hash(path)?,
        })
    }
    pub fn open(&self) -> io::Result<File> {
        File::open(&self.path)
    }
    pub fn read(&self) -> io::Result<String> {
        let mut sql = String::new();
        self.open()?.read_to_string(&mut sql)?;
        Ok(sql)
    }
}

fn parse_filename(filename: String) -> Result<(i32, String), String> {
    let v: Vec<&str> = filename.splitn(2, '_').collect();
    let number = v
        .first()
        .ok_or_else(|| String::from("Expected format <number>_<name>"))?;
    let number = number
        .parse::<i32>()
        .map_err(|e| format!("Invalid number: {}", e))?;
    let name = v.get(1).unwrap_or(&"");
    Ok((number, String::from(*name)))
}

fn calculate_hash(path: &Path) -> Result<Vec<u8>, Error> {
    let mut file = File::open(path)
        .map_err(|e| Error::Io(format!("Error opening SQL file {:?}", path.display()), e))?;
    let mut hasher = Sha512::new();
    let mut buf = [0u8; 512];
    loop {
        let count = file
            .read(&mut buf)
            .map_err(|e| Error::Io(format!("Error reading SQL file {:?}", path.display()), e))?;
        if count == 0 {
            break;
        }
        hasher.update(&buf[..count]);
    }
    Ok(hasher.finalize().to_vec())
}

pub fn load_migration_files(path: &Path) -> Result<Vec<MigrationFile>, Error> {
    let mut migrations: Vec<MigrationFile> = Vec::new();
    let mut number_set: HashSet<i32> = HashSet::new();
    let dir_entries = read_dir(path).map_err(|e| {
        Error::Io(
            format!("Unable to read migrations directory {:?}", path.display()),
            e,
        )
    })?;
    for entry in dir_entries {
        let entry =
            entry.map_err(|e| Error::Io("Error while reading directory entry".into(), e))?;
        if entry.path().extension() != Some(OsStr::new("sql")) {
            // skip files with an other extension than .sql
            continue;
        }
        let migration_file = MigrationFile::from_path(&entry.path())?;
        if number_set.contains(&migration_file.number) {
            return Err(Error::Misc(format!(
                "Migration folder contains multiple files for number {}",
                migration_file.number
            )));
        }
        number_set.insert(migration_file.number);
        migrations.push(migration_file);
    }
    Ok(migrations)
}
