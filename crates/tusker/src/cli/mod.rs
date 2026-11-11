use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::Config;

mod clean;
mod config;
mod migration;
mod query;
mod schema;

#[derive(Debug, Parser)]
#[command(name = "tusker")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Schema commands (diffing)
    #[command(alias = "s")]
    Schema(schema::SchemaCommand),
    /// Remove all temporary databases, schemas and tables created by
    /// tusker
    Clean(clean::CleanArgs),
    /// Query commands
    #[command(alias = "q")]
    Query(query::QueryCommand),
    /// Configuration commands
    #[command(alias = "cfg")]
    Config(config::ConfigCommand),
    /// Migration commands
    #[command(aliases = ["m", "mig"])]
    Migration(migration::MigrationCommand),
    /// Alias for "schema diff"
    #[command(alias = "d")]
    Diff(schema::diff::DiffArgs),
    /// Alias for "schema check"
    #[command(alias = "chk")]
    Check(schema::check::CheckArgs),
    /// Alias for "migration run"
    Migrate(tusker_migration::cli::RunArgs),
}

pub(crate) async fn run(cfg: &Config) -> Result<()> {
    let args = Cli::parse();
    match &args.command {
        Commands::Schema(cmd_args) => {
            schema::cmd(cfg, cmd_args).await?;
        }
        Commands::Clean(cmd_args) => {
            clean::cmd(cfg, cmd_args).await?;
        }
        Commands::Query(args) => query::cmd(cfg, args).await?,
        Commands::Migration(args) => migration::cmd(cfg, args).await?,
        Commands::Config(args) => config::cmd(cfg, args).await?,
        Commands::Diff(args) => schema::diff::cmd(cfg, args).await?,
        Commands::Check(args) => schema::check::cmd(cfg, args).await?,
        Commands::Migrate(args) => {
            let source = tusker_migration::GlobSource::new(&cfg.migrations.filename);
            tusker_migration::cli::run(&(cfg.database.pg_config()?), &source, args).await?
        }
    }
    Ok(())
}
