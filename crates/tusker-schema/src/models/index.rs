use crate::{
    diff::{ChangeType, Diff, DiffSql},
    queries::IndexRow,
    sql::quote_ident,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Index {
    pub schema: String,
    pub table_name: String,
    pub name: String,
    pub definition: String,
}

impl Index {
    fn create_sql(&self) -> String {
        format!(
            "{};\n",
            self.definition.trim_end_matches('\n').trim_end_matches(';')
        )
    }

    fn drop_sql(&self) -> String {
        format!(
            "DROP INDEX {}.{};\n",
            quote_ident(&self.schema),
            quote_ident(&self.name),
        )
    }
}

impl From<IndexRow> for Index {
    fn from(row: IndexRow) -> Self {
        Self {
            schema: row.schema,
            table_name: row.table_name,
            name: row.name,
            definition: row.definition,
        }
    }
}

impl DiffSql for Diff<'_, Index> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropIndex, a.drop_sql()));
        }
        for (a, b) in &self.a_and_b {
            if a != b {
                v.push((ChangeType::DropIndex, a.drop_sql()));
                v.push((ChangeType::CreateIndex, b.create_sql()));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateIndex, b.create_sql()));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use crate::diff::{Diff, DiffSql};

    use super::Index;

    fn index(name: &str, definition: &str) -> Index {
        Index {
            schema: "public".into(),
            table_name: "employees".into(),
            name: name.into(),
            definition: definition.into(),
        }
    }

    #[test]
    fn creates_named_indexes() {
        let idx = index(
            "employees_tenant_id_idx",
            "CREATE INDEX employees_tenant_id_idx ON public.employees USING btree (tenant_id)",
        );
        let diff = Diff {
            a_only: vec![],
            a_and_b: vec![],
            b_only: vec![&idx],
        };

        assert_eq!(
            diff.sql(),
            vec![(
                crate::diff::ChangeType::CreateIndex,
                "CREATE INDEX employees_tenant_id_idx ON public.employees USING btree (tenant_id);\n"
                    .into(),
            )]
        );
    }

    #[test]
    fn recreates_changed_unique_indexes() {
        let old = index(
            "employees_email_uidx",
            "CREATE UNIQUE INDEX employees_email_uidx ON public.employees USING btree (email)",
        );
        let new = index(
            "employees_email_uidx",
            "CREATE UNIQUE INDEX employees_email_uidx ON public.employees USING btree (lower(email))",
        );
        let diff = Diff {
            a_only: vec![],
            a_and_b: vec![(&old, &new)],
            b_only: vec![],
        };

        assert_eq!(
            diff.sql(),
            vec![
                (
                    crate::diff::ChangeType::DropIndex,
                    "DROP INDEX \"public\".\"employees_email_uidx\";\n".into(),
                ),
                (
                    crate::diff::ChangeType::CreateIndex,
                    "CREATE UNIQUE INDEX employees_email_uidx ON public.employees USING btree (lower(email));\n"
                        .into(),
                ),
            ]
        );
    }
}
