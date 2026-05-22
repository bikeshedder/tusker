# tusker-query

`tusker-query` is a small query layer for `tokio-postgres` with derive-based
query definitions and optional compile-time validation from checked `.json`
sidecar files.

This crate provides:

- `#[derive(Query)]` for binding Rust structs to SQL files in `db/queries/`
- `#[derive(FromRow)]` for decoding rows into Rust structs
- `query()` and `query_one()` helpers on top of `tokio-postgres`
- sidecar-driven query checks similar in spirit to SQLx offline metadata

## Features

Feature | Description | Extra dependencies | Default
--- | --- | --- | ---
`with-time-0_3` | Enable typed query checks for `time` 0.3 date/time types | `time` | no
`with-uuid-1` | Enable typed query checks for `uuid` 1 types | `uuid` | no
`with-serde_json-1` | Enable typed query checks for `serde_json::Value` | `serde_json` | no

These feature flags only affect the Rust types accepted by the compile-time
query checker. If a query sidecar references a PostgreSQL type that maps to an
optional feature, the corresponding feature must be enabled in the crate that
uses `tusker-query`.

## Example

```rust
use tusker_query::{query_one, FromRow, Query};

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
}

async fn load_post(
    client: &impl tokio_postgres::GenericClient,
    id: i32,
) -> Result<Post, tokio_postgres::Error> {
    query_one(client, GetPostById { id }).await
}
```

The `Query` derive loads SQL from:

```text
db/queries/get_post_by_id.sql
```

So the SQL file for the example above would look like:

```sql
SELECT id, author, text
FROM post
WHERE id = $1
```

## How it works

`#[derive(Query)]` implements the `tusker_query::Query` trait for a named
struct. Each struct field becomes a bind parameter in declaration order.

`#[derive(FromRow)]` implements `tusker_query::FromRow` for a named struct.
Each struct field is decoded from the row by index in declaration order.

At runtime, `query()` and `query_one()` prepare the SQL, bind the values from
the query struct, execute the statement, and map the result rows through the
generated `FromRow` implementation.

## Checked query metadata

If a matching sidecar file exists next to the SQL file:

```text
db/queries/get_post_by_id.json
```

then `#[derive(Query)]` uses it at compile time to validate:

- parameter count
- parameter types
- result column count
- result column types
- basic nullability expectations
- SQL checksum freshness

If the sidecar checksum does not match the SQL file, the derive emits a compile
error asking you to refresh the metadata.

Queries without a sidecar still compile; they just skip this extra validation.

## Generating sidecars

Use the `tusker` CLI to refresh checked query metadata:

```shell
tusker query sync
```

Or for a specific glob:

```shell
tusker query sync 'db/queries/**/*.sql'
```

To inspect a single query without writing the sidecar:

```shell
tusker query inspect db/queries/get_post_by_id.sql
```

## Supported PostgreSQL type mappings

The checked query metadata currently maps common PostgreSQL types to Rust types
through marker traits in `tusker_query::types`.

Examples:

- `int4` -> `i32`
- `text`, `varchar` -> `String`, `&str`
- `bytea` -> `Vec<u8>`, `&[u8]`
- `timestamptz` -> `time::OffsetDateTime` with `with-time-0_3`
- `uuid` -> `uuid::Uuid` with `with-uuid-1`
- `json` / `jsonb` -> `serde_json::Value` with `with-serde_json-1`

This mapping is intentionally conservative. If a sidecar references a type that
is not supported yet, the derive fails with a compile error instead of quietly
accepting a potentially wrong mapping.

## Limitations

- SQL files are resolved relative to `db/queries/`
- bind parameters are matched by Rust field order
- row decoding is matched by Rust field order
- compile-time checking only runs when a `.json` sidecar exists
- the nullability signal comes from query metadata and is currently best-effort

## Relationship to `tokio-postgres`

`tusker-query` is not a replacement for `tokio-postgres`. It is a thin layer on
top of it:

- `tokio-postgres` still handles connections, prepared statements, and decoding
- `tusker-query` adds query definitions, row mapping derives, and checked
  sidecar metadata

If you already use `tokio-postgres` directly, this crate is meant to give you a
lighter-weight, file-based alternative to handwritten SQL wrappers.
