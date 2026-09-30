//! Compile-fail tests for the diagnostics of checked queries.
//!
//! The expected compiler output lists the Rust types implementing the marker
//! traits, which depends on the enabled features. The tests therefore only run
//! with all optional type features enabled, as in CI (`--all-features`).
#![cfg(all(
    feature = "with-time-0_3",
    feature = "with-uuid-1",
    feature = "with-serde_json-1",
    feature = "with-rust_decimal-1"
))]

use std::{fs, path::Path};

#[test]
fn ui() {
    // trybuild compiles the test cases in its own crate below the target
    // directory, so `#[derive(Query)]` looks for the queries there.
    let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).parent().unwrap();
    let queries_dir = target_dir.join("tests/trybuild/tusker-query/db/queries");
    fs::create_dir_all(&queries_dir).unwrap();
    for entry in fs::read_dir("tests/ui/db/queries").unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), queries_dir.join(entry.file_name())).unwrap();
    }

    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
