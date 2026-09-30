use syn::LitStr;

use crate::case::RenameRule;

pub(crate) struct Overrides {
    pub(crate) name: Option<String>,
    pub(crate) rename_all: Option<RenameRule>,
    pub(crate) allow_mismatch: bool,
}

impl Overrides {
    pub(crate) fn extract(attrs: &[syn::Attribute], container: bool) -> syn::Result<Self> {
        let mut overrides = Self {
            name: None,
            rename_all: None,
            allow_mismatch: false,
        };

        for attr in attrs {
            if !attr.path().is_ident("postgres") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    overrides.name = Some(lit.value());
                    Ok(())
                } else if meta.path.is_ident("rename_all") {
                    if !container {
                        return Err(
                            meta.error("rename_all is only supported on composite and enum types")
                        );
                    }
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    overrides.rename_all = Some(
                        RenameRule::from_str(&lit.value())
                            .ok_or_else(|| meta.error("invalid rename_all rule"))?,
                    );
                    Ok(())
                } else if meta.path.is_ident("allow_mismatch") {
                    if !container {
                        return Err(meta.error("allow_mismatch is only supported on enum types"));
                    }
                    overrides.allow_mismatch = true;
                    Ok(())
                } else {
                    Ok(())
                }
            })?;
        }

        Ok(overrides)
    }
}
