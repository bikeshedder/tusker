use anyhow::Result;
use config::Config;

pub mod cli;
pub mod config;
pub mod db;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::new()?;
    cli::run(&cfg).await?;
    Ok(())
}
