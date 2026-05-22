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

use std::collections::HashMap;

use anyhow::Result;
use diff::{diff, Diff};
use itertools::Itertools;
use models::{
    constraint::Constraint, domain::Domain, extension::Extension, r#enum::Enum, routine::Routine,
    schema::Schema, sequence::Sequence, table::Table, trigger::Trigger, view::View,
};
use queries::Relkind;
use tokio_postgres::Client;

use crate::models::constraint::ConstraintType;

/// Diff primitives and SQL emission helpers used by the schema engine.
pub mod diff;
/// PostgreSQL schema model types used for inspection and comparison.
pub mod models;
pub(crate) mod queries;
pub(crate) mod sql;

#[derive(Debug, Eq, PartialEq)]
/// In-memory representation of inspected PostgreSQL schemas.
pub struct Inspection {
    /// Schemas keyed by schema name.
    pub schemas: HashMap<String, Schema>,
}

impl Inspection {
    /// Creates an empty inspection result.
    pub fn empty() -> Self {
        Self {
            schemas: Default::default(),
        }
    }
    /// Computes the schema-level diff against another inspection result.
    pub fn diff<'a>(&'a self, other: &'a Self) -> Diff<'a, Schema> {
        diff(
            self.schemas.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            other.schemas.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            |schema| &schema.name,
        )
    }
}

/// Inspects the connected PostgreSQL database and returns its schema state.
pub async fn inspect(client: &Client) -> Result<Inspection> {
    let mut schemas: HashMap<String, Schema> = HashMap::new();
    let rows = tusker_query::query(client, queries::Schemas {}).await?;
    for schema in rows {
        let mut schema = Schema::new(&schema.name);
        // Enums
        let rows = tusker_query::query(
            client,
            queries::Enums {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let e = Enum::from(row);
            let _ = schema.enums.insert(e.name.clone(), e);
        }
        // Domains
        let rows = tusker_query::query(
            client,
            queries::Domains {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let domain = Domain::from(row);
            let _ = schema.domains.insert(domain.name.clone(), domain);
        }
        // Sequences
        let rows = tusker_query::query(
            client,
            queries::Sequences {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let sequence = Sequence::from(row);
            let _ = schema.sequences.insert(sequence.name.clone(), sequence);
        }
        // Extensions
        let rows = tusker_query::query(
            client,
            queries::Extensions {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let extension = Extension::from(row);
            let _ = schema.extensions.insert(extension.name.clone(), extension);
        }
        // Indexes
        let rows = tusker_query::query(
            client,
            queries::Indexes {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let index = models::index::Index::from(row);
            let _ = schema.indexes.insert(index.name.clone(), index);
        }
        // Tables
        let rows = tusker_query::query(
            client,
            queries::Classes {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for cls in rows {
            match cls.relkind {
                Relkind::OrdinaryTable => {
                    let _ = schema
                        .tables
                        .insert(cls.name.clone(), Table::try_from(cls)?);
                }
                Relkind::Index => {}
                Relkind::Sequence => {}
                Relkind::ToastTable => {}
                Relkind::View => {
                    let _ = schema.views.insert(cls.name.clone(), View::try_from(cls)?);
                }
                Relkind::MaterializedView => {
                    let _ = schema.views.insert(cls.name.clone(), View::try_from(cls)?);
                }
                Relkind::CompositeType => {}
                Relkind::ForeignTable => {}
                Relkind::PartitionedTable => {}
                Relkind::PartitionedIndex => {}
            };
        }
        // Constraints
        let rows = tusker_query::query(
            client,
            queries::Constraints {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let constraint = Constraint {
                schema: schema.name.clone(),
                table: row.table,
                name: row.name,
                r#type: row.r#type,
                definition: row.def,
            };
            if constraint.r#type == ConstraintType::NotNull {
                // Skip NOT NULL constraints introduced in PostgreSQL 18
                // It might be useful to support named not null constraints
                // in the future. That's why this is not filtered as part of
                // the query but here in the code.
                continue;
            }
            let _ = schema.constraints.insert(
                (constraint.table.clone(), constraint.name.clone()),
                constraint,
            );
        }
        // Routines
        let rows = tusker_query::query(
            client,
            queries::Routines {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let routine = Routine::from(row);
            let _ = schema.routines.insert(
                (routine.name.clone(), routine.identity_arguments.clone()),
                routine,
            );
        }
        // Triggers
        let rows = tusker_query::query(
            client,
            queries::Triggers {
                schema: schema.name.clone(),
            },
        )
        .await?;
        for row in rows {
            let trigger = Trigger::from(row);
            let _ = schema
                .triggers
                .insert((trigger.table_name.clone(), trigger.name.clone()), trigger);
        }
        let _ = schemas.insert(schema.name.clone(), schema);
    }

    Ok(Inspection { schemas })
}
