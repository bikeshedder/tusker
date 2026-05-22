# Tusker

Tusker is a Rust project for PostgreSQL schema diffing, migration running, and
type-safe queries.

The project contains the core building blocks behind the `tusker` CLI: a
schema diff engine, an embeddable migration runner, and a small query layer
with derive-based SQL bindings.

## Important Crates

- [`tusker`](crates/tusker/README.md): the main CLI crate for schema diffing,
  migration workflows, and query metadata tooling
- [`tusker-schema`](crates/tusker-schema/README.md): PostgreSQL schema
  inspection and diff engine
- [`tusker-migration`](crates/tusker-migration/README.md): embeddable
  PostgreSQL migration runner for SQL migration files
- [`tusker-query`](crates/tusker-query/README.md): lightweight query layer on
  top of `tokio-postgres`
- [`tusker-query-derive`](crates/tusker-query-derive/README.md): proc-macro
  derives for `tusker-query`
- [`tusker-query-models`](crates/tusker-query-models/README.md): shared models
  for checked query sidecar metadata

## Project Layout

- [`crates/`](crates/): main project crates
- [`examples/`](examples/): example applications using project crates

If you want the end-user CLI documentation, installation notes, and the
current feature/progress overview, start with
[`crates/tusker/README.md`](crates/tusker/README.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
