use std::{collections::HashMap, net::IpAddr, time::SystemTime};

/// This is merely a marker interface.
pub trait FromSqlTyped<'a, T> {}

/// Marker trait for Rust types accepted as bind parameters for a PostgreSQL type.
pub trait QueryParamTyped<T> {}
/// Marker trait for Rust types accepted as non-null result values for a PostgreSQL type.
pub trait QueryRowTyped<T> {}
/// Marker trait for Rust types accepted as nullable result values for a PostgreSQL type.
pub trait QueryNullableRowTyped<T> {}
/// Marker trait for Rust types accepted when query nullability is best-effort.
pub trait QueryMaybeNullableRowTyped<T> {}

/// PostgreSQL `bool`.
pub struct PgBool;
/// PostgreSQL `char`.
pub struct PgI8;
/// PostgreSQL `smallint` and `smallserial`.
pub struct PgI16;
/// PostgreSQL `int` and `serial`.
pub struct PgI32;
/// PostgreSQL `bigint`, `bigserial`, and `oid`.
pub struct PgI64;
/// PostgreSQL `real`.
pub struct PgF32;
/// PostgreSQL `double precision`.
pub struct PgF64;
/// PostgreSQL text-like string types.
pub struct PgString;
/// PostgreSQL `bytea`.
pub struct PgBytea;
/// PostgreSQL `hstore`.
pub struct PgHstore;
/// PostgreSQL `timestamp`.
pub struct PgTimestamp;
/// PostgreSQL `timestamp with time zone`.
pub struct PgTimestampTz;
/// PostgreSQL `inet`.
pub struct PgInet;
/// PostgreSQL `date`.
pub struct PgDate;
/// PostgreSQL `time`.
pub struct PgTime;
/// PostgreSQL `uuid`.
pub struct PgUuid;
/// PostgreSQL `json` and `jsonb`.
pub struct PgJson;

macro_rules! impl_query_types {
    ($( $marker:ty => param: $param:ty, row: $row:ty; )+ ) => {
        $(
            impl QueryParamTyped<$marker> for $param {}
            impl QueryParamTyped<$marker> for Option<$param> {}

            impl QueryRowTyped<$marker> for $row {}
            impl QueryNullableRowTyped<$marker> for Option<$row> {}
            impl QueryMaybeNullableRowTyped<$marker> for $row {}
            impl QueryMaybeNullableRowTyped<$marker> for Option<$row> {}
        )+
    };
}

impl QueryParamTyped<PgString> for &str {}
impl QueryParamTyped<PgString> for Option<&str> {}
impl QueryParamTyped<PgBytea> for &[u8] {}
impl QueryParamTyped<PgBytea> for Option<&[u8]> {}

impl_query_types! {
    PgBool => param: bool, row: bool;
    PgI8 => param: i8, row: i8;
    PgI16 => param: i16, row: i16;
    PgI32 => param: i32, row: i32;
    PgI64 => param: i64, row: i64;
    PgF32 => param: f32, row: f32;
    PgF64 => param: f64, row: f64;
    PgString => param: String, row: String;
    PgBytea => param: Vec<u8>, row: Vec<u8>;
    PgHstore => param: HashMap<String, Option<String>>, row: HashMap<String, Option<String>>;
    PgTimestamp => param: SystemTime, row: SystemTime;
    PgTimestampTz => param: SystemTime, row: SystemTime;
    PgInet => param: IpAddr, row: IpAddr;
}

impl FromSqlTyped<'_, PgBool> for bool {}
impl FromSqlTyped<'_, PgI8> for i8 {}
impl FromSqlTyped<'_, PgI16> for i16 {}
impl FromSqlTyped<'_, PgI32> for i32 {}
impl FromSqlTyped<'_, PgI64> for i64 {}
impl FromSqlTyped<'_, PgF32> for f32 {}
impl FromSqlTyped<'_, PgF64> for f64 {}
impl<'a> FromSqlTyped<'a, PgString> for &'a str {}
impl FromSqlTyped<'_, PgString> for String {}
impl<'a> FromSqlTyped<'a, PgBytea> for &'a [u8] {}
impl FromSqlTyped<'_, PgBytea> for Vec<u8> {}
impl FromSqlTyped<'_, PgHstore> for HashMap<String, Option<String>> {}
impl FromSqlTyped<'_, PgTimestamp> for SystemTime {}
impl FromSqlTyped<'_, PgTimestampTz> for SystemTime {}
impl FromSqlTyped<'_, PgInet> for IpAddr {}

#[cfg(feature = "with-time-0_3")]
impl_query_types! {
    PgTimestamp => param: time_03::PrimitiveDateTime, row: time_03::PrimitiveDateTime;
    PgTimestampTz => param: time_03::OffsetDateTime, row: time_03::OffsetDateTime;
    PgDate => param: time_03::Date, row: time_03::Date;
    PgTime => param: time_03::Time, row: time_03::Time;
}

#[cfg(feature = "with-time-0_3")]
impl<'a> FromSqlTyped<'a, PgTimestamp> for time_03::PrimitiveDateTime {}
#[cfg(feature = "with-time-0_3")]
impl<'a> FromSqlTyped<'a, PgTimestampTz> for time_03::OffsetDateTime {}
#[cfg(feature = "with-time-0_3")]
impl<'a> FromSqlTyped<'a, PgDate> for time_03::Date {}
#[cfg(feature = "with-time-0_3")]
impl<'a> FromSqlTyped<'a, PgTime> for time_03::Time {}

#[cfg(feature = "with-serde_json-1")]
impl_query_types! {
    PgJson => param: serde_json_1::Value, row: serde_json_1::Value;
}

#[cfg(feature = "with-serde_json-1")]
impl<'a> FromSqlTyped<'a, PgJson> for serde_json_1::Value {}

#[cfg(feature = "with-uuid-1")]
impl_query_types! {
    PgUuid => param: uuid_1::Uuid, row: uuid_1::Uuid;
}

#[cfg(feature = "with-uuid-1")]
impl<'a> FromSqlTyped<'a, PgUuid> for uuid_1::Uuid {}
