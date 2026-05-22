use crate::{
    diff::{ChangeType, Diff, DiffSql},
    queries::TriggerRow,
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Trigger {
    pub schema: String,
    pub table_name: String,
    pub name: String,
    pub enabled: String,
    pub definition: String,
}

impl Trigger {
    fn create_sql(&self) -> String {
        let mut sql = format!("{};\n", self.definition);
        match self.enabled.as_str() {
            "O" => {}
            "D" => {
                sql.push_str(&format!(
                    "ALTER TABLE {}.{} DISABLE TRIGGER {};\n",
                    quote_ident(&self.schema),
                    quote_ident(&self.table_name),
                    quote_ident(&self.name),
                ));
            }
            "R" => {
                sql.push_str(&format!(
                    "ALTER TABLE {}.{} ENABLE REPLICA TRIGGER {};\n",
                    quote_ident(&self.schema),
                    quote_ident(&self.table_name),
                    quote_ident(&self.name),
                ));
            }
            "A" => {
                sql.push_str(&format!(
                    "ALTER TABLE {}.{} ENABLE ALWAYS TRIGGER {};\n",
                    quote_ident(&self.schema),
                    quote_ident(&self.table_name),
                    quote_ident(&self.name),
                ));
            }
            _ => {}
        }
        sql
    }

    fn drop_sql(&self) -> String {
        format!(
            "DROP TRIGGER {} ON {}.{};\n",
            quote_ident(&self.name),
            quote_ident(&self.schema),
            quote_ident(&self.table_name),
        )
    }
}

impl From<TriggerRow> for Trigger {
    fn from(row: TriggerRow) -> Self {
        Self {
            schema: row.schema,
            table_name: row.table_name,
            name: row.name,
            enabled: row.enabled,
            definition: row.definition,
        }
    }
}

impl DiffSql for Diff<'_, Trigger> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropTrigger, a.drop_sql()));
        }
        for (a, b) in &self.a_and_b {
            if a != b {
                v.push((ChangeType::DropTrigger, a.drop_sql()));
                v.push((ChangeType::CreateTrigger, b.create_sql()));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateTrigger, b.create_sql()));
        }
        v
    }
}
