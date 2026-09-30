use postgres_types::{FromSql, ToSql};
use tusker_query::{
    types::{
        PgArray, PgComposite, PgEnum, PgF64, PgField, PgI32, PgString, PgVariant,
        QueryMaybeNullableRowTyped, QueryNullableRowTyped, QueryParamTyped, QueryRowTyped,
    },
    QueryComposite, QueryEnum,
};

fn assert_array_param<T: QueryParamTyped<PgArray<PgI32>>>() {}
fn assert_array_row<T: QueryRowTyped<PgArray<PgI32>>>() {}
fn assert_nullable_array_row<T: QueryNullableRowTyped<PgArray<PgI32>>>() {}
fn assert_maybe_nullable_array_row<T: QueryMaybeNullableRowTyped<PgArray<PgI32>>>() {}

#[test]
fn arrays_are_supported_for_checked_params_and_rows() {
    assert_array_param::<Vec<i32>>();
    assert_array_param::<&[i32]>();
    assert_array_param::<Box<[i32]>>();
    assert_array_param::<Option<Vec<i32>>>();
    assert_array_param::<Option<&[i32]>>();
    assert_array_param::<Option<Box<[i32]>>>();
    assert_array_param::<Vec<Option<i32>>>();

    assert_array_row::<Vec<i32>>();
    assert_array_row::<Vec<Option<i32>>>();
    assert_nullable_array_row::<Option<Vec<i32>>>();
    assert_maybe_nullable_array_row::<Vec<i32>>();
    assert_maybe_nullable_array_row::<Option<Vec<i32>>>();
}

#[derive(Debug, FromSql, QueryComposite, ToSql)]
#[postgres(name = "inventory_item")]
struct InventoryItem {
    name: String,
    supplier_id: i32,
    price: Option<f64>,
}

type InventoryItemSql = PgComposite<
    { stable_name_hash("inventory_item") },
    (
        PgField<{ stable_name_hash("name") }, PgString>,
        PgField<{ stable_name_hash("supplier_id") }, PgI32>,
        PgField<{ stable_name_hash("price") }, PgF64>,
    ),
>;

const fn stable_name_hash(name: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let bytes = name.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        hash ^= bytes[idx] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        idx += 1;
    }
    hash
}

fn assert_composite_param<T: QueryParamTyped<InventoryItemSql>>() {}
fn assert_composite_row<T: QueryRowTyped<InventoryItemSql>>() {}
fn assert_maybe_nullable_composite_row<T: QueryMaybeNullableRowTyped<InventoryItemSql>>() {}

#[test]
fn query_composite_derives_structural_type_checks() {
    assert_composite_param::<InventoryItem>();
    assert_composite_param::<Option<InventoryItem>>();
    assert_composite_row::<InventoryItem>();
    assert_maybe_nullable_composite_row::<InventoryItem>();
    assert_maybe_nullable_composite_row::<Option<InventoryItem>>();
}

#[derive(Debug, FromSql, QueryEnum, ToSql)]
#[postgres(name = "group_kind", rename_all = "snake_case")]
enum GroupKind {
    Public,
    InviteOnly,
    #[postgres(name = "secret")]
    Hidden,
}

// Labels are sorted, so the marker does not depend on declaration order.
type GroupKindSql = PgEnum<
    { stable_name_hash("group_kind") },
    (
        PgVariant<{ stable_name_hash("invite_only") }>,
        PgVariant<{ stable_name_hash("public") }>,
        PgVariant<{ stable_name_hash("secret") }>,
    ),
>;

#[derive(Debug, FromSql, QueryEnum, ToSql)]
#[postgres(name = "group_kind", allow_mismatch)]
enum PartialGroupKind {
    #[postgres(name = "public")]
    Public,
}

fn assert_enum_param<T: QueryParamTyped<GroupKindSql>>() {}
fn assert_enum_row<T: QueryRowTyped<GroupKindSql>>() {}
fn assert_nullable_enum_row<T: QueryNullableRowTyped<GroupKindSql>>() {}
fn assert_maybe_nullable_enum_row<T: QueryMaybeNullableRowTyped<GroupKindSql>>() {}

#[test]
fn query_enum_derives_structural_type_checks() {
    assert_enum_param::<GroupKind>();
    assert_enum_param::<Option<GroupKind>>();
    assert_enum_row::<GroupKind>();
    assert_nullable_enum_row::<Option<GroupKind>>();
    assert_maybe_nullable_enum_row::<GroupKind>();
    assert_maybe_nullable_enum_row::<Option<GroupKind>>();
    assert_array_of_enum_param::<Vec<GroupKind>>();
}

fn assert_array_of_enum_param<T: QueryParamTyped<PgArray<GroupKindSql>>>() {}

#[test]
fn query_enum_allow_mismatch_only_checks_the_type_name() {
    assert_enum_param::<PartialGroupKind>();
    assert_enum_row::<PartialGroupKind>();
}

#[cfg(feature = "with-rust_decimal-1")]
mod numeric_tests {
    use rust_decimal_1::Decimal;
    use tusker_query::types::{
        FromSqlTyped, PgNumeric, QueryMaybeNullableRowTyped, QueryNullableRowTyped,
        QueryParamTyped, QueryRowTyped,
    };

    fn assert_param<T: QueryParamTyped<PgNumeric>>() {}
    fn assert_row<T: QueryRowTyped<PgNumeric>>() {}
    fn assert_nullable_row<T: QueryNullableRowTyped<PgNumeric>>() {}
    fn assert_maybe_nullable_row<T: QueryMaybeNullableRowTyped<PgNumeric>>() {}
    fn assert_from_sql<T>()
    where
        T: FromSqlTyped<'static, PgNumeric>,
    {
    }

    #[test]
    fn rust_decimal_is_supported_for_checked_queries() {
        assert_param::<Decimal>();
        assert_param::<Option<Decimal>>();
        assert_row::<Decimal>();
        assert_nullable_row::<Option<Decimal>>();
        assert_maybe_nullable_row::<Decimal>();
        assert_maybe_nullable_row::<Option<Decimal>>();
        assert_from_sql::<Decimal>();
    }
}

#[cfg(feature = "with-serde_json-1")]
mod json_tests {
    use tusker_query::types::{
        FromSqlTyped, PgJson, QueryMaybeNullableRowTyped, QueryNullableRowTyped, QueryParamTyped,
        QueryRowTyped,
    };

    fn assert_param<T: QueryParamTyped<PgJson>>() {}
    fn assert_row<T: QueryRowTyped<PgJson>>() {}
    fn assert_nullable_row<T: QueryNullableRowTyped<PgJson>>() {}
    fn assert_maybe_nullable_row<T: QueryMaybeNullableRowTyped<PgJson>>() {}
    fn assert_from_sql<T>()
    where
        T: FromSqlTyped<'static, PgJson>,
    {
    }

    #[test]
    fn json_wrapper_is_supported_for_checked_queries() {
        type JsonValue = tusker_query::types::Json<String>;

        assert_param::<JsonValue>();
        assert_param::<Option<JsonValue>>();
        assert_row::<JsonValue>();
        assert_nullable_row::<Option<JsonValue>>();
        assert_maybe_nullable_row::<JsonValue>();
        assert_maybe_nullable_row::<Option<JsonValue>>();
        assert_from_sql::<JsonValue>();
    }
}
