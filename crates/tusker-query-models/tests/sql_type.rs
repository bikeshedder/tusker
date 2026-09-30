use tusker_query_models::{
    Column, Compatibility, CompositeField, Query, QueryVersion, SqlType, Version, FORMAT_VERSION,
};

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
        version: FORMAT_VERSION,
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

#[test]
fn query_sidecar_starts_with_format_version() {
    let query = Query {
        version: FORMAT_VERSION,
        checksum: vec![0xab],
        params: vec![],
        columns: vec![],
    };

    let json = serde_json::to_string_pretty(&query).unwrap();

    assert!(json.starts_with("{\n  \"version\": \"0.1.0\",\n  \"checksum\""));
}

#[test]
fn query_version_can_be_read_from_unparseable_sidecars() {
    let versioned: QueryVersion =
        serde_json::from_str(r#"{"version": "2.1.0", "params": [{"kind": "unknown"}]}"#).unwrap();
    let unversioned: QueryVersion = serde_json::from_str(r#"{"checksum": "ab"}"#).unwrap();

    assert_eq!(versioned.version, Some(Version::new(2, 1, 0)));
    assert_eq!(unversioned.version, None);
}

#[test]
fn format_compatibility_follows_cargo_semver_rules() {
    let compat = |version: &str| Compatibility::of(Some(&Version::parse(version).unwrap()));

    assert_eq!(FORMAT_VERSION, Version::new(0, 1, 0));
    assert_eq!(compat("0.1.0"), Compatibility::Compatible);
    // In 0.x, patch versions are additive, so older readers cannot read them.
    assert_eq!(compat("0.1.1"), Compatibility::Newer);
    // Minor versions are breaking in 0.x.
    assert_eq!(compat("0.2.0"), Compatibility::Newer);
    assert_eq!(compat("0.0.9"), Compatibility::Outdated);
    assert_eq!(compat("1.0.0"), Compatibility::Newer);
    assert_eq!(Compatibility::of(None), Compatibility::Outdated);
}
