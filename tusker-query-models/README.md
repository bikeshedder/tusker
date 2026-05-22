# tusker-query-models

`tusker-query-models` contains the shared data structures for Tusker's checked query metadata.

It defines the serde-serializable models used for `.json` sidecar files next to SQL queries, including:

- query checksums
- parameter PostgreSQL types
- result column names, types, and nullability

This crate is primarily used by `tusker-query-derive` and the tooling that reads or writes query metadata.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
