# tusker-schema

`tusker-schema` is the PostgreSQL schema inspection and diff engine behind
`tusker`.

This crate provides:

- `inspect(&Client)` for loading live PostgreSQL schema state into Rust models
- `Inspection` and per-object models for comparing schema state
- diff helpers that can turn many schema changes into ordered SQL fragments

It currently focuses on objects such as tables, columns, constraints, indexes,
routines, triggers, enums, domains, sequences, and extensions.

## Example

```rust
use tusker_schema::{diff::DiffSql, inspect, models::schema::join_sql};

async fn diff_databases(
    from: &tokio_postgres::Client,
    to: &tokio_postgres::Client,
) -> anyhow::Result<String> {
    let from = inspect(from).await?;
    let to = inspect(to).await?;
    Ok(join_sql(from.diff(&to).sql()))
}
```

## Limitations

- PostgreSQL only
- schema create/drop support is not fully implemented yet
- views and materialized views are inspected, but not currently emitted as diff SQL
- unsupported or risky changes are handled conservatively

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
