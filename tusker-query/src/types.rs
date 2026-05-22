use std::{collections::HashMap, net::IpAddr, time::SystemTime};

/// This is merely a marker interface.
pub trait FromSqlTyped<'a, T> {}

pub trait QueryParamTyped<T> {}
pub trait QueryRowTyped<T> {}
pub trait QueryNullableRowTyped<T> {}
pub trait QueryMaybeNullableRowTyped<T> {}

/// BOOL
pub struct PgBool;
/// CHAR
pub struct PgI8;
/// SMALLINT, SMALLSERIAL
pub struct PgI16;
/// INT, SERIAL
pub struct PgI32;
/// BIGINT, BIGSERIAL, OID
pub struct PgI64;
/// REAL
pub struct PgF32;
/// DOUBLE PRECISION
pub struct PgF64;
/// VARCHAR, CHAR(n), TEXT, CITEXT, NAME, UNKNOWN, LTREE, LQUERY, LTXTQUERY
pub struct PgString;
/// BYTEA
pub struct PgBytea;
/// HSTORE
pub struct PgHstore;
/// TIMESTAMP
pub struct PgTimestamp;
/// TIMESTAMP WITH TIME ZONE
pub struct PgTimestampTz;
/// INET
pub struct PgInet;
/// DATE
pub struct PgDate;
/// TIME
pub struct PgTime;
/// UUID
pub struct PgUuid;
/// JSON, JSONB
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
