use std::process::exit;

use anyhow::Result;
use clap::Parser;

use crate::{config::Config, db::DiffDatabase};

use super::{
    diff::{inspect_backend, require_vendored},
    Backend,
};

#[derive(Copy, Clone, Debug, Parser)]
pub(crate) struct CheckArgs {
    // The default direction mirrors `tusker diff` (migrations -> schema) so that
    // `check` and `diff` evaluate the exact same change. This matters once the
    // outcome becomes direction-dependent, e.g. under
    // `[diff] removed_enum_value = "ignore"` a value removed on the way to the
    // target schema is not a difference, while adding one still is.
    /// from-backend for the diff operation
    #[arg(default_value_t = Backend::Migrations)]
    from: Backend,
    /// to-backend for the diff operation
    #[arg(default_value_t = Backend::Schema)]
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

pub(crate) async fn cmd(cfg: &Config, args: &CheckArgs) -> Result<()> {
    require_vendored(cfg, args.from, args.to)?;
    let mut db = DiffDatabase::new(&cfg.database).await?;
    db.create().await?;
    let from = inspect_backend(cfg, &mut db, args.from).await?;
    let to = inspect_backend(cfg, &mut db, args.to).await?;
    db.drop().await?;
    if from.equivalent(&to, &cfg.diff.options()) {
        println!("Schemas are identical");
        Ok(())
    } else {
        println!("Schemas differ: {} != {}", args.from, args.to);
        println!("Run `tusker diff` to see the differences");
        exit(1);
    }
}
