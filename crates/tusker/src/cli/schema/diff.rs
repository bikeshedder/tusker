use anyhow::Result;
use clap::Parser;
use tokio::{fs::File, io::AsyncReadExt};
use tusker_schema::{diff::DiffSql, models::schema::join_sql, Inspection};

use crate::{
    config::{Config, DatabaseConfig},
    db::DiffDatabase,
};

use super::Backend;

#[derive(Debug, Parser)]
pub struct DiffArgs {
    /// from-backend for the diff operation
    #[arg(default_value_t = Backend::Migrations)]
    from: Backend,
    /// to-backend for the diff operation
    #[arg(default_value_t = Backend::Schema)]
    to: Backend,
    #[arg(long, short)]
    reverse: bool,
    /// throw an exception if drop-statements are generated
    #[arg(long, group = "group_safe")]
    safe: bool,
    /// don't throw an exception if drop-statements are generated
    #[arg(long, group = "group_safe")]
    r#unsafe: bool,
    /// output privilege differences (ie. grant/revoke statements)
    #[arg(long, group = "group_privileges")]
    with_privileges: bool,
    /// don't output privilege differences
    #[arg(long, group = "group_privileges")]
    without_privileges: bool,
}

async fn inspect_sql(db: &DiffDatabase, filename: &str) -> Result<Inspection> {
    let mut client = db.connect().await?;
    let txn = client.transaction().await?;
    for filename in glob::glob(filename)? {
        let filename = filename?;
        let mut file = File::open(filename).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let sql = String::from_utf8(contents)?;
        // FIXME error handling
        txn.simple_query(&sql).await?;
    }
    let inspection = tusker_schema::inspect(txn.client()).await?;
    txn.rollback().await?;
    Ok(inspection)
}

async fn inspect_db(cfg: &DatabaseConfig) -> Result<Inspection> {
    let client = cfg.connect().await?;
    tusker_schema::inspect(&client).await
}

pub async fn inspect_backend(
    cfg: &Config,
    db: &mut DiffDatabase,
    backend: Backend,
) -> Result<Inspection> {
    match backend {
        Backend::Migrations => inspect_sql(db, &cfg.migrations.filename).await,
        Backend::Schema => inspect_sql(db, &cfg.schema.filename).await,
        Backend::Database => inspect_db(&cfg.database).await,
    }
}

pub async fn cmd(cfg: &Config, args: &DiffArgs) -> Result<()> {
    let (from, to) = if args.reverse {
        (args.to, args.from)
    } else {
        (args.from, args.to)
    };

    let mut db = DiffDatabase::new(&cfg.database).await?;
    db.create().await?;

    let from = inspect_backend(cfg, &mut db, from).await?;
    let to = inspect_backend(cfg, &mut db, to).await?;

    let diff = from.diff(&to);
    println!("{}", join_sql(diff.sql()));

    // XXX it would be nice if this was an actual drop guard
    db.drop().await?;

    Ok(())
}
