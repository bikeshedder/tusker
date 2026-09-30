#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(
    nonstandard_style,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
#![forbid(non_ascii_idents, unsafe_code)]
#![warn(
    deprecated_in_future,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]
#![allow(clippy::uninlined_format_args)]

use std::{env, fs, path::PathBuf};

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use sha2::{Digest, Sha512};
use syn::{
    spanned::Spanned,
    visit_mut::{self, VisitMut},
    Data, DeriveInput,
};
use tusker_query_models::{Column, Query as QueryMetadata, SqlType};

mod case;
mod composites;
mod diagnostics;
mod enums;
mod overrides;

#[derive(FromDeriveInput)]
#[darling(attributes(query), supports(struct_named))]
struct QueryTraitOpts {
    ident: syn::Ident,
    sql: String,
    row: syn::Path,
}

#[proc_macro_derive(Query, attributes(query))]
/// Derives `tusker_query::Query` for a named struct.
pub fn derive_query(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    let opts = match QueryTraitOpts::from_derive_input(&ast) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors().into(),
    };
    match expand_query(&ast, &opts) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_query(ast: &DeriveInput, opts: &QueryTraitOpts) -> syn::Result<TokenStream2> {
    let generics = ast.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let Data::Struct(s) = &ast.data else {
        unreachable!();
    };
    let name = &opts.ident;
    let sql_path = &opts.sql;
    let row = &opts.row;
    let params = s.fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        quote! {
            &self.#field_name
        }
    });

    let (sidecar_validation, sidecar_dependency) =
        if let Some(sidecar) = load_sidecar_metadata(sql_path, name)? {
            (
                build_query_validation(
                    s.fields.iter().map(|field| &field.ty).collect(),
                    &ast.generics,
                    row,
                    &sidecar,
                )?,
                quote! {
                    const _: &str = include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/db/queries/",
                        #sql_path,
                        ".json"
                    ));
                },
            )
        } else {
            (quote! {}, quote! {})
        };

    Ok(quote! {
        impl #impl_generics ::tusker_query::Query for #name #ty_generics #where_clause {
            const SQL: &'static str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/db/queries/",
                #sql_path,
                ".sql"
            ));
            type Row = #row;
            fn as_params(&self) -> Box<[&(dyn ::tokio_postgres::types::ToSql + Sync)]> {
                #sidecar_validation
                Box::new([
                    #( #params ),*
                ])
            }
        }

        #sidecar_dependency
    })
}

fn load_sidecar_metadata(
    sql_path: &str,
    error_target: &impl ToTokens,
) -> syn::Result<Option<QueryMetadata>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").map_err(|err| {
        syn::Error::new_spanned(
            error_target,
            format!("Unable to determine CARGO_MANIFEST_DIR: {err}"),
        )
    })?;
    let sql_file = PathBuf::from(&manifest_dir)
        .join("db/queries")
        .join(format!("{sql_path}.sql"));
    let json_file = PathBuf::from(&manifest_dir)
        .join("db/queries")
        .join(format!("{sql_path}.json"));

    if !json_file.exists() {
        return Ok(None);
    }

    let sql = fs::read(&sql_file).map_err(|err| {
        syn::Error::new_spanned(
            error_target,
            format!(
                "Unable to read query SQL file {}: {err}",
                sql_file.display()
            ),
        )
    })?;
    let json = fs::read(&json_file).map_err(|err| {
        syn::Error::new_spanned(
            error_target,
            format!(
                "Unable to read query sidecar file {}: {err}",
                json_file.display()
            ),
        )
    })?;
    let metadata: QueryMetadata = serde_json::from_slice(&json).map_err(|err| {
        syn::Error::new_spanned(
            error_target,
            format!(
                "Unable to parse query sidecar file {}: {err}",
                json_file.display()
            ),
        )
    })?;

    let mut hasher = Sha512::new();
    hasher.update(&sql);
    let checksum = hasher.finalize().to_vec();
    if metadata.checksum != checksum {
        return Err(syn::Error::new_spanned(
            error_target,
            format!(
                "Query sidecar file {} is out of date. Run `tusker query sync` to refresh it.",
                json_file.display()
            ),
        ));
    }

    Ok(Some(metadata))
}

fn build_query_validation(
    field_types: Vec<&syn::Type>,
    generics: &syn::Generics,
    row: &syn::Path,
    sidecar: &QueryMetadata,
) -> syn::Result<TokenStream2> {
    if sidecar.params.len() != field_types.len() {
        return Err(syn::Error::new_spanned(
            row,
            format!(
                "Query parameter count mismatch: Rust struct has {} fields but the sidecar expects {} parameters.",
                field_types.len(),
                sidecar.params.len()
            ),
        ));
    }

    let param_checks = field_types
        .iter()
        .zip(sidecar.params.iter())
        .enumerate()
        .map(|(idx, (field_type, sql_type))| build_param_check(idx, field_type, sql_type, generics))
        .collect::<syn::Result<Vec<_>>>()?;

    let row_checks = sidecar
        .columns
        .iter()
        .enumerate()
        .map(|(idx, column)| build_row_check(row, idx, column, generics))
        .collect::<syn::Result<Vec<_>>>()?;
    let row_len = sidecar.columns.len();
    let row_count_message = format!(
        "`{{Self}}` does not have the {row_len} fields required by the {row_len} columns the query returns"
    );

    let row_count_check = quote_spanned! {row.span()=>
        #[diagnostic::on_unimplemented(
            message = #row_count_message,
            label = "row field count does not match the query"
        )]
        trait __TuskerRowCount {}
        impl<Row> __TuskerRowCount for Row
        where
            Row: ::tusker_query::__private::RowFieldCount<#row_len>,
        {
        }
        fn __tusker_check_row_count<Row: __TuskerRowCount>() {}
        __tusker_check_row_count::<#row>();
    };

    Ok(quote! {
        {
            #(#param_checks)*
            #row_count_check
            #(#row_checks)*
        }
    })
}

fn build_param_check(
    index: usize,
    field_type: &syn::Type,
    sql_type: &SqlType,
    generics: &syn::Generics,
) -> syn::Result<TokenStream2> {
    let position = index + 1;
    let marker = sql_type_marker(sql_type).map_err(|message| {
        syn::Error::new_spanned(
            field_type,
            format!("Parameter {position} cannot be checked: {message}"),
        )
    })?;
    let display_name = diagnostics::escape_format(&sql_type.display_name());
    let message = format!(
        "`{{Self}}` cannot be used for parameter {position} of PostgreSQL type `{display_name}`"
    );
    let label = format!("incompatible with parameter {position}");
    let notes = diagnostics::type_hints(sql_type)
        .iter()
        .map(|note| diagnostics::escape_format(note))
        .collect::<Vec<_>>();
    let trait_ident = format_ident!("__TuskerParam{}", position, span = field_type.span());
    let fn_ident = format_ident!("__tusker_check_param{}", position, span = field_type.span());
    let (marker, enum_check) = match build_enum_check(
        field_type,
        field_type,
        sql_type,
        &format!("parameter {position}"),
        generics,
    ) {
        Some(EnumCheck { marker, checks }) => (marker, checks),
        None => (marker, quote!()),
    };

    Ok(quote_spanned! {field_type.span()=>
        #[diagnostic::on_unimplemented(message = #message, label = #label #(, note = #notes)*)]
        trait #trait_ident {}
        impl<T> #trait_ident for T where T: ::tusker_query::types::QueryParamTyped<#marker> {}
        fn #fn_ident<T: #trait_ident>() {}
        #fn_ident::<#field_type>();
        #enum_check
    })
}

fn build_row_check(
    row: &syn::Path,
    index: usize,
    column: &Column,
    generics: &syn::Generics,
) -> syn::Result<TokenStream2> {
    let position = index + 1;
    let marker = sql_type_marker(&column.r#type).map_err(|message| {
        syn::Error::new_spanned(
            row,
            format!(
                "Column `{}` at position {position} cannot be checked: {message}",
                column.name
            ),
        )
    })?;

    let field_type: syn::Type =
        syn::parse_quote!(<#row as ::tusker_query::__private::RowFieldType<#index>>::Ty);
    let (marker, enum_check) = match build_enum_check(
        row,
        &field_type,
        &column.r#type,
        &format!("column `{}` (position {position})", column.name),
        generics,
    ) {
        Some(EnumCheck { marker, checks }) => (marker, checks),
        None => (marker, quote!()),
    };

    let column_name = diagnostics::escape_format(&column.name);
    let display_name = diagnostics::escape_format(&column.r#type.display_name());
    let mut notes = diagnostics::type_hints(&column.r#type)
        .iter()
        .map(|note| diagnostics::escape_format(note))
        .collect::<Vec<_>>();
    let (bound, message) = match column.notnull {
        Some(true) => {
            notes.push("the column is NOT NULL, so its field must not be an `Option`".to_owned());
            (
                quote!(::tusker_query::types::QueryRowTyped<#marker>),
                format!(
                    "`{{Self}}` cannot be used for NOT NULL column `{column_name}` (position {position}) of PostgreSQL type `{display_name}`"
                ),
            )
        }
        Some(false) | None => (
            quote!(::tusker_query::types::QueryMaybeNullableRowTyped<#marker>),
            format!(
                "`{{Self}}` cannot be used for column `{column_name}` (position {position}) of PostgreSQL type `{display_name}`"
            ),
        ),
    };
    let label = format!("incompatible with column `{column_name}`");
    let trait_ident = format_ident!("__TuskerColumn{}", position, span = row.span());
    let fn_ident = format_ident!("__tusker_check_column{}", position, span = row.span());
    Ok(quote_spanned! {row.span()=>
        #[diagnostic::on_unimplemented(message = #message, label = #label #(, note = #notes)*)]
        trait #trait_ident {}
        impl<T> #trait_ident for T where T: #bound {}
        fn #fn_ident<Row>()
        where
            Row: ::tusker_query::__private::RowFieldType<#index>,
            <Row as ::tusker_query::__private::RowFieldType<#index>>::Ty: #trait_ident,
        {
        }
        #fn_ident::<#row>();
        #enum_check
    })
}

struct EnumCheck {
    /// Marker built from the Rust enum's own name and labels.
    marker: TokenStream2,
    /// Constant assertions comparing the names and labels.
    checks: TokenStream2,
}

/// Builds readable checks for enum (and enum array) parameters and columns.
///
/// The type-level check can only compare name and label hashes, which makes
/// for unreadable errors. Instead, the returned marker is built from the Rust
/// enum's own name and labels, so the type-level check only covers the shape
/// (nullability, arrays), and constant assertions name any mismatching labels.
/// Returns `None` when the Rust type depends on generic type parameters, which
/// constants cannot refer to; the type-level check then covers everything.
fn build_enum_check(
    span: &impl Spanned,
    field_type: &syn::Type,
    sql_type: &SqlType,
    context: &str,
    generics: &syn::Generics,
) -> Option<EnumCheck> {
    let (name, labels, is_array) = match sql_type {
        SqlType::Enum { name, variants, .. } => (name, variants, false),
        SqlType::Array { element } => match element.as_ref() {
            SqlType::Enum { name, variants, .. } => (name, variants, true),
            _ => return None,
        },
        _ => return None,
    };
    let field_type = generic_free_type(field_type, generics)?;
    let span = span.span();
    let labels_trait = quote!(<#field_type as ::tusker_query::types::QueryEnumLabels>);

    let enum_marker = quote!(::tusker_query::types::PgEnum<
        { #labels_trait::NAME_HASH },
        #labels_trait::Labels
    >);
    let marker = if is_array {
        quote!(::tusker_query::types::PgArray<#enum_marker>)
    } else {
        enum_marker
    };

    let name_message = format!(
        "the Rust enum for {context} does not map PostgreSQL enum `{name}`; check its `#[postgres(name = \"...\")]` attribute"
    );
    let missing_checks = labels.iter().map(|label| {
        let message = format!(
            "PostgreSQL enum `{name}` has label '{label}' which the Rust enum for {context} does not map"
        );
        quote_spanned! {span=>
            if !::tusker_query::__private::contains_label(#labels_trait::LABELS, #label) {
                panic!("{}", #message);
            }
        }
    });
    let checks = quote_spanned! {span=>
        const _: () = {
            if !::tusker_query::__private::str_eq(#labels_trait::NAME, #name) {
                panic!("{}", #name_message);
            }
            if !#labels_trait::ALLOW_MISMATCH {
                #(#missing_checks)*
                if let Some(idx) = ::tusker_query::__private::first_extra_label(
                    #labels_trait::LABELS,
                    &[#(#labels),*],
                ) {
                    panic!("{}", #labels_trait::EXTRA_LABEL_ERRORS[idx]);
                }
            }
        };
    };
    Some(EnumCheck { marker, checks })
}

/// Returns `ty` with all lifetimes replaced by `'static`, or `None` if it
/// refers to a generic type or const parameter.
fn generic_free_type(ty: &syn::Type, generics: &syn::Generics) -> Option<syn::Type> {
    struct Visitor<'a> {
        generics: &'a syn::Generics,
        generic: bool,
    }

    impl VisitMut for Visitor<'_> {
        fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
            *lifetime = syn::Lifetime::new("'static", lifetime.span());
        }

        fn visit_path_mut(&mut self, path: &mut syn::Path) {
            if let Some(ident) = path.get_ident() {
                self.generic |= self.generics.params.iter().any(|param| match param {
                    syn::GenericParam::Type(param) => param.ident == *ident,
                    syn::GenericParam::Const(param) => param.ident == *ident,
                    syn::GenericParam::Lifetime(_) => false,
                });
            }
            visit_mut::visit_path_mut(self, path);
        }
    }

    let mut ty = ty.clone();
    let mut visitor = Visitor {
        generics,
        generic: false,
    };
    visitor.visit_type_mut(&mut ty);
    (!visitor.generic).then_some(ty)
}

fn sql_type_marker(sql_type: &SqlType) -> Result<TokenStream2, String> {
    match sql_type {
        SqlType::Array { element } => {
            let element = sql_type_marker(element)?;
            Ok(quote!(::tusker_query::types::PgArray<#element>))
        }
        SqlType::Composite { name, fields, .. } => {
            composites::sql_type_marker(name, fields, sql_type_marker)
        }
        SqlType::Enum { name, variants, .. } => Ok(enums::sql_type_marker(name, variants)),
        SqlType::Scalar { name, schema } => match scalar_type(name) {
            Some(scalar) => Ok(scalar.marker),
            None => {
                let mut message = format!(
                    "PostgreSQL type `{}` is not supported by tusker-query yet. Remove the `.json` sidecar to compile this query without checks.",
                    sql_type.display_name()
                );
                if schema.is_empty() {
                    message.push_str(&format!(
                        " If `{name}` is an enum type, the sidecar was generated by an older version of tusker; run `tusker query sync` to refresh it."
                    ));
                }
                Err(message)
            }
        },
    }
}

/// A supported PostgreSQL scalar type.
pub(crate) struct ScalarType {
    /// Marker type used by the type-level check.
    pub(crate) marker: TokenStream2,
    /// Human-readable list of the Rust types it maps to.
    pub(crate) rust_types: &'static str,
}

pub(crate) fn scalar_type(sql_type: &str) -> Option<ScalarType> {
    let (marker, rust_types) = match sql_type {
        "bool" => (quote!(::tusker_query::types::PgBool), "`bool`"),
        "char" => (quote!(::tusker_query::types::PgI8), "`i8`"),
        "int2" => (quote!(::tusker_query::types::PgI16), "`i16`"),
        "int4" => (quote!(::tusker_query::types::PgI32), "`i32`"),
        "int8" | "oid" => (quote!(::tusker_query::types::PgI64), "`i64`"),
        "float4" => (quote!(::tusker_query::types::PgF32), "`f32`"),
        "float8" => (quote!(::tusker_query::types::PgF64), "`f64`"),
        "numeric" => (
            quote!(::tusker_query::types::PgNumeric),
            "`rust_decimal::Decimal` (requires the `with-rust_decimal-1` feature of tusker-query)",
        ),
        "varchar" | "bpchar" | "text" | "citext" | "name" | "unknown" | "ltree" | "lquery"
        | "ltxtquery" => (
            quote!(::tusker_query::types::PgString),
            "`String`, or `&str` for parameters",
        ),
        "bytea" => (
            quote!(::tusker_query::types::PgBytea),
            "`Vec<u8>`, or `&[u8]` for parameters",
        ),
        "hstore" => (
            quote!(::tusker_query::types::PgHstore),
            "`HashMap<String, Option<String>>`",
        ),
        "timestamp" => (
            quote!(::tusker_query::types::PgTimestamp),
            "`std::time::SystemTime`, or `time::PrimitiveDateTime` (requires the `with-time-0_3` feature of tusker-query)",
        ),
        "timestamptz" => (
            quote!(::tusker_query::types::PgTimestampTz),
            "`std::time::SystemTime`, or `time::OffsetDateTime` (requires the `with-time-0_3` feature of tusker-query)",
        ),
        "inet" => (quote!(::tusker_query::types::PgInet), "`std::net::IpAddr`"),
        "date" => (
            quote!(::tusker_query::types::PgDate),
            "`time::Date` (requires the `with-time-0_3` feature of tusker-query)",
        ),
        "time" => (
            quote!(::tusker_query::types::PgTime),
            "`time::Time` (requires the `with-time-0_3` feature of tusker-query)",
        ),
        "uuid" => (
            quote!(::tusker_query::types::PgUuid),
            "`uuid::Uuid` (requires the `with-uuid-1` feature of tusker-query)",
        ),
        "json" | "jsonb" => (
            quote!(::tusker_query::types::PgJson),
            "`serde_json::Value` or `tusker_query::types::Json<T>` (requires the `with-serde_json-1` feature of tusker-query)",
        ),
        _ => return None,
    };
    Some(ScalarType { marker, rust_types })
}

#[proc_macro_derive(QueryComposite, attributes(postgres))]
/// Derives structural `tusker_query` metadata for PostgreSQL composite types.
pub fn derive_query_composite(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    match composites::expand_query_composite(&ast) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_derive(QueryEnum, attributes(postgres))]
/// Derives structural `tusker_query` metadata for PostgreSQL enum types.
pub fn derive_query_enum(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    match enums::expand_query_enum(&ast) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(FromDeriveInput)]
#[darling(supports(struct_named))]
struct FromRowTraitOpts {
    ident: syn::Ident,
}

#[proc_macro_derive(FromRow)]
/// Derives `tusker_query::FromRow` for a named struct.
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    let opts = match FromRowTraitOpts::from_derive_input(&ast) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors().into(),
    };
    let generics = ast.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let Data::Struct(s) = ast.data else {
        unreachable!();
    };
    let name = opts.ident;
    let fields = s.fields.iter().enumerate().map(|(idx, field)| {
        let field_name = &field.ident;
        quote! {
            #field_name: row.get(#idx)
        }
    });
    let field_type_assertions = s.fields.iter().enumerate().map(|(idx, field)| {
        let field_type = &field.ty;
        quote! {
            impl #impl_generics ::tusker_query::__private::RowFieldType<#idx> for #name #ty_generics #where_clause {
                type Ty = #field_type;
            }
        }
    });
    let field_count = s.fields.len();
    quote! {
        impl #impl_generics ::tusker_query::FromRow for #name #ty_generics #where_clause {
            fn from_row(row: ::tokio_postgres::Row) -> Self {
                Self {
                    #( #fields ),*
                }
            }
        }

        impl #impl_generics ::tusker_query::__private::RowFieldCount<#field_count> for #name #ty_generics #where_clause {}

        #( #field_type_assertions )*
    }
    .into()
}
