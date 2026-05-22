use crate::{
    diff::{diff, ChangeType, Diff, DiffSql},
    queries::DomainRow,
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Domain {
    pub schema: String,
    pub name: String,
    pub base_type: String,
    pub default: Option<String>,
    pub not_null: bool,
    pub constraints: Vec<DomainConstraint>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DomainConstraint {
    pub name: String,
    pub definition: String,
}

impl Domain {
    fn qualified_name(&self) -> String {
        format!("{}.{}", quote_ident(&self.schema), quote_ident(&self.name),)
    }

    fn create_sql(&self) -> String {
        let mut parts = vec![format!(
            "CREATE DOMAIN {} AS {}",
            self.qualified_name(),
            self.base_type
        )];
        if let Some(default) = &self.default {
            parts.push(format!("DEFAULT {}", default));
        }
        if self.not_null {
            parts.push("NOT NULL".to_string());
        }
        for constraint in &self.constraints {
            parts.push(format!(
                "CONSTRAINT {} {}",
                quote_ident(&constraint.name),
                constraint.definition
            ));
        }
        format!("{};\n", parts.join(" "))
    }

    fn drop_sql(&self) -> String {
        format!("DROP DOMAIN {};\n", self.qualified_name())
    }

    fn alter_sql(&self, previous: &Self) -> Vec<(ChangeType, String)> {
        if self.base_type != previous.base_type {
            return vec![(
                ChangeType::AlterType,
                format!(
                    "-- WARNING: domain {} changed base type from {} to {} and no safe automatic migration was generated.\n\
-- Suggested manual approach:\n\
-- 1. Drop or migrate dependent columns, defaults, and routines.\n\
-- 2. Recreate the domain with the desired base type.\n\
-- 3. Reapply dependencies.\n\
DO $$\n\
BEGIN\n\
    RAISE EXCEPTION 'Unsafe domain migration required for {}';\n\
END\n\
$$;\n",
                    self.qualified_name(),
                    previous.base_type,
                    self.base_type,
                    self.qualified_name(),
                ),
            )];
        }

        let mut sql = Vec::new();

        if previous.default != self.default {
            sql.push((
                ChangeType::AlterType,
                match &self.default {
                    Some(default) => format!(
                        "ALTER DOMAIN {} SET DEFAULT {};\n",
                        self.qualified_name(),
                        default
                    ),
                    None => format!("ALTER DOMAIN {} DROP DEFAULT;\n", self.qualified_name()),
                },
            ));
        }

        if previous.not_null != self.not_null {
            sql.push((
                ChangeType::AlterType,
                format!(
                    "ALTER DOMAIN {} {};\n",
                    self.qualified_name(),
                    if self.not_null {
                        "SET NOT NULL"
                    } else {
                        "DROP NOT NULL"
                    }
                ),
            ));
        }

        let constraint_diff = diff(
            previous.constraints.iter(),
            self.constraints.iter(),
            |constraint| &constraint.name,
        );

        for constraint in &constraint_diff.a_only {
            sql.push((
                ChangeType::AlterType,
                format!(
                    "ALTER DOMAIN {} DROP CONSTRAINT {};\n",
                    self.qualified_name(),
                    quote_ident(&constraint.name)
                ),
            ));
        }

        for (old, new) in &constraint_diff.a_and_b {
            if old != new {
                sql.push((
                    ChangeType::AlterType,
                    format!(
                        "ALTER DOMAIN {} DROP CONSTRAINT {};\n",
                        self.qualified_name(),
                        quote_ident(&old.name)
                    ),
                ));
                sql.push((
                    ChangeType::AlterType,
                    format!(
                        "ALTER DOMAIN {} ADD CONSTRAINT {} {};\n",
                        self.qualified_name(),
                        quote_ident(&new.name),
                        new.definition
                    ),
                ));
            }
        }

        for constraint in &constraint_diff.b_only {
            sql.push((
                ChangeType::AlterType,
                format!(
                    "ALTER DOMAIN {} ADD CONSTRAINT {} {};\n",
                    self.qualified_name(),
                    quote_ident(&constraint.name),
                    constraint.definition
                ),
            ));
        }

        sql
    }
}

impl From<DomainRow> for Domain {
    fn from(row: DomainRow) -> Self {
        Self {
            schema: row.schema,
            name: row.name,
            base_type: row.base_type,
            default: row.default,
            not_null: row.notnull,
            constraints: row
                .constraint_names
                .into_iter()
                .zip(row.constraint_definitions)
                .map(|(name, definition)| DomainConstraint { name, definition })
                .collect(),
        }
    }
}

impl DiffSql for Diff<'_, Domain> {
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
