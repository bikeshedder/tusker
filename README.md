# Tusker [![Latest Version](https://img.shields.io/crates/v/tusker.svg)](https://crates.io/crates/tusker) [![Build Status](https://img.shields.io/github/actions/workflow/status/bikeshedder/tusker/rust.yml?branch=main)](https://github.com/bikeshedder/tusker/actions/workflows/rust.yml?query=branch%3Amain) ![Unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg "Unsafe forbidden") [![Rust 1.88+](https://img.shields.io/badge/rustc-1.88+-lightgray.svg "Rust 1.88+")](https://www.rust-lang.org/)

Tusker is a Rust project for PostgreSQL schema diffing, migration running, and
type-safe queries.

The project contains the core building blocks behind the `tusker` CLI: a
schema diff engine, an embeddable migration runner, and a small query layer
with derive-based SQL bindings.

## Important Crates

- [`tusker`](crates/tusker/): the main CLI crate for schema diffing,
  migration workflows, and query metadata tooling
- [`tusker-schema`](crates/tusker-schema/): PostgreSQL schema
  inspection and diff engine
- [`tusker-migration`](crates/tusker-migration/): embeddable
  PostgreSQL migration runner for SQL migration files
- [`tusker-query`](crates/tusker-query/): lightweight query layer on
  top of `tokio-postgres`
- [`tusker-query-derive`](crates/tusker-query-derive/): proc-macro
  derives for `tusker-query`
- [`tusker-query-models`](crates/tusker-query-models/): shared models
  for checked query sidecar metadata

## Project Layout

- [`crates/`](crates/): main project crates
- [`examples/`](examples/): example applications using project crates

If you want the end-user CLI documentation, installation notes, and the
current feature/progress overview, start with
[`crates/tusker/`](crates/tusker/).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
