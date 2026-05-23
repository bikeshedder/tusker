# tusker-query-derive

`tusker-query-derive` provides the derive macros behind `tusker-query`.

It exports:

- `#[derive(Query)]` for binding a named Rust struct to a SQL file in `db/queries/`
- `#[derive(FromRow)]` for decoding a `tokio_postgres::Row` into a named Rust struct
- optional compile-time validation when matching `.json` sidecar metadata exists next to the SQL file

This crate is usually consumed through `tusker-query` rather than as a direct dependency.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
