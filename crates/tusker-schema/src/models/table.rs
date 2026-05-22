use itertools::Itertools;
use thiserror::Error;

use crate::{
    diff::{diff, ChangeType, Diff, DiffSql},
    queries::{Class, Relkind},
    sql::quote_ident,
};

use super::column::Column;

#[derive(Debug, Eq, PartialEq)]
pub struct Table {
    pub schema: String,
    pub name: String,
    pub kind: Relkind,
    pub columns: Vec<Column>,
}

impl TryFrom<Class> for Table {
    type Error = InvalidRelkind;
    fn try_from(cls: Class) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: cls.schema,
            name: cls.name,
            kind: cls.relkind,
            columns: cls.columns.0,
        })
    }
}

#[derive(Debug, Error)]
#[error("Unsupported table for table: {0}")]
pub struct InvalidRelkind(Relkind);

impl Table {
    pub fn create(&self) -> String {
        let cols = self.columns.iter().map(|col| col.sql()).join(",\n    ");
        format!(
            "CREATE TABLE {}.{} (\n    {}\n);\n",
            quote_ident(&self.schema),
            quote_ident(&self.name),
            cols
        )
    }
    pub fn drop(&self) -> String {
        format!(
            "DROP TABLE {}.{};\n",
            quote_ident(&self.schema),
            quote_ident(&self.name),
        )
    }
    pub fn alter_sql(&self, col_sql: Vec<(ChangeType, String)>) -> String {
        let mut output = Vec::new();
        let (errors, alter_clauses): (Vec<_>, Vec<_>) = col_sql
            .into_iter()
            .partition(|(change_type, _)| *change_type == ChangeType::Unsupported);
        let has_errors = !errors.is_empty();

        if !alter_clauses.is_empty() {
            output.push(format!(
                "ALTER TABLE {}.{}\n{};\n",
                quote_ident(&self.schema),
                quote_ident(&self.name),
                alter_clauses
                    .iter()
                    .map(|(_, sql)| format!("    {}", sql))
                    .join(",\n"),
            ));
        }

        output.extend(errors.into_iter().map(|(_, sql)| sql));

        if has_errors {
            output.push(format!(
                "DO $$\n\
BEGIN\n\
    RAISE EXCEPTION 'Unsupported schema change for table {}.{}; check the warnings above';\n\
END\n\
$$;\n",
                self.schema, self.name
            ));
        }

        output.join("\n")
    }
    pub fn diff_columns<'a>(&'a self, other: &'a Self) -> Diff<'a, Column> {
        diff(self.columns.iter(), other.columns.iter(), |c| &c.name)
    }
}

impl DiffSql for Diff<'_, Table> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropTable, a.drop()));
        }
        for (a, b) in &self.a_and_b {
            let col_sql = a.diff_columns(b).sql();
            if !col_sql.is_empty() {
                v.push((ChangeType::AlterColumn, b.alter_sql(col_sql)));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateTable, b.create()));
        }
        v
    }
}
