use tusker_query_models::{Column, CompositeField, Query, SqlType};

#[test]
fn legacy_type_strings_deserialize_as_scalars() {
    let ty: SqlType = serde_json::from_str(r#""int4""#).unwrap();

    match ty {
        SqlType::Scalar { schema, name } => {
            assert_eq!(schema, "");
            assert_eq!(name, "int4");
        }
        _ => panic!("legacy type should deserialize as scalar"),
    }
}

#[test]
fn scalar_type_serializes_in_compact_form() {
    let ty = SqlType::scalar("pg_catalog", "int4");

    assert_eq!(serde_json::to_string(&ty).unwrap(), r#""int4""#);
}

#[test]
fn structured_array_type_round_trips() {
    let ty = SqlType::Array {
        element: Box::new(SqlType::scalar("pg_catalog", "int4")),
    };

    let json = serde_json::to_string(&ty).unwrap();
    let parsed: SqlType = serde_json::from_str(&json).unwrap();

    assert_eq!(json, r#"{"kind":"array","element":"int4"}"#);
    assert_eq!(parsed.display_name(), "int4[]");
}

#[test]
fn structured_composite_type_round_trips() {
    let ty = SqlType::Composite {
        schema: "public".to_owned(),
        name: "inventory_item".to_owned(),
        fields: vec![CompositeField {
            name: "price".to_owned(),
            r#type: SqlType::scalar("pg_catalog", "float8"),
        }],
    };

    let json = serde_json::to_string(&ty).unwrap();
    let parsed: SqlType = serde_json::from_str(&json).unwrap();

    assert!(json.contains(r#""type":"float8""#));
    assert_eq!(parsed.display_name(), "inventory_item");
}

#[test]
fn structured_enum_type_round_trips() {
    let ty = SqlType::Enum {
        schema: "public".to_owned(),
        name: "group_kind".to_owned(),
        variants: vec!["public".to_owned(), "private".to_owned()],
    };

    let json = serde_json::to_string(&ty).unwrap();
    let parsed: SqlType = serde_json::from_str(&json).unwrap();

    assert_eq!(
        json,
        r#"{"kind":"enum","schema":"public","name":"group_kind","variants":["public","private"]}"#
    );
    match parsed {
        SqlType::Enum { name, variants, .. } => {
            assert_eq!(name, "group_kind");
            assert_eq!(variants, ["public", "private"]);
        }
        _ => panic!("enum type should deserialize as enum"),
    }
}

#[test]
fn query_sidecar_keeps_scalar_params_and_columns_compact() {
    let query = Query {
        checksum: vec![0xab],
        params: vec![SqlType::scalar("pg_catalog", "int4")],
        columns: vec![Column {
            name: "id".to_owned(),
            r#type: SqlType::scalar("pg_catalog", "int4"),
            notnull: Some(true),
        }],
    };

    let json = serde_json::to_string_pretty(&query).unwrap();

    assert!(json.contains("\"params\": [\n    \"int4\"\n  ]"));
    assert!(json.contains(r#""type": "int4""#));
}
