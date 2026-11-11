use std::process::exit;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{config::Config, vendor};

#[derive(Debug, Args)]
#[command(about = "Manage database migrations")]
pub(crate) struct MigrationCommand {
    #[command(subcommand)]
    command: MigrationSubcommand,
}

#[derive(Debug, Subcommand)]
enum MigrationSubcommand {
    /// Copy the migrations of dependencies ([vendor.<crate>] in tusker.toml)
    /// that are not in the migration folder yet to its end
    Vendor(VendorArgs),
    #[command(flatten)]
    Runner(tusker_migration::cli::Subcommands),
}

#[derive(Debug, Clone, Copy, Args)]
struct VendorArgs {
    /// Only report: exit with status 1 when migrations are not vendored yet
    /// or vendored migrations do not match their dependency
    #[arg(long)]
    check: bool,
}

pub(crate) async fn cmd(cfg: &Config, args: &MigrationCommand) -> Result<()> {
    match &args.command {
        MigrationSubcommand::Vendor(args) => cmd_vendor(cfg, args),
        MigrationSubcommand::Runner(subcommand) => {
            let source = tusker_migration::GlobSource::new(&cfg.migrations.filename);
            let command = tusker_migration::cli::Command {
                subcommand: *subcommand,
            };
            tusker_migration::cli::cmd(&(cfg.database.pg_config()?), &source, &command).await?;
            Ok(())
        }
    }
}

fn cmd_vendor(cfg: &Config, args: &VendorArgs) -> Result<()> {
    let vendored = vendor::load(&cfg.vendor, &std::env::current_dir()?)?;
    let project = vendor::load_project_migrations(&cfg.migrations.filename)?;
    let report = vendored.analyze(&project)?;
    if !report.problems.is_empty() {
        report.print();
        exit(1);
    }
    if report.pending.is_empty() {
        if cfg.vendor.is_empty() {
            println!("tusker.toml has no [vendor.<crate>] section, so there is nothing to vendor");
        } else {
            println!("All migrations of the dependencies are vendored");
        }
        return Ok(());
    }
    if args.check {
        report.print();
        exit(1);
    }
    let dir = vendor::glob_base(&cfg.migrations.filename);
    for path in vendor::write_pending(&report, &project, &dir)? {
        println!("Vendored {}", path.display());
    }
    Ok(())
}
