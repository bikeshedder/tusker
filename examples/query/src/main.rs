//! Example application that executes a checked Tusker query through a
//! `deadpool-postgres` pool.
//!
//! It shows how to load connection settings from the environment, create a
//! connection pool, and run a generated Tusker query type.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(
    nonstandard_style,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
#![forbid(non_ascii_idents, unsafe_code)]
#![warn(
    deprecated_in_future,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]
#![allow(clippy::uninlined_format_args)]

use deadpool_postgres::Runtime;
use serde::Deserialize;
use tokio_postgres::{GenericClient, NoTls};
use tusker_query::{query_one, FromRow, Query};

#[derive(Debug, Deserialize)]
struct Config {
    pg: deadpool_postgres::Config,
}

impl Config {
    fn from_env() -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }
}

#[derive(Query)]
#[query(sql = "get_post_by_id", row = Post)]
struct GetPostById {
    pub id: i32,
}

#[derive(FromRow)]
struct Post {
    pub id: i32,
    pub author: String,
    pub text: String,
    pub created: time::OffsetDateTime,
    #[allow(dead_code)]
    pub deleted: Option<time::OffsetDateTime>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().unwrap();
    let pool = cfg.pg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
    let db = pool.get().await.unwrap();
    let post = query_one(db.client(), GetPostById { id: 1 }).await.unwrap();
    println!(
        "[post.{}] {} <{}> {}",
        post.id, post.created, post.author, post.text
    );
}
