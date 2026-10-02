# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-10-02

### Added

- Add `#[derive(QueryEnum)]` for generating structural metadata used to check PostgreSQL enum parameter and row types
- Support checked query metadata for PostgreSQL `numeric` types
- Support checked query metadata for PostgreSQL enum types

### Changed

- Improve compile errors of checked queries: they name the parameter or column, its PostgreSQL type, and the supported Rust types including the required feature flag. Enum mismatches name the missing or extra labels.
- **Breaking:** check the format version of query sidecars. Sidecars without a version or with an incompatible older version must be refreshed with `tusker query sync`, and sidecars with a newer version require upgrading tusker-query

## [0.2.0] - 2026-07-06

### Added

- Add `#[derive(QueryComposite)]` for generating structural metadata used to check PostgreSQL composite parameter and row types
- Support checked query metadata for PostgreSQL array and composite types

## [0.1.1] - 2026-05-23

### Changed

- Improve crate metadata with repository, keywords, and categories
- Clarify documentation around `.json` sidecar metadata files

## [0.1.0] - 2026-05-22

### Added

- Initial release

[unreleased]: https://github.com/bikeshedder/tusker/compare/tusker-query-derive-v0.3.0...HEAD
[0.3.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-derive-v0.3.0
[0.2.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-derive-v0.2.0
[0.1.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-derive-v0.1.1
[0.1.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-derive-v0.1.0
