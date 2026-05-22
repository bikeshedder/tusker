#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(
    nonstandard_style,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
#![forbid(non_ascii_idents, unsafe_code)]
#![warn(
    deprecated_in_future,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]
#![allow(clippy::uninlined_format_args)]

use tokio_postgres::{types::ToSql, GenericClient, Row};

pub use tusker_query_derive::Query;

/// Marker traits and PostgreSQL type markers used by checked query validation.
pub mod types;

#[doc(hidden)]
pub mod __private {
    pub trait RowFieldCount<const N: usize> {}

    pub trait RowFieldType<const I: usize> {
        type Ty;
    }
}

/// A typed SQL query that can be executed through `tokio-postgres`.
pub trait Query: Sized {
    /// SQL text loaded from the query file.
    const SQL: &'static str;
    /// Row type returned by the query.
    type Row: FromRow;
    /// Query bind parameters in positional order.
    fn as_params(&self) -> Box<[&(dyn ToSql + Sync)]>;
}

/// Converts a `tokio-postgres` row into a strongly typed Rust value.
pub trait FromRow {
    /// Builds `Self` from a single database row.
    fn from_row(row: Row) -> Self;
}

pub use tusker_query_derive::FromRow;

impl FromRow for () {
    fn from_row(_: Row) -> Self {}
}

impl __private::RowFieldCount<0> for () {}

/// Executes a query that must return exactly one row.
pub async fn query_one<Q: Query>(
    client: &impl GenericClient,
    query: Q,
) -> Result<Q::Row, tokio_postgres::Error> {
    let stmt = client.prepare(Q::SQL).await?;
    Ok(Q::Row::from_row(
        client.query_one(&stmt, &query.as_params()).await?,
    ))
}

/// Executes a query and collects all returned rows.
pub async fn query<Q: Query>(
    client: &impl GenericClient,
    query: Q,
) -> Result<Vec<Q::Row>, tokio_postgres::Error> {
    let stmt = client.prepare(Q::SQL).await?;
    let rows = client.query(&stmt, &query.as_params()).await?;
    Ok(rows.into_iter().map(Q::Row::from_row).collect())
}
