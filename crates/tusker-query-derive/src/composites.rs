use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_quote, Data, DeriveInput, Fields};
use tusker_query_models::{CompositeField, SqlType};

use crate::overrides::Overrides;

pub(crate) fn sql_type_marker(
    name: &str,
    fields: &[CompositeField],
    nested_sql_type_marker: fn(&SqlType) -> Result<TokenStream2, String>,
) -> Result<TokenStream2, String> {
    let name_hash = stable_name_hash(name);
    let fields = composite_field_markers(fields, nested_sql_type_marker)?;
    Ok(quote!(::tusker_query::types::PgComposite<#name_hash, #fields>))
}

fn composite_field_markers(
    fields: &[CompositeField],
    nested_sql_type_marker: fn(&SqlType) -> Result<TokenStream2, String>,
) -> Result<TokenStream2, String> {
    let fields = fields
        .iter()
        .map(|field| {
            let name_hash = stable_name_hash(&field.name);
            let field_type = nested_sql_type_marker(&field.r#type)?;
            Ok(quote!(::tusker_query::types::PgField<#name_hash, #field_type>))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(tuple_type(fields))
}

pub(crate) fn expand_query_composite(ast: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Struct(s) = &ast.data else {
        return Err(syn::Error::new_spanned(
            ast,
            "QueryComposite can only be derived for structs",
        ));
    };
    let Fields::Named(fields) = &s.fields else {
        return Err(syn::Error::new_spanned(
            ast,
            "QueryComposite can only be derived for structs with named fields",
        ));
    };

    let overrides = Overrides::extract(&ast.attrs, true)?;
    let type_name = overrides
        .name
        .unwrap_or_else(|| strip_raw_ident(&ast.ident.to_string()).to_owned());
    let type_hash = stable_name_hash(&type_name);

    let mut field_hashes = Vec::new();
    let mut field_types = Vec::new();
    for field in &fields.named {
        let ident = field.ident.as_ref().unwrap();
        let field_overrides = Overrides::extract(&field.attrs, false)?;
        let field_name = field_overrides.name.unwrap_or_else(|| {
            let ident_string = ident.to_string();
            let name = strip_raw_ident(&ident_string);
            overrides
                .rename_all
                .map(|rule| rule.apply_to_field(name))
                .unwrap_or_else(|| name.to_owned())
        });
        field_hashes.push(stable_name_hash(&field_name));
        field_types.push(field.ty.clone());
    }

    let param_impl = composite_impl(
        ast,
        type_hash,
        &field_hashes,
        &field_types,
        CompositeImpl::Param,
    );
    let row_impl = composite_impl(
        ast,
        type_hash,
        &field_hashes,
        &field_types,
        CompositeImpl::Row,
    );

    Ok(quote! {
        #param_impl
        #row_impl
    })
}

#[derive(Clone, Copy)]
enum CompositeImpl {
    Param,
    Row,
}

fn composite_impl(
    ast: &DeriveInput,
    type_hash: u64,
    field_hashes: &[u64],
    field_types: &[syn::Type],
    kind: CompositeImpl,
) -> TokenStream2 {
    let name = &ast.ident;
    let (_, ty_generics, _) = ast.generics.split_for_impl();
    let mut generics = ast.generics.clone();
    let sql_idents = (0..field_hashes.len())
        .map(|idx| syn::Ident::new(&format!("TuskerSql{idx}"), name.span()))
        .collect::<Vec<_>>();

    for ident in &sql_idents {
        generics
            .params
            .push(syn::GenericParam::Type(parse_quote!(#ident)));
    }

    {
        let where_clause = generics.make_where_clause();
        for (field_type, sql_ident) in field_types.iter().zip(sql_idents.iter()) {
            match kind {
                CompositeImpl::Param => where_clause
                    .predicates
                    .push(parse_quote!(#field_type: ::tusker_query::types::QueryParamTyped<#sql_ident>)),
                CompositeImpl::Row => where_clause
                    .predicates
                    .push(parse_quote!(#field_type: ::tusker_query::types::QueryMaybeNullableRowTyped<#sql_ident>)),
            }
        }
        match kind {
            CompositeImpl::Param => where_clause
                .predicates
                .push(parse_quote!(#name #ty_generics: ::tokio_postgres::types::ToSql)),
            CompositeImpl::Row => where_clause.predicates.push(
                parse_quote!(#name #ty_generics: for<'a> ::tokio_postgres::types::FromSql<'a>),
            ),
        }
    }

    let fields = tuple_type(
        field_hashes
            .iter()
            .zip(sql_idents.iter())
            .map(|(field_hash, sql_ident)| {
                quote!(::tusker_query::types::PgField<#field_hash, #sql_ident>)
            })
            .collect(),
    );
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    match kind {
        CompositeImpl::Param => quote! {
            impl #impl_generics ::tusker_query::types::QueryCompositeParamTyped<#type_hash, #fields>
                for #name #ty_generics #where_clause
            {
            }

            impl #impl_generics ::tusker_query::types::QueryParamTyped<
                ::tusker_query::types::PgComposite<#type_hash, #fields>
            > for #name #ty_generics #where_clause
            {
            }
        },
        CompositeImpl::Row => quote! {
            impl #impl_generics ::tusker_query::types::QueryCompositeRowTyped<#type_hash, #fields>
                for #name #ty_generics #where_clause
            {
            }

            impl #impl_generics ::tusker_query::types::QueryRowTyped<
                ::tusker_query::types::PgComposite<#type_hash, #fields>
            > for #name #ty_generics #where_clause
            {
            }

            impl #impl_generics ::tusker_query::types::QueryMaybeNullableRowTyped<
                ::tusker_query::types::PgComposite<#type_hash, #fields>
            > for #name #ty_generics #where_clause
            {
            }
        },
    }
}

pub(crate) fn tuple_type(items: Vec<TokenStream2>) -> TokenStream2 {
    match items.as_slice() {
        [] => quote!(()),
        [item] => quote!((#item,)),
        _ => quote!((#(#items),*)),
    }
}

pub(crate) fn stable_name_hash(name: &str) -> u64 {
    // Composite, field, enum, and label names have to participate in generated type markers,
    // but stable Rust const generics cannot use string literals as parameters.
    // A stable integer hash keeps structural name checks in the type system.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn strip_raw_ident(name: &str) -> &str {
    name.strip_prefix("r#").unwrap_or(name)
}
