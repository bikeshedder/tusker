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
    /// PostgreSQL parameter types in bind order.
    pub params: Vec<SqlType>,
    /// Result columns returned by the query.
    pub columns: Vec<Column>,
}

/// Offline metadata for a single result column.
#[derive(Debug, Serialize, Deserialize)]
pub struct Column {
    /// Column name as reported by PostgreSQL.
    pub name: String,
    /// PostgreSQL type for the column.
    pub r#type: SqlType,
    /// Nullability hint, when PostgreSQL could determine one.
    pub notnull: Option<bool>,
}

/// PostgreSQL type metadata used by checked query validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "SqlTypeRepr", into = "SqlTypeRepr")]
pub enum SqlType {
    /// A scalar type identified by schema and name.
    Scalar {
        /// PostgreSQL schema containing the type.
        schema: String,
        /// PostgreSQL type name.
        name: String,
    },
    /// A PostgreSQL array type.
    Array {
        /// Array element type.
        element: Box<SqlType>,
    },
    /// A PostgreSQL composite type.
    Composite {
        /// PostgreSQL schema containing the type.
        schema: String,
        /// PostgreSQL type name.
        name: String,
        /// Composite fields in PostgreSQL declaration order.
        fields: Vec<CompositeField>,
    },
    /// A PostgreSQL enum type.
    Enum {
        /// PostgreSQL schema containing the type.
        schema: String,
        /// PostgreSQL type name.
        name: String,
        /// Enum labels in PostgreSQL declaration order.
        variants: Vec<String>,
    },
}

impl SqlType {
    /// Creates scalar metadata.
    pub fn scalar(schema: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Scalar {
            schema: schema.into(),
            name: name.into(),
        }
    }

    /// Returns a human-readable PostgreSQL type name.
    pub fn display_name(&self) -> String {
        match self {
            Self::Scalar { schema, name }
            | Self::Composite { schema, name, .. }
            | Self::Enum { schema, name, .. } => {
                if schema == "public" || schema == "pg_catalog" || schema.is_empty() {
                    name.clone()
                } else {
                    format!("{schema}.{name}")
                }
            }
            Self::Array { element } => format!("{}[]", element.display_name()),
        }
    }
}

/// PostgreSQL composite field metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeField {
    /// Field name.
    pub name: String,
    /// Field type.
    pub r#type: SqlType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StructuredSqlType {
    Scalar {
        schema: String,
        name: String,
    },
    Array {
        element: Box<SqlType>,
    },
    Composite {
        schema: String,
        name: String,
        fields: Vec<CompositeField>,
    },
    Enum {
        schema: String,
        name: String,
        variants: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum SqlTypeRepr {
    Legacy(String),
    Structured(StructuredSqlType),
}

impl From<SqlTypeRepr> for SqlType {
    fn from(value: SqlTypeRepr) -> Self {
        match value {
            SqlTypeRepr::Legacy(name) => Self::Scalar {
                schema: String::new(),
                name,
            },
            SqlTypeRepr::Structured(StructuredSqlType::Scalar { schema, name }) => {
                Self::Scalar { schema, name }
            }
            SqlTypeRepr::Structured(StructuredSqlType::Array { element }) => {
                Self::Array { element }
            }
            SqlTypeRepr::Structured(StructuredSqlType::Composite {
                schema,
                name,
                fields,
            }) => Self::Composite {
                schema,
                name,
                fields,
            },
            SqlTypeRepr::Structured(StructuredSqlType::Enum {
                schema,
                name,
                variants,
            }) => Self::Enum {
                schema,
                name,
                variants,
            },
        }
    }
}

impl From<SqlType> for SqlTypeRepr {
    fn from(value: SqlType) -> Self {
        match value {
            SqlType::Scalar { name, .. } => Self::Legacy(name),
            SqlType::Array { element } => Self::Structured(StructuredSqlType::Array { element }),
            SqlType::Composite {
                schema,
                name,
                fields,
            } => Self::Structured(StructuredSqlType::Composite {
                schema,
                name,
                fields,
            }),
            SqlType::Enum {
                schema,
                name,
                variants,
            } => Self::Structured(StructuredSqlType::Enum {
                schema,
                name,
                variants,
            }),
        }
    }
}
