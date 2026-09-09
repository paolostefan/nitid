# How `samples/` Is Used in the Rust Tests

The `samples/` directory doubles as a corpus of example `.nt` programs **and** as the primary input for the Rust
integration tests. Nothing in `samples/` is hardcoded by name — the tests **discover** the files at runtime by scanning
the directory. Adding a new `.nt` file there automatically exercises it in the test suite.

The integration tests live under `tests/`:

```
tests/
├── common/mod.rs   # Shared test helpers (batch runners, expectation parsing)
├── valid.rs        # Compiles every top-level sample, expecting success
└── errors.rs       # Compiles every sample/errors/ file, expecting a stated failure
```

## How the tests find the samples

Sample paths are resolved relative to the Cargo workspace root, not relative to the test files:

```rust
Path::new(env!("CARGO_MANIFEST_DIR")).join("samples")
```

Because the integration tests run from the crate root, `CARGO_MANIFEST_DIR` is the project root and this resolves
directly to the `samples/` directory in the repo.

## Valid samples — `tests/valid.rs`

The `all_valid_samples` test:

1. Calls `common::discover_valid_samples()` to list every `.nt` file **directly inside `samples/`** (top level only, so
   `samples/errors/` is excluded).
2. Asserts the list is non-empty.
3. Feeds the whole directory to `common::run_ok_batch()`, which compiles every file and collects failures.
4. Asserts there are no failures — i.e. **every `samples/*.nt` file must compile without errors**.

Any top-level `.nt` sample is therefore expected to be a working, compilable program (e.g. `hello.nt`,
`functions.nt`, the `structs_*` and `array*` variants).

## Error samples — `tests/errors.rs`

The `all_error_samples` test handles the `samples/errors/` subdirectory. These files are expected to **fail**
compilation, and each one states *how* it should fail via a comment header before the source:

```rust
// expect-contains: Undefined variable 'y'
// error raised in src/sema.rs:154
// All variables must be declared before use

x := y + z;
```

Two header conventions are recognized (first match in the header wins):

| Header | Meaning |
|--------|---------|
| `// expect: <exact message>` | The error string must **equal** this exactly |
| `// expect-contains: <substring>` | The error string must **contain** this substring |

The `// error raised in src/xxx.rs:N` annotation is documentation-only (it names the source location that raised the
error) and is not checked by the tests.

`common::run_error_file()` compiles each file and checks:

- If compilation **succeeds** when an error expectation is set → failure ("expected a compilation error").
- If there is **no** expectation header → any error is accepted.
- Otherwise it compares the produced error against the expected exact/substring value from the header.

## Shared helpers — `tests/common/mod.rs`

| Function | Purpose |
|----------|---------|
| `compile_file(path)` | Read a `.nt` file and run the full pipeline |
| `run_ok_batch(dir)` | Compile every `.nt` in a directory, expecting success |
| `extract_expect(content)` | Parse the expectation header from a `.nt` file |
| `run_error_file(path)` | Compile one error sample, check it matches its header |
| `run_error_batch(dir)` | Batch runner for error samples |
| `discover_valid_samples()` | List top-level (non-error) `.nt` files, sorted |

Each file is compiled through the **full four-phase pipeline** exposed as the public API `nitid::compile(path, &content, "")`
— lexing, parsing, semantic analysis, and code generation (`src/lib.rs:376`) — so every sample exercises the entire
transpiler rather than a single stage.

## Conventions for adding samples

- **Top-level `samples/*.nt`** → must compile cleanly (fails `valid.rs` otherwise).
- **`samples/errors/*.nt`** → must fail to compile; add a `// expect:` or `// expect-contains:` header (unless you
  accept any error) so `errors.rs` knows the expected failure mode.

Files are sorted by path and filtered by the `.nt` extension before batch compilation, so ordering is deterministic.
