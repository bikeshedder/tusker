use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio_postgres::{Client as PgClient, Config as PgConfig, NoTls};
use uzers::get_current_username;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub schema: SchemaConfig,
    #[serde(default)]
    pub migrations: MigrationsConfig,
    #[serde(default)]
    pub diff: DiffConfig,
    #[serde(default)]
    pub queries: QueriesConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub url: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub dbname: String,
}

impl Config {
    pub fn new() -> Result<Self> {
        ::config::Config::builder()
            .add_source(::config::File::with_name("tusker.toml").required(false))
            .add_source(::config::Environment::with_prefix("TUSKER").separator("_"))
            .build()?
            .try_deserialize()
            .context("Configuration error")
    }
    pub fn template() -> Self {
        Self {
            database: DatabaseConfig {
                url: Some("".into()),
                host: Some("".into()),
                dbname: "".into(),
                password: Some("".into()),
                port: Some(5432),
                user: Some("".into()),
            },
            schema: SchemaConfig {
                filename: default_schema_filename(),
            },
            migrations: MigrationsConfig {
                filename: default_migrations_filename(),
            },
            diff: DiffConfig {
                privileges: default_diff_privileges(),
                safe: default_diff_safe(),
            },
            queries: QueriesConfig {
                filename: default_queries_filename(),
            },
        }
    }
}

impl DatabaseConfig {
    pub fn pg_config(&self) -> Result<PgConfig> {
        let mut cfg = if let Some(url) = &self.url {
            tokio_postgres::Config::from_str(url)?
        } else {
            tokio_postgres::Config::new()
        };
        if let Some(host) = &self.host {
            cfg.host(host);
        } else {
            cfg.host_path("/var/run/postgresql");
        }
        if let Some(port) = self.port {
            cfg.port(port);
        }
        if let Some(user) = &self.user {
            cfg.user(user);
        } else {
            cfg.user(
                get_current_username()
                    .with_context(|| {
                        "No database user specified. Fallback to system user name failed."
                    })?
                    .to_str()
                    .with_context(|| "System user name contains non-UTF-8 characters")?,
            );
        }
        if let Some(password) = &self.password {
            cfg.password(password);
        }
        cfg.dbname(&self.dbname);
        Ok(cfg)
    }
    pub async fn connect(&self) -> Result<PgClient> {
        let (client, connection) = self.pg_config()?.connect(NoTls).await?;
        tokio::spawn(connection);
        Ok(client)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffConfig {
    #[serde(default = "default_diff_safe")]
    pub safe: bool,
    #[serde(default = "default_diff_privileges")]
    pub privileges: bool,
}

fn default_diff_safe() -> bool {
    false
}

fn default_diff_privileges() -> bool {
    true
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            safe: default_diff_safe(),
            privileges: default_diff_privileges(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaConfig {
    #[serde(default = "default_schema_filename")]
    pub filename: String,
}

fn default_schema_filename() -> String {
    "db/schema/**/*.sql".into()
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            filename: default_schema_filename(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationsConfig {
    #[serde(default = "default_migrations_filename")]
    pub filename: String,
}

fn default_migrations_filename() -> String {
    "db/migrations/**/*.sql".into()
}

impl Default for MigrationsConfig {
    fn default() -> Self {
        Self {
            filename: default_migrations_filename(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueriesConfig {
    #[serde(default = "default_queries_filename")]
    pub filename: String,
}

fn default_queries_filename() -> String {
    "db/queries/**/*.sql".into()
}

impl Default for QueriesConfig {
    fn default() -> Self {
        Self {
            filename: default_queries_filename(),
        }
    }
}
