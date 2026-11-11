use std::fmt;
use std::io;
use tokio_postgres::Error as PgError;

use crate::models::MigrationState;

#[derive(Debug)]
/// Errors returned by migration file loading, CLI execution, or PostgreSQL access.
pub enum Error {
    /// I/O failure while reading migration files or directories.
    Io(String, io::Error),
    /// Generic user-facing migration error.
    Misc(String),
    /// PostgreSQL failure while querying or mutating migration state.
    Pg(String, PgError),
    /// SQL text annotated with a database error location.
    Sql(String),
    /// The source and the database are not in sync: these migrations are
    /// not [`MigrationStatus::Ok`](crate::MigrationStatus::Ok).
    NotClean(Vec<MigrationState>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(m, e) => write!(f, "{}: {}", m, e),
            Error::Misc(m) => write!(f, "{}", m),
            Error::Pg(m, e) => write!(f, "{}: {}", m, e),
            Error::Sql(m) => write!(f, "{}", m),
            Error::NotClean(states) => {
                write!(f, "Not all migrations cleanly applied: ")?;
                for (i, state) in states.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(
                        f,
                        "{:04}_{} ({})",
                        state.number,
                        state.name(),
                        state.status()
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(_, e) => Some(e),
            Error::Pg(_, e) => Some(e),
            _ => None,
        }
    }
}

impl Error {
    /// The migrations that are not in sync, if this is [`Error::NotClean`].
    pub fn not_clean(&self) -> Option<&[MigrationState]> {
        match self {
            Error::NotClean(states) => Some(states),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io("I/O error".into(), error)
    }
}
