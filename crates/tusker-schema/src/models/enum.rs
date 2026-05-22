use itertools::Itertools;

use crate::{
    diff::{ChangeType, Diff, DiffSql},
    queries::EnumRow,
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Enum {
    pub schema: String,
    pub name: String,
    pub labels: Vec<String>,
}

impl Enum {
    fn create_sql(&self) -> String {
        format!(
            "CREATE TYPE {}.{} AS ENUM ({});\n",
            quote_ident(&self.schema),
            quote_ident(&self.name),
            self.labels
                .iter()
                .map(|label| quote_literal(label))
                .join(", ")
        )
    }

    fn drop_sql(&self) -> String {
        format!(
            "DROP TYPE {}.{};\n",
            quote_ident(&self.schema),
            quote_ident(&self.name),
        )
    }

    fn alter_sql(&self, previous: &Self) -> Vec<(ChangeType, String)> {
        if can_safely_add_values(previous, self) {
            self.add_value_sql(previous)
        } else {
            vec![(
                ChangeType::AlterType,
                format!(
                    "-- WARNING: enum {}.{} changed incompatibly and no safe automatic migration was generated.\n\
-- Previous labels: {}\n\
-- Target labels: {}\n\
-- Suggested manual approach:\n\
-- 1. Change dependent columns to TEXT with an explicit USING cast.\n\
-- 2. Rewrite any rows, defaults, functions, or views that still reference removed/renamed values.\n\
-- 3. Recreate the enum type with the desired labels.\n\
-- 4. Cast dependent columns back to the enum type.\n\
DO $$\n\
BEGIN\n\
    RAISE EXCEPTION 'Unsafe enum migration required for {}.{}';\n\
END\n\
$$;\n",
                    quote_ident(&self.schema),
                    quote_ident(&self.name),
                    join_labels(&previous.labels),
                    join_labels(&self.labels),
                    self.schema,
                    self.name,
                ),
            )]
        }
    }

    fn add_value_sql(&self, previous: &Self) -> Vec<(ChangeType, String)> {
        // PostgreSQL applies enum additions one statement at a time. The
        // placement of later INSERTs therefore depends on the enum state after
        // the earlier INSERTs have already been applied. We mirror that here
        // by keeping a mutable `current` label list and updating it after each
        // generated statement.
        let mut current = previous.labels.clone();
        let mut statements = Vec::new();

        for (idx, label) in self.labels.iter().enumerate() {
            // If the label is already at the right position in the current
            // intermediate state, nothing needs to be emitted.
            if idx < current.len() && current[idx] == *label {
                continue;
            }
            // Existing labels may appear later in `self.labels` because new
            // values were inserted ahead of them. They still do not require an
            // ALTER TYPE statement.
            if current.contains(label) {
                continue;
            }

            // Only emit placement when the new label must appear before an
            // already-existing label. Appends can use a plain ADD VALUE.
            let placement = if idx < current.len() {
                format!(" BEFORE {}", quote_literal(&current[idx]))
            } else {
                String::new()
            };

            statements.push((
                ChangeType::AlterType,
                format!(
                    "ALTER TYPE {}.{} ADD VALUE {}{};\n",
                    quote_ident(&self.schema),
                    quote_ident(&self.name),
                    quote_literal(label),
                    placement,
                ),
            ));

            // Keep `current` in sync with the statement we just emitted so the
            // next iteration sees the same enum ordering PostgreSQL will see.
            current.insert(idx, label.clone());
        }

        statements
    }
}

impl From<EnumRow> for Enum {
    fn from(row: EnumRow) -> Self {
        Self {
            schema: row.schema,
            name: row.name,
            labels: row.labels,
        }
    }
}

impl DiffSql for Diff<'_, Enum> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropType, a.drop_sql()));
        }
        for (a, b) in &self.a_and_b {
            if a != b {
                v.extend(b.alter_sql(a));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateType, b.create_sql()));
        }
        v
    }
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn join_labels(labels: &[String]) -> String {
    labels.iter().map(|label| quote_literal(label)).join(", ")
}

fn can_safely_add_values(previous: &Enum, new: &Enum) -> bool {
    if previous.labels.len() > new.labels.len() {
        return false;
    }
    previous.labels.iter().eq(new
        .labels
        .iter()
        .filter(|label| previous.labels.contains(label)))
}
