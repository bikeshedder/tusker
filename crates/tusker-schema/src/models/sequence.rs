use crate::{
    diff::{ChangeType, Diff, DiffSql},
    queries::SequenceRow,
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Sequence {
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

impl Sequence {
    fn qualified_name(&self) -> String {
        format!("{}.{}", quote_ident(&self.schema), quote_ident(&self.name))
    }

    fn options_sql(&self) -> String {
        let mut sql = format!(
            "    AS {}\n    INCREMENT BY {}\n    MINVALUE {}\n    MAXVALUE {}\n",
            self.data_type, self.increment_by, self.min_value, self.max_value,
        );
        sql.push_str(&format!("    START WITH {}\n", self.start_value));
        sql.push_str(&format!(
            "    CACHE {}\n    {}",
            self.cache_size,
            if self.cycle { "CYCLE" } else { "NO CYCLE" },
        ));
        sql
    }

    fn create_sql(&self) -> String {
        format!(
            "CREATE SEQUENCE {}\n{};\n",
            self.qualified_name(),
            self.options_sql(),
        )
    }

    fn drop_sql(&self) -> String {
        format!("DROP SEQUENCE {};\n", self.qualified_name())
    }

    fn alter_sql(&self, previous: &Self) -> String {
        let mut statements = Vec::new();

        // PostgreSQL validates the existing restart value against the new
        // MINVALUE before applying START WITH from the same ALTER SEQUENCE.
        // If the new minimum crosses above the old start, we have to move
        // START/RESTART first and then apply the full alter.
        if self.min_value > previous.start_value {
            statements.push(format!(
                "ALTER SEQUENCE {}\n    START WITH {}\n    RESTART WITH {};\n",
                self.qualified_name(),
                self.start_value,
                self.start_value,
            ));
        }

        statements.push(format!(
            "ALTER SEQUENCE {}\n{};\n",
            self.qualified_name(),
            self.options_sql()
        ));

        statements.join("\n")
    }
}

impl From<SequenceRow> for Sequence {
    fn from(row: SequenceRow) -> Self {
        Self {
            schema: row.schema,
            name: row.name,
            data_type: row.data_type,
            start_value: row.start_value,
            min_value: row.min_value,
            max_value: row.max_value,
            increment_by: row.increment_by,
            cycle: row.cycle,
            cache_size: row.cache_size,
        }
    }
}

impl DiffSql for Diff<'_, Sequence> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropSequence, a.drop_sql()));
        }
        for (a, b) in &self.a_and_b {
            if a != b {
                v.push((ChangeType::AlterSequence, b.alter_sql(a)));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateSequence, b.create_sql()));
        }
        v
    }
}
