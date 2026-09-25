/// Integration tests: error samples.
///
/// Discovers every `.nt` file under `samples/errors/` and runs the
/// full transpilation pipeline on each.  Each sample file must
/// contain a comment header declaring what error is expected
/// (see [`common::extract_expect`]).
///
/// The test passes if every file produces the expected error.
mod common;

#[test]
fn all_error_samples() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples")
        .join("errors")
        .to_string_lossy()
        .to_string();
    let failures = common::run_error_batch(&dir);
    assert!(
        failures.is_empty(),
        "Error sample mismatches:\n  {}",
        failures.join("\n  ")
    );
}

fn get_err(path: &str, src: &str) -> nitid::diagnostic::Diagnostic {
    match nitid::compile(path, src, "") {
        Ok(_) => panic!("Expected compilation error but it succeeded for: {src}"),
        Err(e) => e,
    }
}

fn assert_at(err: &nitid::diagnostic::Diagnostic, file: &str, line: usize, col: Option<usize>) {
    assert_eq!(err.span.file, file, "file, got {err}");
    assert_eq!(err.span.line, line, "line, got {err}");
    if let Some(col) = col {
        assert_eq!(err.span.col, col, "col, got {err}");
    }
    if line > 0 {
        let rendered = err.to_string();
        let prefix = match col {
            Some(c) => format!("{file}:{line}:{c}:"),
            None => format!("{file}:{line}:"),
        };
        assert!(
            rendered.starts_with(&prefix),
            "Display should start with {prefix}, got {rendered}"
        );
    }
}

#[test]
fn error_messages_contain_source_span() {
    let err = get_err("test.nt", "let x = 5;");
    assert_at(&err, "test.nt", 1, Some(1));
    assert!(err.message.contains("Unexpected token"), "{err}");

    let err = get_err("test.nt", "x := 5 + \"hello\";");
    assert_at(&err, "test.nt", 1, None);
    assert!(err.message.contains("Type mismatch in arithmetic"), "{err}");

    let err = get_err("test.nt", "x := y;");
    assert_at(&err, "test.nt", 1, None);
    assert!(err.message.contains("Undefined variable 'y'"), "{err}");

    let err = get_err("test.nt", "fn foo -> int { return; }");
    assert_at(&err, "test.nt", 1, None);
    assert!(
        err.message
            .contains("Return 0 values but function expects 1"),
        "{err}"
    );

    let err = get_err("test.nt", "fn main { foo(); }");
    assert_at(&err, "test.nt", 1, None);
    assert!(err.message.contains("Undefined function 'foo'"), "{err}");

    let err = get_err("test.nt", "fn main { int x = 1; int x = 2; }");
    assert_at(&err, "test.nt", 1, None);
    assert!(
        err.message.contains("already declared in this scope"),
        "{err}"
    );

    let err = get_err("test.nt", "int x =");
    assert!(err.message.contains("Unexpected EOF"), "{err}");

    let err = get_err("test.nt", "fn 5 {}");
    assert_at(&err, "test.nt", 1, Some(4));
    assert!(err.message.contains("Expected identifier"), "{err}");

    let err = get_err("test.nt", "fn foo -> {}");
    assert_at(&err, "test.nt", 1, Some(11));
    assert!(err.message.contains("Expected type"), "{err}");

    match nitid::compile("test.nt", "fn main { int x; x = 1; }", "") {
        Ok(_) => {}
        Err(e) => panic!("Declare-then-assign should be valid, got error: {e}"),
    }

    let err = get_err("test.nt", "fn main { int x; }");
    assert_at(&err, "test.nt", 1, None);
    assert!(err.message.contains("must be initialized"), "{err}");

    let err = get_err("test.nt", "int x;");
    assert_at(&err, "test.nt", 1, None);
    assert!(err.message.contains("must be initialized"), "{err}");
}
