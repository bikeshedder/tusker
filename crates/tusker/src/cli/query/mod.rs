use std::{ffi::OsString, fs, io::ErrorKind, path::Path};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha512};
use tokio_postgres::{
    types::{Kind, Type},
    Client,
};
use tusker_query_models::{Column, CompositeField, SqlType, FORMAT_VERSION};

use crate::config::Config;

mod nullability;
#[cfg(test)]
mod tests;

use nullability::PlanRelations;

#[derive(Debug, Parser)]
pub(crate) struct QueryCommand {
    #[command(subcommand)]
    command: QuerySubcommand,
}
#[derive(Debug, Subcommand)]
enum QuerySubcommand {
    /// Inspect one query file and print inferred type metadata as JSON
    Inspect(QueryInspectArgs),
    /// Sync JSON sidecar files for a query glob or the configured query glob
    Sync(QuerySyncArgs),
}

#[derive(Debug, Parser)]
struct QueryInspectArgs {
    /// SQL file to inspect
    filename: OsString,
}

#[derive(Debug, Parser)]
struct QuerySyncArgs {
    /// Glob pattern of query files to sync. Defaults to queries.filename from config.
    path: Option<String>,
}

pub(crate) async fn cmd(cfg: &Config, args: &QueryCommand) -> Result<()> {
    match &args.command {
        QuerySubcommand::Inspect(args) => cmd_inspect(cfg, args).await?,
        QuerySubcommand::Sync(args) => cmd_sync(cfg, args).await?,
    }
    Ok(())
}

async fn cmd_inspect(cfg: &Config, args: &QueryInspectArgs) -> Result<()> {
    let client = cfg.database.connect().await?;
    let query = inspect_query_file(&client, Path::new(&args.filename)).await?;
    println!("{}", serde_json::to_string_pretty(&query)?);
    Ok(())
}

async fn cmd_sync(cfg: &Config, args: &QuerySyncArgs) -> Result<()> {
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
    let mut plan: Option<PlanRelations> = None;
    for c in stmt.columns() {
        let notnull = match (c.table_oid(), c.column_id()) {
            (Some(table_oid), Some(column_id)) => {
                let source = inspect_source_column(client, table_oid, column_id).await?;
                if !source.notnull {
                    Some(false)
                } else {
                    // The constraint of the table column only holds for the
                    // result if the query cannot NULL-extend the table's rows.
                    let plan = match &mut plan {
                        Some(plan) => plan,
                        None => plan.insert(PlanRelations::explain(client, sql).await?),
                    };
                    Some(plan.keeps_not_null(&source.relations))
                }
            }
            _ => None,
        };
        columns.push(Column {
            name: c.name().to_owned(),
            r#type: inspect_type(c.type_()),
            notnull,
        })
    }

    Ok(tusker_query_models::Query {
        version: FORMAT_VERSION,
        checksum: Vec::from_iter(digest),
        params: stmt.params().iter().map(inspect_type).collect(),
        columns,
    })
}

fn inspect_type(ty: &Type) -> SqlType {
    match ty.kind() {
        Kind::Array(element) => SqlType::Array {
            element: Box::new(inspect_type(element)),
        },
        Kind::Composite(fields) => SqlType::Composite {
            schema: ty.schema().to_owned(),
            name: ty.name().to_owned(),
            fields: fields
                .iter()
                .map(|field| CompositeField {
                    name: field.name().to_owned(),
                    r#type: inspect_type(field.type_()),
                })
                .collect(),
        },
        Kind::Enum(variants) => SqlType::Enum {
            schema: ty.schema().to_owned(),
            name: ty.name().to_owned(),
            variants: variants.clone(),
        },
        _ => SqlType::scalar(ty.schema(), ty.name()),
    }
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

/// The table column a result column is taken from.
struct SourceColumn {
    /// Whether the table column is declared `NOT NULL`.
    notnull: bool,
    /// Schema and name of the table and of all tables inheriting from it,
    /// including partitions.
    relations: Vec<(String, String)>,
}

async fn inspect_source_column(
    client: &Client,
    table_oid: u32,
    column_id: i16,
) -> Result<SourceColumn> {
    let row = client
        .query_one(
            "SELECT attnotnull FROM pg_catalog.pg_attribute WHERE attrelid=$1 AND attnum=$2",
            &[&table_oid, &column_id],
        )
        .await?;
    let notnull = row.get(0);
    let relations = client
        .query(
            "WITH RECURSIVE rel(oid) AS (\
                SELECT $1::oid \
                UNION \
                SELECT i.inhrelid FROM pg_catalog.pg_inherits i JOIN rel ON i.inhparent = rel.oid\
            ) \
            SELECT n.nspname::text, c.relname::text \
            FROM rel \
            JOIN pg_catalog.pg_class c ON c.oid = rel.oid \
            JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace",
            &[&table_oid],
        )
        .await?
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    Ok(SourceColumn { notnull, relations })
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
