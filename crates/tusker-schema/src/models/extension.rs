use crate::{
    diff::{ChangeType, Diff, DiffSql},
    queries::ExtensionRow,
    sql::quote_ident,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Extension {
    pub schema: String,
    pub name: String,
    pub version: String,
}

impl Extension {
    fn create_sql(&self) -> String {
        format!(
            "CREATE EXTENSION IF NOT EXISTS {} WITH SCHEMA {} VERSION '{}';\n",
            quote_ident(&self.name),
            quote_ident(&self.schema),
            quote_literal(&self.version),
        )
    }

    fn drop_sql(&self) -> String {
        format!("DROP EXTENSION IF EXISTS {};\n", quote_ident(&self.name),)
    }

    fn alter_sql(&self, previous: &Self) -> Vec<(ChangeType, String)> {
        let mut statements = Vec::new();

        if self.schema != previous.schema {
            statements.push((
                ChangeType::AlterExtension,
                format!(
                    "ALTER EXTENSION {} SET SCHEMA {};\n",
                    quote_ident(&self.name),
                    quote_ident(&self.schema),
                ),
            ));
        }

        if self.version != previous.version {
            statements.push((
                ChangeType::AlterExtension,
                format!(
                    "ALTER EXTENSION {} UPDATE TO '{}';\n",
                    quote_ident(&self.name),
                    quote_literal(&self.version),
                ),
            ));
        }

        statements
    }
}

impl From<ExtensionRow> for Extension {
    fn from(row: ExtensionRow) -> Self {
        Self {
            schema: row.schema,
            name: row.name,
            version: row.version,
        }
    }
}

impl DiffSql for Diff<'_, Extension> {
    fn sql(&self) -> Vec<(ChangeType, String)> {
        let mut v = Vec::new();
        for a in &self.a_only {
            v.push((ChangeType::DropExtension, a.drop_sql()));
        }
        for (a, b) in &self.a_and_b {
            if a != b {
                v.extend(b.alter_sql(a));
            }
        }
        for b in &self.b_only {
            v.push((ChangeType::CreateExtension, b.create_sql()));
        }
        v
    }
}

fn quote_literal(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use crate::{
        diff::DiffSql,
        models::{extension::Extension, schema::Schema},
    };

    #[test]
    fn creates_extensions() {
        let from = Schema::new("public");
        let mut to = Schema::new("public");
        to.extensions.insert(
            "hstore".into(),
            Extension {
                schema: "public".into(),
                name: "hstore".into(),
                version: "1.8".into(),
            },
        );

        assert_eq!(
            from.diff_extensions(&to).sql(),
            vec![(
                crate::diff::ChangeType::CreateExtension,
                "CREATE EXTENSION IF NOT EXISTS \"hstore\" WITH SCHEMA \"public\" VERSION '1.8';\n"
                    .into(),
            )]
        );
    }

    #[test]
    fn alters_extension_schema_and_version() {
        let mut from = Schema::new("public");
        from.extensions.insert(
            "postgis".into(),
            Extension {
                schema: "public".into(),
                name: "postgis".into(),
                version: "3.4.0".into(),
            },
        );
        let mut to = Schema::new("extensions");
        to.extensions.insert(
            "postgis".into(),
            Extension {
                schema: "extensions".into(),
                name: "postgis".into(),
                version: "3.5.0".into(),
            },
        );

        assert_eq!(
            from.diff_extensions(&to).sql(),
            vec![
                (
                    crate::diff::ChangeType::AlterExtension,
                    "ALTER EXTENSION \"postgis\" SET SCHEMA \"extensions\";\n".into(),
                ),
                (
                    crate::diff::ChangeType::AlterExtension,
                    "ALTER EXTENSION \"postgis\" UPDATE TO '3.5.0';\n".into(),
                ),
            ]
        );
    }

    #[test]
    fn drops_extensions() {
        let mut from = Schema::new("public");
        from.extensions.insert(
            "hstore".into(),
            Extension {
                schema: "public".into(),
                name: "hstore".into(),
                version: "1.8".into(),
            },
        );
        let to = Schema::new("public");

        assert_eq!(
            from.diff_extensions(&to).sql(),
            vec![(
                crate::diff::ChangeType::DropExtension,
                "DROP EXTENSION IF EXISTS \"hstore\";\n".into(),
            )]
        );
    }
}
