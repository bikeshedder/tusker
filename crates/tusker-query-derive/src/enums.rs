use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::{
    composites::{stable_name_hash, strip_raw_ident, tuple_type},
    overrides::Overrides,
};

pub(crate) fn sql_type_marker(name: &str, variants: &[String]) -> TokenStream2 {
    let name_hash = stable_name_hash(name);
    let variants = variant_markers(variants.iter().map(String::as_str));
    quote!(::tusker_query::types::PgEnum<#name_hash, #variants>)
}

fn variant_markers<'a>(variants: impl Iterator<Item = &'a str>) -> TokenStream2 {
    // `postgres-types` accepts an enum when the label sets match regardless of
    // declaration order, so the labels are sorted to make the generated marker
    // order-independent as well.
    let mut variants = variants.collect::<Vec<_>>();
    variants.sort_unstable();
    tuple_type(
        variants
            .into_iter()
            .map(|variant| {
                let hash = stable_name_hash(variant);
                quote!(::tusker_query::types::PgVariant<#hash>)
            })
            .collect(),
    )
}

pub(crate) fn expand_query_enum(ast: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Enum(e) = &ast.data else {
        return Err(syn::Error::new_spanned(
            ast,
            "QueryEnum can only be derived for enums",
        ));
    };

    let overrides = Overrides::extract(&ast.attrs, true)?;
    let type_name = overrides
        .name
        .unwrap_or_else(|| strip_raw_ident(&ast.ident.to_string()).to_owned());
    let type_hash = stable_name_hash(&type_name);

    let mut labels = Vec::new();
    for variant in &e.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "QueryEnum only supports enums with unit variants",
            ));
        }
        let variant_overrides = Overrides::extract(&variant.attrs, false)?;
        let label = variant_overrides.name.unwrap_or_else(|| {
            let ident_string = variant.ident.to_string();
            let name = strip_raw_ident(&ident_string);
            overrides
                .rename_all
                .map(|rule| rule.apply_to_field(name))
                .unwrap_or_else(|| name.to_owned())
        });
        labels.push(label);
    }

    let name = &ast.ident;
    let mut generics = ast.generics.clone();
    let (_, ty_generics, _) = ast.generics.split_for_impl();
    let label_markers = variant_markers(labels.iter().map(String::as_str));
    let variants = if overrides.allow_mismatch {
        // `#[postgres(allow_mismatch)]` only compares the type name at runtime,
        // so any label set is accepted here as well.
        generics.params.push(syn::parse_quote!(TuskerVariants));
        quote!(TuskerVariants)
    } else {
        label_markers.clone()
    };
    let marker = quote!(::tusker_query::types::PgEnum<#type_hash, #variants>);

    let mut param_generics = generics.clone();
    param_generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(#name #ty_generics: ::tokio_postgres::types::ToSql));
    let (param_impl_generics, _, param_where_clause) = param_generics.split_for_impl();

    let mut row_generics = generics;
    row_generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(
            #name #ty_generics: for<'a> ::tokio_postgres::types::FromSql<'a>
        ));
    let (row_impl_generics, _, row_where_clause) = row_generics.split_for_impl();

    let allow_mismatch = overrides.allow_mismatch;
    let extra_label_errors = labels.iter().map(|label| {
        format!("`{name}` maps label '{label}' which PostgreSQL enum `{type_name}` does not define")
    });
    let (impl_generics, _, where_clause) = ast.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::tusker_query::types::QueryEnumLabels for #name #ty_generics #where_clause {
            const NAME: &'static str = #type_name;
            const NAME_HASH: u64 = #type_hash;
            type Labels = #label_markers;
            const LABELS: &'static [&'static str] = &[#(#labels),*];
            const EXTRA_LABEL_ERRORS: &'static [&'static str] = &[#(#extra_label_errors),*];
            const ALLOW_MISMATCH: bool = #allow_mismatch;
        }

        impl #param_impl_generics ::tusker_query::types::QueryParamTyped<#marker>
            for #name #ty_generics #param_where_clause
        {
        }

        impl #row_impl_generics ::tusker_query::types::QueryRowTyped<#marker>
            for #name #ty_generics #row_where_clause
        {
        }

        impl #row_impl_generics ::tusker_query::types::QueryMaybeNullableRowTyped<#marker>
            for #name #ty_generics #row_where_clause
        {
        }
    })
}
