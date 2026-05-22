use std::fmt;
use std::io;
use tokio_postgres::Error as PgError;

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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Error::Io(m, e) => format!("{}: {}", m, e),
                Error::Misc(m) => m.to_string(),
                Error::Pg(m, e) => format!("{}: {}", m, e),
                Error::Sql(m) => m.to_string(),
            }
        )
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io("I/O error".into(), error)
    }
}
