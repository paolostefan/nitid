#![allow(dead_code)]

/// Shared test helpers for Nitid integration tests.
///
/// Provides:
/// - [`compile_file`] — run the full pipeline on a single `.nt` file.
/// - [`run_ok_batch`] — compile every `.nt` in a directory, expecting success.
/// - [`extract_expect`] — parse error expectations from `.nt` comment headers.
/// - [`run_error_file`] — compile a single error sample and verify the error message.
/// - [`run_error_batch`] — batch runner for error samples.
/// - [`discover_valid_samples`] — list valid `.nt` sample files.
use std::path::Path;

/// What error message a test file expects.
#[derive(Debug)]
pub enum Expect {
    /// The error must match this string exactly.
    Exact(String),
    /// The error must contain this substring.
    Contains(String),
}

/// Compile a .nt file and return Ok(()) on success.
pub fn compile_file(path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))?;
    nitid::compile(path, &content, "").map(|_| ())
}

/// Run every .nt file in `dir`, expecting compilation to succeed.
/// Returns a list of failure descriptions (empty = all passed).
pub fn run_ok_batch(dir: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "nt"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in &entries {
        let path_str = entry.path().to_string_lossy().to_string();
        if let Err(e) = compile_file(&path_str) {
            failures.push(format!("{}: {}", path_str, e));
        }
    }
    failures
}

/// Extract expected error from the comment header of a .nt file.
///
/// Convention (first match wins):
///   `// expect: <exact message>`       — exact error string match
///   `// expect-contains: <substring>`  — error must contain substring
pub fn extract_expect(content: &str) -> Option<Expect> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(msg) = trimmed.strip_prefix("// expect: ") {
            return Some(Expect::Exact(msg.to_string()));
        }
        if let Some(msg) = trimmed.strip_prefix("// expect-contains: ") {
            return Some(Expect::Contains(msg.to_string()));
        }
    }
    None
}

/// Run a single error test file, checking that the compilation error matches
/// the expectation embedded in its comments.
/// Returns `Ok(())` if the error matches, or a description of what went wrong.
pub fn run_error_file(path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))?;
    let expect = extract_expect(&content);

    let result = nitid::compile(path, &content, "");
    match (result, &expect) {
        (Ok(_), _) => {
            Err(format!("Expected compilation error, but it succeeded"))
        }
        (Err(_err), None) => {
            // No expectation set — any error is accepted
            Ok(())
        }
        (Err(err), Some(exp)) => {
            let ok = match exp {
                Expect::Exact(expected) => err == *expected,
                Expect::Contains(sub) => err.contains(sub.as_str()),
            };
            if ok {
                Ok(())
            } else {
                let desc = match exp {
                    Expect::Exact(e) => format!("exact match '{}'", e),
                    Expect::Contains(c) => format!("contains '{}'", c),
                };
                Err(format!("Expected {}, got error '{}'", desc, err))
            }
        }
    }
}

/// Run every .nt file in `dir`, checking that errors match embedded
/// expectations. Returns a list of failure descriptions.
pub fn run_error_batch(dir: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "nt"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in &entries {
        let path_str = entry.path().to_string_lossy().to_string();
        if let Err(e) = run_error_file(&path_str) {
            let name = entry.path().file_name().unwrap().to_string_lossy().to_string();
            failures.push(format!("{}: {}", name, e));
        }
    }
    failures
}

/// Discover valid .nt sample files (non-error) from `samples/` directory.
/// Returns sorted list of paths.
pub fn discover_valid_samples() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "nt") {
            files.push(path.to_string_lossy().to_string());
        }
    }
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// Package test helpers
// ---------------------------------------------------------------------------

/// Discover all immediate subdirectories under `samples/package/` that
/// contain a `main.nt` entry point. Returns `(dir_name, main_path)` pairs
/// sorted by directory name.
pub fn discover_package_dirs() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples").join("package");
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let main = path.join("main.nt");
        if main.exists() {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            dirs.push((name, main.to_string_lossy().to_string()));
        }
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    dirs
}

/// Compile the `main.nt` of a package directory.
/// Returns the full `nitid::compile` result on success.
pub fn compile_package_main(dir: &str) -> Result<
    (nitid::ast::Program, Vec<nitid::codegen::CFile>, String),
    String,
> {
    let main_path = format!("{}/main.nt", dir);
    let content = std::fs::read_to_string(&main_path)
        .map_err(|e| format!("Failed to read '{}': {}", main_path, e))?;
    nitid::compile(&main_path, &content, "")
}

/// Compile a package main and return the concatenated C source text
/// of all generated `.c` files.  Useful for asserting on mangled names, etc.
pub fn get_package_c_output(dir: &str) -> Result<String, String> {
    let (_, c_files, _) = compile_package_main(dir)?;
    let mut out = String::new();
    for cf in &c_files {
        out.push_str(&cf.content);
        out.push('\n');
    }
    Ok(out)
}

/// Helper: get the full path to a subdirectory under `samples/package/`.
pub fn package_path(subdir: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples")
        .join("package")
        .join(subdir)
        .to_string_lossy()
        .to_string()
}
