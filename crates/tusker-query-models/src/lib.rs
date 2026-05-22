#![doc = include_str!("../README.md")]
#![deny(nonstandard_style, rust_2018_idioms)]
#![forbid(non_ascii_idents, unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Query {
    #[serde(
        serialize_with = "hex::serde::serialize",
        deserialize_with = "hex::serde::deserialize"
    )]
    pub checksum: Vec<u8>,
    pub params: Vec<String>,
    pub columns: Vec<Column>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub r#type: String,
    pub notnull: Option<bool>,
}
