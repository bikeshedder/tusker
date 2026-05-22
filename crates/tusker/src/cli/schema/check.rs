use std::process::exit;

use anyhow::Result;
use clap::Parser;

use crate::{config::Config, db::DiffDatabase};

use super::{diff::inspect_backend, Backend};

#[derive(Debug, Parser)]
pub struct CheckArgs {
    /// from-backend for the diff operation
    #[arg(default_value_t = Backend::Schema)]
    from: Backend,
    /// to-backend for the diff operation
    #[arg(default_value_t = Backend::Migrations)]
    to: Backend,
    /// swaps the "from" and "to" arguments creating a reverse diff
    #[arg(long, short)]
    reverse: bool,
    /// check privilege differences (ie. grant/revoke statements)
    #[arg(long, group = "group_privileges")]
    with_privileges: bool,
    /// don't check privilege differences
    #[arg(long, group = "group_privileges")]
    without_privileges: bool,
}

pub async fn cmd(cfg: &Config, args: &CheckArgs) -> Result<()> {
    let mut db = DiffDatabase::new(&cfg.database).await?;
    db.create().await?;
    let from = inspect_backend(cfg, &mut db, args.from).await?;
    let to = inspect_backend(cfg, &mut db, args.to).await?;
    db.drop().await?;
    if from == to {
        println!("Schemas are identical");
        Ok(())
    } else {
        println!("Schemas differ: {} != {}", args.from, args.to);
        println!("Run `tusker diff` to see the differences");
        exit(1);
    }
}
