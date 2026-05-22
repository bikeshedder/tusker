#![deny(nonstandard_style, rust_2018_idioms)]
#![forbid(non_ascii_idents, unsafe_code)]

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
