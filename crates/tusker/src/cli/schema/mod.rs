use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::Config;

use self::{check::CheckArgs, diff::DiffArgs};

pub mod check;
pub mod diff;

#[derive(Debug, Parser)]
pub struct SchemaCommand {
    #[command(subcommand)]
    pub command: SchemaSubcommand,
}
#[derive(Debug, Subcommand)]
pub enum SchemaSubcommand {
    /// Show differences between two schemas
    ///
    /// This command calculates the difference between two database schemas.
    /// The from- and to-parameter accept one of the following backends:
    /// migrations, schema, database
    #[command(alias = "d")]
    Diff(DiffArgs),
    /// Check for differences between schemas
    ///
    /// This command checks for differences between two or more schemas.
    /// Exit code 0 means that the schemas are all in sync. Otherwise
    /// the exit code 1 is used. This is useful for continuous integration
    /// checks.
    #[command(alias = "chk")]
    Check(CheckArgs),
}

pub async fn cmd(cfg: &Config, args: &SchemaCommand) -> Result<()> {
    match &args.command {
        SchemaSubcommand::Diff(cmd_args) => diff::cmd(cfg, cmd_args).await?,
        SchemaSubcommand::Check(cmd_args) => check::cmd(cfg, cmd_args).await?,
    }
    Ok(())
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Backend {
    Migrations,
    Schema,
    Database,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}
