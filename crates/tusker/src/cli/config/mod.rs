use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::Config;

#[derive(Debug, Parser)]
pub(crate) struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}
#[derive(Debug, Subcommand)]
enum ConfigSubcommand {
    Default,
    Template,
    Show,
}

pub(crate) async fn cmd(cfg: &Config, args: &ConfigCommand) -> Result<()> {
    match args.command {
        ConfigSubcommand::Default => cmd_default().await?,
        ConfigSubcommand::Template => cmd_template().await?,
        ConfigSubcommand::Show => cmd_show(cfg).await?,
    }
    Ok(())
}

fn print_config(cfg: &Config) -> Result<()> {
    let content = toml::to_string_pretty(&cfg)?;
    println!("{}", content);
    Ok(())
}

async fn cmd_default() -> Result<()> {
    print_config(&Config::default())
}

async fn cmd_template() -> Result<()> {
    print_config(&Config::template())
}

async fn cmd_show(cfg: &Config) -> Result<()> {
    print_config(cfg)
}
