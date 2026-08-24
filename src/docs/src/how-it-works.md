# Nitid Internals — A Developer's Guide

**Nitid** is the name of both the language and its **transpiler**: it reads `.nt` source files and translates them into
equivalent C code, which you then compile with any C compiler (like `gcc` or `clang`).

Think of it as a source-to-source compiler.

The language specifications are [here](specs/1.nitid-language-specs.md).

The project is written in **Rust**. If you don't know Rust, don't worry — this doc explains the Rust concepts you'll
encounter.

---

## Icons used throughout the docs

📝 **Draft** : sections marked with the _memo_ emoji are under active discussion or in draft status. This means
they aren't available yet, at least not in the `main` branch, and they may be subject to radical changes, or discarded.

---

## Project Structure

```
nitid/
├── Cargo.toml                  # Rust project config (package name, deps)
├── Makefile                    # Convenience build targets (doc, serve, open, clean)
├── src/                        # Rust sources (compiler crate)
│   ├── main.rs                 # CLI entry, pipeline orchestrator
│   ├── lib.rs                  # Library root — public API for tests
│   ├── ast.rs                  # Abstract Syntax Tree
│   ├── lexer.rs                # Lexer — source text → tokens
│   ├── parser.rs               # Parser — tokens → AST
│   ├── types.rs                # Type definitions (i8, i32, string, etc.)
│   ├── sema.rs                 # Semantic analysis — type / scope checks
│   ├── codegen.rs              # Code generator — AST → C output
│   └── docs/                   # Documentation source (mdBook project)
│       ├── book.toml           # mdBook configuration
│       └── src/                # Markdown sources for the book
│           ├── how-it-works.md # ← You are here
│           ├── SUMMARY.md
│           └── specs/          # Language spec chapters
├── runtime/                    # C runtime library shipped with generated code
│   ├── CMakeLists.txt          # Static library build config
│   ├── nitid_array.{c,h}
│   ├── nitid_string.{c,h}
│   ├── nitid_string16.{c,h}
│   ├── nitid_string32.{c,h}
│   └── tests/                  # C unit tests for runtime types
│       ├── CMakeLists.txt
│       ├── nitid_test.h        # Lightweight test harness (macros)
│       ├── test_main.c         # Test runner entry
│       ├── test_nitid_array.c
│       ├── test_nitid_string.c
│       ├── test_nitid_string16.c
│       └── test_nitid_string32.c
├── c_src/                      # Default output directory for generated C files
├── samples/                    # Example .nt programs
│   └── errors/                 # Error-sample test cases (expected failures)
├── tests/                      # Rust integration tests (batch-compile all samples)
│   ├── common/                 # Shared test helpers
│   ├── valid.rs
│   └── errors.rs
└── docs/                       # Generated documentation site (rebuild with `make`)
    ├── index.html              # mdBook output (`mdbook build -d docs src/docs/`)
    └── api/                    # Rust API docs (`cargo doc`, copied here by `make doc`)
```

---

## The Pipeline

When you run `nitid myfile.nt`, the compiler does this, in order:

```
Source code (.nt)
    │
    ▼
1. LEXER (lexer.rs)            "string" → [Token, Token, ...]
    │
    ▼
2. PARSER (parser.rs)          [Token, ...] → AST (Abstract Syntax Tree)
    │
    ▼
3. SEMANTIC ANALYSIS (sema.rs) Checks types, variable scopes, etc.
    │
    ▼
4. CODEGEN (codegen.rs)        AST → C source code
    │
    ▼
C files (.c) + CMakeLists.txt
```

Steps 3 and 4 operate on the **AST** (`ast.rs`), which is defined in its own file and used by the parser, semantic
analyzer, and code generator.

The **type system** (`types.rs`) is also shared across steps.

---

## Build Instructions

### Prerequisites

- **Rust toolchain** (rustc + cargo). Install from https://rustup.rs/:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **CMake** (>= 3.10) and a C compiler (gcc or clang) — only needed if you use `--run`

### Build Nitid

```bash
# Debug build
cargo build

# Release build (optimized, faster)
cargo build --release
```

The binary will be at `target/debug/nitid` (debug) or `target/release/nitid` (release).

### Run Nitid on a file

```bash
# Transpile only — produces .c files in c_src/
cargo run -- samples/hello.nt

# Print generated C to stdout
cargo run -- samples/hello.nt --emit-c

# Or use the built binary directly
./target/debug/nitid samples/hello.nt
```

### Transpile, compile, and run in one step

```bash
cargo run -- samples/hello.nt --run
```

This:

1. Transpiles to C in `c_src/`
2. Runs CMake to build
3. Executes the resulting binary

### Custom output directory

```bash
cargo run -- samples/hello.nt --c-dir my_output
```

### Full CLI reference

```
Usage: nitid [OPTIONS] <FILES>...

Arguments:
  <FILES>...  Source .nt files to transpile

Options:
  --c-dir <C_DIR>      Output C source directory [default: c_src]
  --emit-c             Print generated C to stdout instead of writing files
  --run                Build and run after transpiling
  --cc <CC>            C compiler to use [default: gcc]
  -o <OUTPUT>          Output binary name [default: program]
  -h, --help           Print help
  -V, --version        Print version
```

---

## C Runtime Tests

The `runtime/tests/` directory contains a lightweight C test suite for the runtime library types (`nitid_array`,
`nitid_string`, `nitid_string16`, `nitid_string32`).

### Test harness (`nitid_test.h`)

A minimal, macro-based framework — no external test library required:

| Macro                                   | Purpose                                                      |
|-----------------------------------------|--------------------------------------------------------------|
| `TEST(name)`                            | Defines a test function `static void test_name(void)`        |
| `ASSERT(cond)`                          | Fails the test if `cond` is false                            |
| `ASSERT_EQ(a, b)`                       | Fails if `a != b`                                            |
| `ASSERT_STR_EQ(a, b)`                   | Fails if `strcmp(a, b) != 0`                                 |
| `ASSERT_NULL(p)` / `ASSERT_NOT_NULL(p)` | Nullity checks                                               |
| `RUN_TEST(name)`                        | Invokes a test, prints PASS/FAIL, increments global counters |
| `TEST_GLOBALS`                          | Declares `_test_passed` / `_test_failed` counters            |
| `test_summary()`                        | Prints tally and returns exit code                           |

### Test structure

Each runtime component has its own `.c` file:

- **`test_nitid_array.c`** — `from_lit` (empty, single, multiple), `get` with negative indexing, type-specific accessors
  (i32, i64, u8, f64, bool), data independence
- **`test_nitid_string.c`** — construction (`from`, `from_n`), `clone`, `at` (valid, OOB, empty), `append` (within
  capacity, multiple, beyond capacity, large), `free` (sets null, double-free), code-point access (`at_cp` — ASCII,
  multibyte, negative), `concat`, `eq`/`ne`, `lt`/`gt`/`le`/`ge`
- **`test_nitid_string16.c`** / **`test_nitid_string32.c`** — UTF-16 / UTF-32 string operations

Each file exports a `register_*_tests()` function called from `test_main.c`, which orchestrates the runner.

### Running

```bash
cmake -S runtime -B runtime/build -DBUILD_TESTS=ON
cmake --build runtime/build
./runtime/build/tests/nitid_runtime_tests
```

The `BUILD_TESTS=ON` flag is off by default — the runtime builds as a static library alone unless tests are explicitly
enabled.

---

## Summary

| Step                  | File         | Input                | Output                                  |
|-----------------------|--------------|----------------------|-----------------------------------------|
| 1. Lexing             | `lexer.rs`   | Source text (`&str`) | `Vec<Token>`                            |
| 2. Parsing            | `parser.rs`  | `Vec<Token>`         | `Program` (AST)                         |
| 3a. Shared types      | `types.rs`   | N/A                  | `Type` enum                             |
| 3b. AST definitions   | `ast.rs`     | N/A                  | `Program`, `Decl`, `Stmt`, `Expr`, etc. |
| 4. Semantic analysis  | `sema.rs`    | `&mut Program`       | Validation + type info                  |
| 5. Code generation    | `codegen.rs` | `&Program`           | C source files                          |
| 0. Boot + orchestrate | `main.rs`    | CLI args             | Drives steps 1–5                        |

---

## Rust API Reference

Full API documentation generated from `///` comments is available at
[`api/nitid/index.html`](api/nitid/index.html). Build it with `cargo doc --no-deps` and copy the result to `docs/api`
(the `make doc` target does this for you — see the `Makefile` at the project root).
