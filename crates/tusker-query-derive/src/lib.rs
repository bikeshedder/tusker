#![doc = include_str!("../README.md")]
#![deny(nonstandard_style, rust_2018_idioms)]
#![forbid(non_ascii_idents, unsafe_code)]

use std::{env, fs, path::PathBuf};

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use sha2::{Digest, Sha512};
use syn::{Data, DeriveInput};
use tusker_query_models::{Column, Query as QueryMetadata};

#[derive(FromDeriveInput)]
#[darling(attributes(query), supports(struct_named))]
struct QueryTraitOpts {
    ident: syn::Ident,
    sql: String,
    row: syn::Path,
}

#[proc_macro_derive(Query, attributes(query))]
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

    let param_assertions = field_types
        .iter()
        .zip(sidecar.params.iter())
        .enumerate()
        .map(|(idx, (field_type, sql_type))| {
            let marker = sql_type_marker(sql_type).map_err(|message| {
                syn::Error::new_spanned(
                    field_type,
                    format!(
                        "Unsupported SQL parameter type at position {}: {message}",
                        idx + 1
                    ),
                )
            })?;
            Ok(quote! {
                __assert_param_type::<#field_type, #marker>();
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let row_assertions = sidecar
        .columns
        .iter()
        .enumerate()
        .map(|(idx, column)| build_row_assertion(row, idx, column))
        .collect::<syn::Result<Vec<_>>>()?;
    let row_len = sidecar.columns.len();

    Ok(quote! {
        {
            fn __assert_param_type<T, Sql>()
            where
                T: ::tusker_query::types::QueryParamTyped<Sql>,
            {
            }

            fn __assert_row_count<Row, const N: usize>()
            where
                Row: ::tusker_query::__private::RowFieldCount<N>,
            {
            }

            fn __assert_row_type<Row, const I: usize, Sql>()
            where
                Row: ::tusker_query::__private::RowFieldType<I>,
                <Row as ::tusker_query::__private::RowFieldType<I>>::Ty:
                    ::tusker_query::types::QueryRowTyped<Sql>,
            {
            }

            fn __assert_nullable_row_type<Row, const I: usize, Sql>()
            where
                Row: ::tusker_query::__private::RowFieldType<I>,
                <Row as ::tusker_query::__private::RowFieldType<I>>::Ty:
                    ::tusker_query::types::QueryNullableRowTyped<Sql>,
            {
            }

            fn __assert_maybe_nullable_row_type<Row, const I: usize, Sql>()
            where
                Row: ::tusker_query::__private::RowFieldType<I>,
                <Row as ::tusker_query::__private::RowFieldType<I>>::Ty:
                    ::tusker_query::types::QueryMaybeNullableRowTyped<Sql>,
            {
            }

            #(#param_assertions)*
            __assert_row_count::<#row, #row_len>();
            #(#row_assertions)*
        }
    })
}

fn build_row_assertion(
    row: &syn::Path,
    index: usize,
    column: &Column,
) -> syn::Result<TokenStream2> {
    let marker = sql_type_marker(&column.r#type).map_err(|message| {
        syn::Error::new_spanned(
            row,
            format!(
                "Unsupported SQL result type for column `{}` at position {}: {message}",
                column.name,
                index + 1
            ),
        )
    })?;

    Ok(match column.notnull {
        Some(true) => {
            quote! { __assert_row_type::<#row, #index, #marker>(); }
        }
        Some(false) => {
            quote! { __assert_maybe_nullable_row_type::<#row, #index, #marker>(); }
        }
        None => {
            quote! { __assert_maybe_nullable_row_type::<#row, #index, #marker>(); }
        }
    })
}

fn sql_type_marker(sql_type: &str) -> Result<TokenStream2, String> {
    match sql_type {
        "bool" => Ok(quote!(::tusker_query::types::PgBool)),
        "char" => Ok(quote!(::tusker_query::types::PgI8)),
        "int2" => Ok(quote!(::tusker_query::types::PgI16)),
        "int4" => Ok(quote!(::tusker_query::types::PgI32)),
        "int8" | "oid" => Ok(quote!(::tusker_query::types::PgI64)),
        "float4" => Ok(quote!(::tusker_query::types::PgF32)),
        "float8" => Ok(quote!(::tusker_query::types::PgF64)),
        "varchar" | "bpchar" | "text" | "citext" | "name" | "unknown" | "ltree" | "lquery"
        | "ltxtquery" => Ok(quote!(::tusker_query::types::PgString)),
        "bytea" => Ok(quote!(::tusker_query::types::PgBytea)),
        "hstore" => Ok(quote!(::tusker_query::types::PgHstore)),
        "timestamp" => Ok(quote!(::tusker_query::types::PgTimestamp)),
        "timestamptz" => Ok(quote!(::tusker_query::types::PgTimestampTz)),
        "inet" => Ok(quote!(::tusker_query::types::PgInet)),
        "date" => Ok(quote!(::tusker_query::types::PgDate)),
        "time" => Ok(quote!(::tusker_query::types::PgTime)),
        "uuid" => Ok(quote!(::tusker_query::types::PgUuid)),
        "json" | "jsonb" => Ok(quote!(::tusker_query::types::PgJson)),
        other => Err(format!("`{other}` is not supported yet")),
    }
}

#[derive(FromDeriveInput)]
#[darling(supports(struct_named))]
struct FromRowTraitOpts {
    ident: syn::Ident,
}

#[proc_macro_derive(FromRow)]
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
