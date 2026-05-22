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

pub mod diff;
pub mod models;
pub mod queries;
pub(crate) mod sql;

#[derive(Debug, Eq, PartialEq)]
pub struct Inspection {
    pub schemas: HashMap<String, Schema>,
}

impl Inspection {
    pub fn empty() -> Self {
        Self {
            schemas: Default::default(),
        }
    }
    pub fn diff<'a>(&'a self, other: &'a Self) -> Diff<'a, Schema> {
        diff(
            self.schemas.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            other.schemas.values().sorted_by(|a, b| a.name.cmp(&b.name)),
            |schema| &schema.name,
        )
    }
}

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
            schema.enums.insert(e.name.clone(), e);
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
            schema.domains.insert(domain.name.clone(), domain);
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
            schema.sequences.insert(sequence.name.clone(), sequence);
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
            schema.extensions.insert(extension.name.clone(), extension);
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
            schema.indexes.insert(index.name.clone(), index);
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
                    schema
                        .tables
                        .insert(cls.name.clone(), Table::try_from(cls)?);
                }
                Relkind::Index => {}
                Relkind::Sequence => {}
                Relkind::ToastTable => {}
                Relkind::View => {
                    schema.views.insert(cls.name.clone(), View::try_from(cls)?);
                }
                Relkind::MaterializedView => {
                    schema.views.insert(cls.name.clone(), View::try_from(cls)?);
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
            schema.constraints.insert(
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
            schema.routines.insert(
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
            schema
                .triggers
                .insert((trigger.table_name.clone(), trigger.name.clone()), trigger);
        }
        schemas.insert(schema.name.clone(), schema);
    }

    Ok(Inspection { schemas })
}
