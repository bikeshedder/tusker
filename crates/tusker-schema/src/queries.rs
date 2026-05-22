use std::fmt;

use postgres_types::FromSql;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_postgres::types::Json;
use tusker_query::{FromRow, Query};

use crate::models::{column::Column, constraint::ConstraintType};

#[derive(Query)]
#[query(sql="schemas", row=Schema)]
pub struct Schemas {}

#[derive(Debug, FromRow)]
pub struct Schema {
    pub name: String,
}

#[derive(Query)]
#[query(sql="classes", row=Class)]
pub struct Classes {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct Class {
    pub schema: String,
    pub name: String,
    pub relkind: Relkind,
    pub columns: Json<Vec<Column>>,
    pub viewdef: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Relkind {
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

#[derive(Query)]
#[query(sql="constraints", row=Constraint)]
pub struct Constraints {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct Constraint {
    pub table: String,
    pub name: String,
    pub r#type: ConstraintType,
    pub def: String,
}

#[derive(Query)]
#[query(sql = "routines", row = RoutineRow)]
pub struct Routines {
    pub schema: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RoutineKind {
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
pub struct RoutineRow {
    pub schema: String,
    pub name: String,
    pub kind: RoutineKind,
    pub identity_arguments: String,
    pub definition: String,
    pub dependencies: Json<Vec<RoutineDependencyRow>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct RoutineDependencyRow {
    pub schema: String,
    pub name: String,
    pub identity_arguments: String,
}

#[derive(Query)]
#[query(sql = "enums", row = EnumRow)]
pub struct Enums {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct EnumRow {
    pub schema: String,
    pub name: String,
    pub labels: Vec<String>,
}

#[derive(Query)]
#[query(sql = "domains", row = DomainRow)]
pub struct Domains {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct DomainRow {
    pub schema: String,
    pub name: String,
    pub base_type: String,
    pub default: Option<String>,
    pub notnull: bool,
    pub constraint_names: Vec<String>,
    pub constraint_definitions: Vec<String>,
}

#[derive(Query)]
#[query(sql = "sequences", row = SequenceRow)]
pub struct Sequences {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct SequenceRow {
    pub schema: String,
    pub name: String,
    pub data_type: String,
    pub start_value: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub increment_by: i64,
    pub cycle: bool,
    pub cache_size: i64,
}

#[derive(Query)]
#[query(sql = "extensions", row = ExtensionRow)]
pub struct Extensions {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct ExtensionRow {
    pub schema: String,
    pub name: String,
    pub version: String,
}

#[derive(Query)]
#[query(sql = "indexes", row = IndexRow)]
pub struct Indexes {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct IndexRow {
    pub schema: String,
    pub table_name: String,
    pub name: String,
    pub definition: String,
}

#[derive(Query)]
#[query(sql = "triggers", row = TriggerRow)]
pub struct Triggers {
    pub schema: String,
}

#[derive(Debug, FromRow)]
pub struct TriggerRow {
    pub schema: String,
    pub table_name: String,
    pub name: String,
    pub definition: String,
    pub enabled: String,
}
