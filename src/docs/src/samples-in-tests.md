# How `samples/` Is Used in the Rust Tests

The `samples/` directory doubles as a corpus of example `.nt` programs **and** as the primary input for the Rust
integration tests. Nothing in `samples/` is hardcoded by name — the tests **discover** the files at runtime by scanning
the directory. Adding a new `.nt` file there automatically exercises it in the test suite.

The integration tests live under `tests/`:

```
tests/
├── common/mod.rs   # Shared test helpers (batch runners, expectation parsing)
├── valid.rs        # Compiles every top-level sample, expecting success
├── errors.rs       # Compiles every sample/errors/ file, expecting a stated failure
└── packages.rs     # Package/module system tests (v0.1.1–v0.1.6)
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

## Package samples — `tests/packages.rs`

The `samples/package/` directory contains multi-file package samples, each in a **version-focused subdirectory** named
after the roadmap feature it exercises (`v0.1.1-basic-import/`, `v0.1.2-qualified-access/`, etc.). Every subdirectory
that contains a `main.nt` entry point is automatically discovered and compiled.

```
samples/package/
├── main.nt                          # Existing Math/Utils cross-import demo
├── Math/                            #   └── package Math (vector.nt, matrix.nt, internal/helpers.nt)
├── Utils/                           #   └── package Utils (util.nt)
├── v0.1.1-basic-import/             # File-level import resolution
│   ├── main.nt                      #   import Foo; Foo.hello();
│   └── Foo/greet.nt                #   package Foo; fn hello()
├── v0.1.2-qualified-access/         # Foo.func() qualified calls
│   ├── main.nt                      #   Foo.add(), Foo.sub()
│   └── Foo/                         #   package Foo; arith + types (struct, enum)
├── v0.1.3-import-alias/             # import Foo as f;
│   ├── main.nt                      #   import Foo as f; f.hello();
│   └── Foo/greet.nt                #   package Foo; fn hello()
└── v0.1.4-transitive/               # A→B→C dependency chain
    ├── main.nt                      #   import A; A.foo();
    ├── A/a.nt                      #   package A; import B; fn foo() → B.bar()
    └── B/b.nt                      #   package B; fn bar()
```

### Roadmap features covered

| Version | Feature | What the tests verify |
|---------|---------|----------------------|
| **v0.1.1** | File-level import resolution | `import Foo;` finds `Foo/` dir, parses `package Foo;`, merges into symbol table |
| **v0.1.2** | Qualified access | `Foo.add()` compiles; C output contains mangled `Foo_add`; struct/enum accessible via package prefix |
| **v0.1.3** | Import aliasing | `import Foo as f;` sets alias; `f.hello()` resolves; original name `Foo` is hidden after alias |
| **v0.1.4** | Multi-file compilation | Transitive A→B→C chain compiles; C output has files for all three packages; `B_bar` and `A_foo` both mangled |
| **v0.1.5** | Name conflict detection | `parse_package_dir` rejects mismatched package declarations; `import Nonexistent` errors |
| **v0.1.6** | Mangled C names | Imported functions produce `Pkg_func` C names, not bare `func`; two packages with same function name get distinct C symbols |

### Package test helpers

Additional helpers in `common/mod.rs` (alongside the existing valid/error helpers):

| Function | Purpose |
|----------|---------|
| `discover_package_dirs()` | Find all immediate subdirs of `samples/package/` containing `main.nt` → `Vec<(name, path)>` |
| `compile_package_main(dir)` | Read `{dir}/main.nt`, run full pipeline → `Result<(Program, Vec<CFile>, cmake), String>` |
| `get_package_c_output(dir)` | Compile package main, concatenate all C file contents → `Result<String, String>` |
| `package_path(subdir)` | Resolve `samples/package/{subdir}` to an absolute path |

### Inline tests (no sample file needed)

Some package tests are defined inline in `tests/packages.rs` without a corresponding sample file:

- **`v0_1_3_alias_original_name_hidden`** — compiles `import Foo as f; Foo.hello();` inline and asserts it fails.
- **`v0_1_5_duplicate_symbol_across_imports`** — compiles two conflicting imports inline.
- **`v0_1_5_package_name_mismatch_error`** — creates a temp dir with mismatched `package` declarations, calls
  `nitid::parse_package_dir()` directly, asserts the error.
- **`build_package_context_from_parsed_files`** / **`merge_contexts_combines_packages`** — unit-test the
  `build_package_context` and `merge_contexts` APIs by parsing source strings directly.

## Shared helpers — `tests/common/mod.rs`

| Function | Purpose |
|----------|---------|
| `compile_file(path)` | Read a `.nt` file and run the full pipeline |
| `run_ok_batch(dir)` | Compile every `.nt` in a directory, expecting success |
| `extract_expect(content)` | Parse the expectation header from a `.nt` file |
| `run_error_file(path)` | Compile one error sample, check it matches its header |
| `run_error_batch(dir)` | Batch runner for error samples |
| `discover_valid_samples()` | List top-level (non-error) `.nt` files, sorted |
| `discover_package_dirs()` | Find package subdirs with `main.nt` → `Vec<(name, path)>` |
| `compile_package_main(dir)` | Compile a package's `main.nt` entry point |
| `get_package_c_output(dir)` | Compile and return concatenated C output text |
| `package_path(subdir)` | Absolute path to `samples/package/{subdir}` |

Each file is compiled through the **full four-phase pipeline** exposed as the public API `nitid::compile(path, &content, "")`
— lexing, parsing, semantic analysis, and code generation (`src/lib.rs:376`) — so every sample exercises the entire
transpiler rather than a single stage.

## Conventions for adding samples

- **Top-level `samples/*.nt`** → must compile cleanly (fails `valid.rs` otherwise).
- **`samples/errors/*.nt`** → must fail to compile; add a `// expect:` or `// expect-contains:` header (unless you
  accept any error) so `errors.rs` knows the expected failure mode.
- **`samples/package/<version-*>/`** → must compile cleanly via `main.nt` entry point (fails `packages.rs` otherwise).
  Each subdirectory should be named after the roadmap feature it tests (e.g. `v0.1.1-basic-import/`).
  Package files use `package Foo;` declarations and `import Foo;` statements.

Files are sorted by path and filtered by the `.nt` extension before batch compilation, so ordering is deterministic.
