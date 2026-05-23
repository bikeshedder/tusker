# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-05-23

### Changed

- Improve crate metadata with repository, keywords, and categories
- Clarify documentation around query metadata, type information, and `.json` sidecar files

## [0.2.0] - 2026-05-23

### Added

- Support `tusker_query::types::Json<T>` for checked `json` and `jsonb` query parameters and row types
- Add a `deadpool` feature so `query()` and `query_one()` use `prepare_cached()` with `deadpool-postgres` clients

### Changed

- Make the deadpool client abstraction internal; callers should pass supported client types directly instead of naming a public `QueryClient` bound

## [0.1.0] - 2026-05-22

### Added

- Initial release

[unreleased]: https://github.com/bikeshedder/tusker/compare/tusker-query-v0.2.1...HEAD
[0.2.1]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-v0.2.1
[0.2.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-v0.2.0
[0.1.0]: https://github.com/bikeshedder/tusker/releases/tag/tusker-query-v0.1.0
