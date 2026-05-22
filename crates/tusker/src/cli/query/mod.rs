use std::{ffi::OsString, fs, io::ErrorKind, path::Path};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha512};
use tokio_postgres::Client;
use tusker_query_models::Column;

use crate::config::Config;

#[derive(Debug, Parser)]
pub struct QueryCommand {
    #[command(subcommand)]
    command: QuerySubcommand,
}
#[derive(Debug, Subcommand)]
pub enum QuerySubcommand {
    /// Inspect one query file and print inferred type metadata as JSON
    Inspect(QueryInspectArgs),
    /// Sync JSON sidecar files for a query glob or the configured query glob
    Sync(QuerySyncArgs),
}

#[derive(Debug, Parser)]
pub struct QueryInspectArgs {
    /// SQL file to inspect
    filename: OsString,
}

#[derive(Debug, Parser)]
pub struct QuerySyncArgs {
    /// Glob pattern of query files to sync. Defaults to queries.filename from config.
    path: Option<String>,
}

pub async fn cmd(cfg: &Config, args: &QueryCommand) -> Result<()> {
    match &args.command {
        QuerySubcommand::Inspect(args) => cmd_inspect(cfg, args).await?,
        QuerySubcommand::Sync(args) => cmd_sync(cfg, args).await?,
    }
    Ok(())
}

pub async fn cmd_inspect(cfg: &Config, args: &QueryInspectArgs) -> Result<()> {
    let client = cfg.database.connect().await?;
    let query = inspect_query_file(&client, Path::new(&args.filename)).await?;
    println!("{}", serde_json::to_string_pretty(&query)?);
    Ok(())
}

pub async fn cmd_sync(cfg: &Config, args: &QuerySyncArgs) -> Result<()> {
    let client = cfg.database.connect().await?;
    let pattern = args.path.as_deref().unwrap_or(&cfg.queries.filename);
    let query_files = glob::glob(pattern)
        .with_context(|| format!("Invalid query glob: {pattern}"))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for filename in query_files {
        let output = filename.with_extension("json");
        let query = inspect_query_file(&client, &filename).await?;
        let status = write_query_types(&output, &query)?;
        println!("{} {}", status.label(), output.display());
    }
    Ok(())
}

async fn inspect_query_file(
    client: &Client,
    filename: &Path,
) -> Result<tusker_query_models::Query> {
    let content = fs::read(filename)
        .with_context(|| format!("Failed to read query file {}", filename.display()))?;
    let sql = String::from_utf8(content)
        .with_context(|| format!("Query file is not valid UTF-8: {}", filename.display()))?;
    inspect_query_sql(client, &sql)
        .await
        .with_context(|| format!("Failed to inspect query {}", filename.display()))
}

async fn inspect_query_sql(client: &Client, sql: &str) -> Result<tusker_query_models::Query> {
    let mut hasher = Sha512::new();
    hasher.update(sql);
    let digest = hasher.finalize();

    let stmt = client.prepare(sql).await?;

    let mut columns: Vec<Column> = Vec::new();
    for c in stmt.columns() {
        columns.push(Column {
            name: c.name().to_owned(),
            r#type: c.type_().to_string(),
            notnull: if let (Some(table_oid), Some(column_id)) = (c.table_oid(), c.column_id()) {
                Some(is_nullable(client, table_oid, column_id).await?)
            } else {
                None
            },
        })
    }

    Ok(tusker_query_models::Query {
        checksum: Vec::from_iter(digest),
        params: stmt.params().iter().map(|p| p.name().to_owned()).collect(),
        columns,
    })
}

fn write_query_types(output: &Path, query: &tusker_query_models::Query) -> Result<WriteStatus> {
    let rendered = serde_json::to_string_pretty(&query)?;

    let status = match fs::read_to_string(output) {
        Ok(existing) if existing == rendered => WriteStatus::Unchanged,
        Ok(_) => WriteStatus::Updated,
        Err(err) if err.kind() == ErrorKind::NotFound => WriteStatus::Created,
        Err(err) => {
            return Err(err).with_context(|| format!("Failed to read {}", output.display()))
        }
    };

    if !matches!(status, WriteStatus::Unchanged) {
        fs::write(output, rendered)
            .with_context(|| format!("Failed to write {}", output.display()))?;
    }

    Ok(status)
}

async fn is_nullable(client: &Client, table_id: u32, column_id: i16) -> Result<bool> {
    let stmt = client
        .prepare("SELECT attnotnull FROM pg_catalog.pg_attribute WHERE attrelid=$1 AND attnum=$2")
        .await?;
    let row = client.query_one(&stmt, &[&table_id, &column_id]).await?;
    Ok(row.get(0))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum WriteStatus {
    Created,
    Updated,
    Unchanged,
}

impl WriteStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Unchanged => "unchanged",
        }
    }
}
