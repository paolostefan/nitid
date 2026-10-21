/// Integration tests: package error samples.
///
/// Discovers every immediate subdirectory under `samples/package-errors/` that
/// contains a `main.nt` entry point and runs the full transpilation pipeline on
/// it. Each `main.nt` must fail and declare the expected error in a comment
/// header (see [`common::extract_expect`]). Without a header any error passes.
///
/// A case lives in its own directory because imports resolve relative to the
/// importing file, so both `main.nt` and the package directories it imports
/// must sit together.
mod common;

#[test]
fn all_package_error_samples() {
    let dirs = common::discover_package_error_dirs();
    assert!(
        !dirs.is_empty(),
        "No package error dirs with main.nt found"
    );

    let failures = common::run_package_error_batch();
    assert!(
        failures.is_empty(),
        "Package error sample mismatches:\n  {}",
        failures
            .iter()
            .map(|(name, failure)| format!("{name}: {failure}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
