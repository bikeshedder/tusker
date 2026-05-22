#![doc = include_str!("../README.md")]
#![deny(nonstandard_style, rust_2018_idioms)]
#![forbid(non_ascii_idents, unsafe_code)]

pub mod cli;
pub mod db;
pub mod error;
pub mod file;
pub mod models;
pub mod queries;
