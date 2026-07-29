/// Integration tests: valid Nitid samples.
///
/// Discovers every `.nt` file under `samples/` and runs the full
/// transpilation pipeline on each.  The test passes if every file
/// compiles without errors.
mod common;

#[test]
fn all_valid_samples() {
    let files = common::discover_valid_samples();
    assert!(!files.is_empty(), "No valid .nt sample files found");
    let failures = common::run_ok_batch(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples").to_string_lossy(),
    );
    assert!(
        failures.is_empty(),
        "Valid sample compilation failures:\n  {}",
        failures.join("\n  ")
    );
}
