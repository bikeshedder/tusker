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

use serde::{Deserialize, Serialize};

/// Offline metadata for a checked SQL query.
#[derive(Debug, Serialize, Deserialize)]
pub struct Query {
    #[serde(
        serialize_with = "hex::serde::serialize",
        deserialize_with = "hex::serde::deserialize"
    )]
    /// SHA-512 digest of the SQL file contents.
    pub checksum: Vec<u8>,
    /// PostgreSQL parameter type names in bind order.
    pub params: Vec<String>,
    /// Result columns returned by the query.
    pub columns: Vec<Column>,
}

/// Offline metadata for a single result column.
#[derive(Debug, Serialize, Deserialize)]
pub struct Column {
    /// Column name as reported by PostgreSQL.
    pub name: String,
    /// PostgreSQL type name for the column.
    pub r#type: String,
    /// Nullability hint, when PostgreSQL could determine one.
    pub notnull: Option<bool>,
}
