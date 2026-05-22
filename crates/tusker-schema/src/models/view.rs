use thiserror::Error;

use crate::{
    queries::{Class, Relkind},
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
/// A PostgreSQL view or materialized view definition.
pub struct View {
    /// Schema that owns the view.
    pub schema: String,
    /// View name.
    pub name: String,
    kind: Relkind,
    /// Whether this definition describes a materialized view.
    pub materialized: bool,
    /// SQL body used to define the view.
    pub viewdef: String,
}

impl View {
    /// Renders a `CREATE OR REPLACE VIEW` statement for the view.
    pub fn create(&self) -> String {
        format!(
            "CREATE OR REPLACE {}VIEW {}.{} AS\n{};\n",
            if self.materialized {
                "MATERIALIZED "
            } else {
                ""
            },
            quote_ident(&self.schema),
            quote_ident(&self.name),
            self.viewdef,
        )
    }
}

impl TryFrom<Class> for View {
    type Error = InvalidRelkind;
    fn try_from(cls: Class) -> Result<Self, Self::Error> {
        let materialized = match cls.relkind {
            Relkind::View => false,
            Relkind::MaterializedView => true,
            _ => return Err(InvalidRelkind(cls.relkind)),
        };
        Ok(Self {
            schema: cls.schema,
            name: cls.name,
            kind: cls.relkind,
            materialized,
            viewdef: cls.viewdef.unwrap(),
        })
    }
}

/// Error returned when a non-view relation is converted into a [`View`].
#[derive(Debug, Error)]
#[error("Unsupported table for view: {0}")]
pub struct InvalidRelkind(Relkind);
