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

/// Extract the error string from a compilation result.
fn get_err(path: &str, src: &str) -> String {
    match nitid::compile(path, src, "") {
        Ok(_) => panic!("Expected compilation error but it succeeded for: {src}"),
        Err(e) => e,
    }
}

/// Verify all error categories include `file:line:col:` source location.
#[test]
fn error_messages_contain_source_span() {
    // Parser error: unexpected keyword
    let err = get_err("test.nt", "let x = 5;");
    assert!(
        err.contains("test.nt:1:1:"),
        "Parser error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Unexpected token"),
        "Parser error should mention the token, got: {err}"
    );

    // Sema error: type mismatch in binary op
    let err = get_err("test.nt", "x := 5 + \"hello\";");
    assert!(
        err.contains("test.nt:1:"),
        "Sema arithmetic error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Type mismatch in arithmetic"),
        "Sema error should mention the mismatch, got: {err}"
    );

    // Sema error: undefined variable
    let err = get_err("test.nt", "x := y;");
    assert!(
        err.contains("test.nt:1:"),
        "Sema undefined-var error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Undefined variable 'y'"),
        "Sema error should name the variable, got: {err}"
    );

    // Sema error: return count mismatch
    let err = get_err("test.nt", "fn foo -> int { return; }");
    assert!(
        err.contains("test.nt:1:"),
        "Sema return-count error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Return 0 values but function expects 1"),
        "Sema error should describe the mismatch, got: {err}"
    );

    // Sema error: undefined function
    let err = get_err("test.nt", "fn main { foo(); }");
    assert!(
        err.contains("test.nt:1:"),
        "Sema undefined-fn error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Undefined function 'foo'"),
        "Sema error should name the function, got: {err}"
    );

    // Sema error: redeclared variable
    let err = get_err("test.nt", "fn main { int x = 1; int x = 2; }");
    assert!(
        err.contains("test.nt:1:"),
        "Sema redeclared-var error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("already declared in this scope"),
        "Sema error should mention redeclaration, got: {err}"
    );

    // Parser error: unexpected EOF
    let err = get_err("test.nt", "int x =");
    assert!(
        err.contains("Unexpected EOF"),
        "Parser EOF error should be present, got: {err}"
    );

    // Parser error via expect_ident: fn followed by literal
    let err = get_err("test.nt", "fn 5 {}");
    assert!(
        err.contains("test.nt:1:4:"),
        "Parser expect-ident error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Expected identifier"),
        "Parser error should mention expected identifier, got: {err}"
    );

    // Parser error via expect: `->` without a type
    let err = get_err("test.nt", "fn foo -> {}");
    assert!(
        err.contains("test.nt:1:11:"),
        "Parser expect-type error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("Expected type"),
        "Parser error should mention expected type, got: {err}"
    );

    // Declare-then-assign within same block: OK (not an error)
    match nitid::compile("test.nt", "fn main { int x; x = 1; }", "") {
        Ok(_) => {} // expected
        Err(e) => panic!("Declare-then-assign should be valid, got error: {e}"),
    }

    // Sema error: uninitialized variable with explicit type
    let err = get_err("test.nt", "fn main { int x; }");
    assert!(
        err.contains("test.nt:1:"),
        "Uninitialized var error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("must be initialized"),
        "Uninitialized var error should say 'must be initialized', got: {err}"
    );

    // Sema error: uninitialized variable with explicit type (no function wrapper)
    let err = get_err("test.nt", "int x;");
    assert!(
        err.contains("test.nt:1:"),
        "Global uninitialized var error should contain file:line:col:, got: {err}"
    );
    assert!(
        err.contains("must be initialized"),
        "Global uninitialized var error should say 'must be initialized', got: {err}"
    );
}
