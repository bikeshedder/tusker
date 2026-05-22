use std::fmt;

use postgres_types::FromSql;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_postgres::types::Json;
use tusker_query::{FromRow, Query};

use crate::models::{column::Column, constraint::ConstraintType};

#[derive(Copy, Clone, Debug, Default, Query)]
#[query(sql="schemas", row=Schema)]
pub(crate) struct Schemas {}

#[derive(Debug, FromRow)]
pub(crate) struct Schema {
    pub(crate) name: String,
}

#[derive(Debug, Query)]
#[query(sql="classes", row=Class)]
pub(crate) struct Classes {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct Class {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) relkind: Relkind,
    pub(crate) columns: Json<Vec<Column>>,
    pub(crate) viewdef: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum Relkind {
    #[serde(rename = "r")]
    OrdinaryTable,
    #[serde(rename = "i")]
    Index,
    #[serde(rename = "S")]
    Sequence,
    #[serde(rename = "t")]
    ToastTable,
    #[serde(rename = "v")]
    View,
    #[serde(rename = "m")]
    MaterializedView,
    #[serde(rename = "c")]
    CompositeType,
    #[serde(rename = "f")]
    ForeignTable,
    #[serde(rename = "p")]
    PartitionedTable,
    #[serde(rename = "I")]
    PartitionedIndex,
}

impl fmt::Display for Relkind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

impl FromSql<'_> for Relkind {
    fn from_sql(
        _ty: &postgres_types::Type,
        raw: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(match raw {
            b"r" => Self::OrdinaryTable,
            b"i" => Self::Index,
            b"S" => Self::Sequence,
            b"t" => Self::ToastTable,
            b"v" => Self::View,
            b"m" => Self::MaterializedView,
            b"c" => Self::CompositeType,
            b"p" => Self::PartitionedTable,
            b"I" => Self::PartitionedIndex,
            x => Err(UnsupportedRelkind(x.to_owned()))?,
        })
    }
    fn accepts(ty: &postgres_types::Type) -> bool {
        *ty == postgres_types::Type::CHAR
    }
}

#[derive(Error, Debug)]
#[error("Unsupported relkind value")]
struct UnsupportedRelkind(Vec<u8>);

#[derive(Debug, Query)]
#[query(sql="constraints", row=Constraint)]
pub(crate) struct Constraints {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct Constraint {
    pub(crate) table: String,
    pub(crate) name: String,
    pub(crate) r#type: ConstraintType,
    pub(crate) def: String,
}

#[derive(Debug, Query)]
#[query(sql = "routines", row = RoutineRow)]
pub(crate) struct Routines {
    pub(crate) schema: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum RoutineKind {
    Function,
    Procedure,
    Aggregate,
    Window,
}

impl FromSql<'_> for RoutineKind {
    fn from_sql(
        _ty: &postgres_types::Type,
        raw: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(match raw {
            b"f" => Self::Function,
            b"p" => Self::Procedure,
            b"a" => Self::Aggregate,
            b"w" => Self::Window,
            x => Err(UnsupportedRoutineKind(x.to_owned()))?,
        })
    }
    fn accepts(ty: &postgres_types::Type) -> bool {
        *ty == postgres_types::Type::CHAR
    }
}

#[derive(Error, Debug)]
#[error("Unsupported routine kind value")]
struct UnsupportedRoutineKind(Vec<u8>);

#[derive(Debug, FromRow)]
pub(crate) struct RoutineRow {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) kind: RoutineKind,
    pub(crate) identity_arguments: String,
    pub(crate) definition: String,
    pub(crate) dependencies: Json<Vec<RoutineDependencyRow>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub(crate) struct RoutineDependencyRow {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) identity_arguments: String,
}

#[derive(Debug, Query)]
#[query(sql = "enums", row = EnumRow)]
pub(crate) struct Enums {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct EnumRow {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) labels: Vec<String>,
}

#[derive(Debug, Query)]
#[query(sql = "domains", row = DomainRow)]
pub(crate) struct Domains {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct DomainRow {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) base_type: String,
    pub(crate) default: Option<String>,
    pub(crate) notnull: bool,
    pub(crate) constraint_names: Vec<String>,
    pub(crate) constraint_definitions: Vec<String>,
}

#[derive(Debug, Query)]
#[query(sql = "sequences", row = SequenceRow)]
pub(crate) struct Sequences {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct SequenceRow {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) start_value: i64,
    pub(crate) min_value: i64,
    pub(crate) max_value: i64,
    pub(crate) increment_by: i64,
    pub(crate) cycle: bool,
    pub(crate) cache_size: i64,
}

#[derive(Debug, Query)]
#[query(sql = "extensions", row = ExtensionRow)]
pub(crate) struct Extensions {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct ExtensionRow {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) version: String,
}

#[derive(Debug, Query)]
#[query(sql = "indexes", row = IndexRow)]
pub(crate) struct Indexes {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct IndexRow {
    pub(crate) schema: String,
    pub(crate) table_name: String,
    pub(crate) name: String,
    pub(crate) definition: String,
}

#[derive(Debug, Query)]
#[query(sql = "triggers", row = TriggerRow)]
pub(crate) struct Triggers {
    pub(crate) schema: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct TriggerRow {
    pub(crate) schema: String,
    pub(crate) table_name: String,
    pub(crate) name: String,
    pub(crate) definition: String,
    pub(crate) enabled: String,
}
