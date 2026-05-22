use std::cmp::Reverse;

use anyhow::anyhow;
use postgres_types::FromSql;

use crate::{
    diff::{ChangeType, Diff, DiffSql},
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Constraint {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub r#type: ConstraintType,
    pub definition: String,
}

impl Constraint {
    fn create_sql(&self) -> String {
        format!(
            "ALTER TABLE {}.{} ADD CONSTRAINT {} {};\n",
            quote_ident(&self.schema),
            quote_ident(&self.table),
            quote_ident(&self.name),
            self.definition,
        )
    }
    fn drop_sql(&self) -> String {
        format!(
            "ALTER TABLE {}.{} DROP CONSTRAINT {};\n",
            quote_ident(&self.schema),
            quote_ident(&self.table),
            quote_ident(&self.name),
        )
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ConstraintType {
    Check,
    NotNull,
    PrimaryKey,
    Unique,
    Trigger,
    Exclusion,
    // Foreign keys need to be created last as they depend on unique
    // constraints or primary keys.
    ForeignKey,
}

impl<'a> FromSql<'a> for ConstraintType {
    fn from_sql(
        ty: &postgres_types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let s = String::from_sql(ty, raw)?;
        match s.as_str() {
            "c" => Ok(Self::Check),
            "n" => Ok(Self::NotNull),
            "f" => Ok(Self::ForeignKey),
            "p" => Ok(Self::PrimaryKey),
            "u" => Ok(Self::Unique),
            "t" => Ok(Self::Trigger),
            "x" => Ok(Self::Exclusion),
            _ => Err(anyhow!("Unsupported contype: {s}"))?,
        }
    }

    fn accepts(ty: &postgres_types::Type) -> bool {
        *ty == postgres_types::Type::CHAR
    }
}

impl DiffSql for Diff<'_, Constraint> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropConstraint(Reverse(a.r#type)), a.drop_sql()));
        }
        for (a, b) in &self.a_and_b {
            if a != b {
                v.push((ChangeType::DropConstraint(Reverse(a.r#type)), a.drop_sql()));
                v.push((ChangeType::CreateConstraint(b.r#type), b.create_sql()));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateConstraint(b.r#type), b.create_sql()));
        }
        v
    }
}
