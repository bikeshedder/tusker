use std::io::Write;

use clap::{Args, Subcommand};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use time::OffsetDateTime;
use tokio_postgres::Config;

use tokio_postgres::Client;

use crate::error::Error;
use crate::migrator::{self, FixAction, RunOptions};
use crate::models::MigrationStatus;
use crate::source::MigrationSource;

#[derive(Debug, Clone, Copy, Args)]
#[clap(about = "Manage database migrations")]
/// Top-level migration command used by embedding CLIs.
pub struct Command {
    /// The migration subcommand to run.
    #[clap(subcommand)]
    pub subcommand: Subcommands,
}

impl From<Subcommands> for Command {
    fn from(subcommand: Subcommands) -> Self {
        Self { subcommand }
    }
}

/// The migration subcommands. Public so that embedding CLIs can flatten them
/// into a subcommand enum of their own and add commands next to them.
#[derive(Debug, Clone, Copy, Subcommand)]
pub enum Subcommands {
    /// List migrations and show their current status.
    #[clap(aliases = &["ls", "list"], about = "List migrations and show their current status")]
    Status,

    /// Show the migration log.
    #[clap(aliases = &["history"], about = "Show migration log")]
    Log,

    /// Run migrations on the database.
    #[clap(aliases = &["apply"], about = "Run migrations on the database")]
    Run(RunArgs),

    /// Fail unless all migrations are cleanly applied.
    #[clap(about = "Fail unless all migrations are cleanly applied")]
    Check,

    /// Fix a database migration.
    #[clap(about = "Fix database migration")]
    Fix(FixArgs),
}

#[derive(Debug, Clone, Copy, Default, Args)]
/// Arguments for applying migrations.
pub struct RunArgs {
    /// Apply outstanding migrations only up to and including this number.
    #[clap(
        long,
        short,
        help = "Apply outstanding migrations only up to and including this number. Without it all outstanding migrations are applied."
    )]
    pub number: Option<i32>,
}

/// Arguments for fixing a migration.
#[derive(Debug, Clone, Copy, Args)]
pub struct FixArgs {
    /// Number of the migration to fix.
    #[clap(value_name = "NUMBER")]
    pub number: i32,
}

struct Colors {
    pub bold: ColorSpec,
    pub ok: ColorSpec,
    pub error: ColorSpec,
    pub new: ColorSpec,
    pub modified: ColorSpec,
}

impl Colors {
    fn new() -> Self {
        Self {
            bold: {
                let mut c = ColorSpec::new();
                let _ = c.set_bold(true);
                c
            },
            ok: {
                let mut c = ColorSpec::new();
                let _ = c.set_fg(Some(Color::Green));
                let _ = c.set_bold(true);
                c
            },
            error: {
                let mut c = ColorSpec::new();
                let _ = c.set_fg(Some(Color::Red));
                let _ = c.set_bold(true);
                c
            },
            new: {
                let mut c = ColorSpec::new();
                let _ = c.set_fg(Some(Color::Cyan));
                let _ = c.set_bold(true);
                c
            },
            modified: {
                let mut c = ColorSpec::new();
                let _ = c.set_fg(Some(Color::Yellow));
                let _ = c.set_bold(true);
                c
            },
        }
    }
}

async fn connect(pg_config: &Config) -> Result<Client, Error> {
    let (client, connection) = pg_config
        .connect(tokio_postgres::NoTls)
        .await
        .map_err(|e| Error::Pg("Unable to connect to database".into(), e))?;
    drop(tokio::spawn(connection));
    Ok(client)
}

/// Dispatches a migration subcommand using the given PostgreSQL configuration
/// and migration source.
///
/// This connects without TLS. To connect differently, use the functions of
/// [`crate::migrator`] with a client of your own.
pub async fn cmd(
    pg_config: &Config,
    source: &dyn MigrationSource,
    cmd: &Command,
) -> Result<(), Error> {
    match &cmd.subcommand {
        Subcommands::Status => status(pg_config, source).await?,
        Subcommands::Log => log(pg_config).await?,
        Subcommands::Check => check(pg_config, source).await?,
        Subcommands::Run(args) => run(pg_config, source, args).await?,
        Subcommands::Fix(args) => fix(pg_config, source, args).await?,
    }
    Ok(())
}

async fn status(pg_config: &Config, source: &dyn MigrationSource) -> Result<(), Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let colors = Colors::new();
    let client = connect(pg_config).await?;
    let states = migrator::status(&client, source).await?;
    if states.iter().all(|state| state.applied.is_none()) {
        writeln!(&mut stdout, "No migrations applied yet.")?;
    }
    writeln!(
        &mut stdout,
        "   # Name                                             Status"
    )?;
    writeln!(
        &mut stdout,
        "----------------------------------------------------------------------------"
    )?;
    for state in states.iter() {
        write!(&mut stdout, "{:04} ", state.number)?;
        stdout.set_color(&colors.bold)?;
        write!(&mut stdout, "{:48} ", state.name())?;
        match state.status() {
            MigrationStatus::Ok => {
                stdout.set_color(&colors.ok)?;
                write!(&mut stdout, "Ok")?;
            }
            MigrationStatus::Mismatch { name, hash } => {
                stdout.set_color(&colors.modified)?;
                write!(&mut stdout, "Mismatch:")?;
                if name {
                    write!(&mut stdout, " name")?;
                }
                if hash {
                    write!(&mut stdout, " hash")?;
                }
            }
            MigrationStatus::New => {
                stdout.set_color(&colors.new)?;
                write!(&mut stdout, "New")?;
            }
            MigrationStatus::FileMissing => {
                stdout.set_color(&colors.error)?;
                write!(&mut stdout, "Migration file missing")?;
            }
        }
        stdout.reset()?;
        writeln!(&mut stdout)?;
    }
    Ok(())
}

async fn log(pg_config: &Config) -> Result<(), Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let client = connect(pg_config).await?;
    let colors = Colors::new();
    let log = migrator::log(&client).await?;
    writeln!(
        &mut stdout,
        "Timestamp                  Operation    #    Name"
    )?;
    writeln!(
        &mut stdout,
        "----------------------------------------------------------------------------"
    )?;
    for log_entry in log {
        write!(stdout, "{} ", format_timestamp(log_entry.timestamp))?;
        match log_entry.operation.as_str() {
            "apply" => {
                stdout.set_color(&colors.ok)?;
            }
            "update" => {
                stdout.set_color(&colors.modified)?;
            }
            "delete" => {
                stdout.set_color(&colors.error)?;
            }
            "fake" => {
                stdout.set_color(&colors.modified)?;
            }
            _ => {}
        }
        write!(stdout, "{:10}", log_entry.operation)?;
        stdout.reset()?;
        write!(stdout, "{:04} ", log_entry.number)?;
        stdout.set_color(&colors.bold)?;
        writeln!(stdout, "{}", log_entry.name)?;
        stdout.reset()?;
    }
    Ok(())
}

/// Formats a timestamp as `YYYY-MM-DD HH:MM:SS +HH:MM`.
///
/// The `Display` implementation of `OffsetDateTime` prints sub second
/// precision, seconds of the UTC offset and varies in width causing the
/// table to be misaligned.
fn format_timestamp(timestamp: OffsetDateTime) -> String {
    let offset = timestamp.offset();
    let sign = if offset.is_negative() { '-' } else { '+' };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}{:02}:{:02}",
        timestamp.year(),
        u8::from(timestamp.month()),
        timestamp.day(),
        timestamp.hour(),
        timestamp.minute(),
        timestamp.second(),
        sign,
        offset.whole_hours().unsigned_abs(),
        offset.minutes_past_hour().unsigned_abs(),
    )
}

/// Prints whether all migrations are cleanly applied. Returns
/// [`Error::NotClean`] if not, so a binary exits with a failure.
async fn check(pg_config: &Config, source: &dyn MigrationSource) -> Result<(), Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let client = connect(pg_config).await?;
    let colors = Colors::new();
    match migrator::check(&client, source).await {
        Ok(()) => {
            stdout.set_color(&colors.ok)?;
            writeln!(stdout, "All migrations applied")?;
            stdout.reset()?;
            Ok(())
        }
        Err(Error::NotClean(states)) => {
            stdout.set_color(&colors.error)?;
            writeln!(
                stdout,
                "Not all migrations cleanly applied. See `status` for more details"
            )?;
            stdout.reset()?;
            Err(Error::NotClean(states))
        }
        Err(e) => Err(e),
    }
}

/// Applies all outstanding migrations in order, or those up to
/// [`RunArgs::number`].
pub async fn run(
    pg_config: &Config,
    source: &dyn MigrationSource,
    args: &RunArgs,
) -> Result<(), Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let mut client = connect(pg_config).await?;
    let colors = Colors::new();
    let mut options = RunOptions::new();
    if let Some(number) = args.number {
        options = options.up_to(number);
    }
    let applied = migrator::run(&mut client, source, &options).await?;
    for migration in &applied {
        write!(stdout, "Applied migration {}: ", migration.number)?;
        stdout.set_color(&colors.bold)?;
        write!(stdout, "{}", migration.name)?;
        stdout.reset()?;
        writeln!(stdout)?;
    }
    stdout.set_color(&colors.ok)?;
    if applied.is_empty() {
        writeln!(stdout, "Nothing to apply.")?;
    } else {
        writeln!(stdout, "Done.")?;
    }
    stdout.reset()?;
    Ok(())
}

async fn fix(
    pg_config: &Config,
    source: &dyn MigrationSource,
    args: &FixArgs,
) -> Result<(), Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let mut client = connect(pg_config).await?;
    let action = migrator::fix(&mut client, source, args.number).await?;
    writeln!(
        stdout,
        "Migration {}: {}",
        args.number,
        match action {
            FixAction::Updated => "name and hash updated",
            FixAction::Faked => "recorded as applied without running it",
            FixAction::Removed => "entry removed",
        }
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

    use super::format_timestamp;

    fn datetime(nanoseconds: u32, offset: UtcOffset) -> OffsetDateTime {
        Date::from_calendar_date(2026, Month::July, 7)
            .unwrap()
            .with_time(Time::from_hms_nano(8, 49, 5, nanoseconds).unwrap())
            .assume_offset(offset)
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(
            format_timestamp(datetime(27_663_000, UtcOffset::UTC)),
            "2026-07-07 08:49:05 +00:00"
        );
        assert_eq!(
            format_timestamp(datetime(0, UtcOffset::from_hms(-5, -30, 0).unwrap())),
            "2026-07-07 08:49:05 -05:30"
        );
    }
}
