#![deny(nonstandard_style, rust_2018_idioms)]
#![forbid(non_ascii_idents, unsafe_code)]

use deadpool_postgres::Runtime;
use serde::Deserialize;
use tokio_postgres::{GenericClient, NoTls};
use tusker_query::{query_one, FromRow, Query};

#[derive(Debug, Deserialize)]
struct Config {
    pg: deadpool_postgres::Config,
}

impl Config {
    pub fn from_env() -> Result<Self, config::ConfigError> {
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
