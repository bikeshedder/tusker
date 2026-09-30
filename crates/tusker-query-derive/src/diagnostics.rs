use tusker_query_models::SqlType;

use crate::scalar_type;

/// Returns notes explaining which Rust types can represent `sql_type`.
pub(crate) fn type_hints(sql_type: &SqlType) -> Vec<String> {
    let display_name = sql_type.display_name();
    match sql_type {
        SqlType::Scalar { name, .. } => scalar_type(name)
            .map(|scalar| {
                vec![format!(
                    "PostgreSQL `{display_name}` maps to {}",
                    scalar.rust_types
                )]
            })
            .unwrap_or_default(),
        SqlType::Array { element } => {
            let mut hints = vec![format!(
                "PostgreSQL `{display_name}` maps to `Vec<T>`, `&[T]` or `Box<[T]>` for parameters and `Vec<T>` for columns, where `T` maps `{}`",
                element.display_name()
            )];
            hints.extend(type_hints(element));
            hints
        }
        SqlType::Composite { fields, .. } => {
            let fields = fields
                .iter()
                .map(|field| format!("`{}` ({})", field.name, field.r#type.display_name()))
                .collect::<Vec<_>>()
                .join(", ");
            vec![format!(
                "PostgreSQL composite `{display_name}` maps to a Rust struct deriving `QueryComposite` with the fields {fields}"
            )]
        }
        SqlType::Enum { variants, .. } => {
            let labels = variants
                .iter()
                .map(|label| format!("'{label}'"))
                .collect::<Vec<_>>()
                .join(", ");
            vec![format!(
                "PostgreSQL enum `{display_name}` maps to a Rust enum deriving `QueryEnum` with exactly the labels {labels}"
            )]
        }
    }
}

/// Escapes text for use in `#[diagnostic::on_unimplemented]` format strings.
pub(crate) fn escape_format(text: &str) -> String {
    text.replace('{', "{{").replace('}', "}}")
}
