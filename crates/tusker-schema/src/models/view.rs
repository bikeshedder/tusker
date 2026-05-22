use thiserror::Error;

use crate::{
    queries::{Class, Relkind},
    sql::quote_ident,
};

#[derive(Debug, Eq, PartialEq)]
pub struct View {
    pub schema: String,
    pub name: String,
    pub kind: Relkind,
    pub materialized: bool,
    pub viewdef: String,
}

impl View {
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

#[derive(Debug, Error)]
#[error("Unsupported table for view: {0}")]
pub struct InvalidRelkind(Relkind);
