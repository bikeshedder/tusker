use anyhow::Result;
use clap::Parser;

use crate::{config::Config, db::DiffDatabase};

#[derive(Debug, Parser)]
pub struct CleanArgs {
    /// Don't actually perform the clean up operation but rather show
    /// which queries need to be executed.
    #[clap(long)]
    dry_run: bool,
}

pub async fn cmd(cfg: &Config, args: &CleanArgs) -> Result<()> {
    let db = DiffDatabase::new(&cfg.database).await?;
    for dbname in db.leftover_database().await? {
        println!("Dropping {} ...", dbname);
        if !args.dry_run {
            db.drop_dbname(&dbname).await?;
        }
    }
    Ok(())
}
