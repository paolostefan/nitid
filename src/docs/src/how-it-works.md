/e# Nitid Internals — A Developer's Guide

**Nitid** is the name of both the language and its **transpiler**: it reads `.nt` source files and translates them into
equivalent C code, which you then compile with any C compiler (like `gcc` or `clang`).

Think of it as a source-to-source compiler.

The project is written in **Rust**. If you don't know Rust, don't worry — this doc explains the Rust concepts you'll
encounter.

---

## Project Structure

```
nitid/
├── Cargo.toml              # Rust project config (package name, deps)
├── Makefile                # Convenience build targets (doc, serve, clean)
├── src/
│   ├── main.rs             # CLI entry, pipeline orchestrator
│   ├── lib.rs              # Library root — public API for tests
│   ├── ast.rs              # Abstract Syntax Tree
│   ├── lexer.rs            # Lexer — source text → tokens
│   ├── parser.rs           # Parser — tokens → AST
│   ├── types.rs            # Type definitions (i8, i32, string, etc.)
│   ├── sema.rs             # Semantic analysis — type / scope checks
│   └── codegen.rs          # Code generator — AST → C output
├── runtime/                # C runtime library shipped with generated code
│   ├── CMakeLists.txt      # Static library build config
│   ├── nitid_array.{c,h}
│   ├── nitid_string.{c,h}
│   ├── nitid_string16.{c,h}
│   ├── nitid_string32.{c,h}
│   └── tests/              # C unit tests for runtime types
│       ├── CMakeLists.txt
│       ├── nitid_test.h    # Lightweight test harness (macros)
│       ├── test_main.c     # Test runner entry
│       ├── test_nitid_array.c
│       ├── test_nitid_string.c
│       ├── test_nitid_string16.c
│       └── test_nitid_string32.c
├── c_src/                  # Default output directory for generated C files
├── samples/                # Example .nt programs
│   └── errors/             # Error-sample test cases (expected failures)
├── tests/                  # Rust integration tests (batch-compile all samples)
│   ├── valid.rs
│   └── errors.rs
└── docs/
    ├── book/               # mdBook output (generated; rebuild with `make`)
    ├── src/                # Documentation source
    │   ├── how-it-works.md # ← You are here
    │   ├── SUMMARY.md
    │   └── specs/
    └── specs/              # Standalone language spec copies
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

## Rust Concepts (for non-Rustaceans)

### `enum` (Rust's tagged union / sum type)

Like a C enum on steroids. Each variant can carry data:

```rust
enum TokenKind {
  Ident(String),      // carries a String
  IntLit(String),
  Fn,                 // carries nothing (like a plain C enum)
  Plus,
  Semicolon,
  // ...
}
```

You match on it with `match`:

```rust
match token.kind {
TokenKind::Ident(name) => { /* name is a String here */ }
TokenKind::Fn => { /* this is just a marker */ }
_ => { /* default case */ }
}
```

### `struct`

Like a C struct but with methods defined in an `impl` block:

```rust
struct Lexer {
  chars: Vec<char>,
  pos: usize,
}

impl Lexer {
  fn new(input: &str) -> Self { /* constructor */ }
  fn peek(&self) -> Option<char> { /* method */ }
  fn advance(&mut self) { /* mutating method */ }
}
```

- `&self` = read-only reference to the struct (like a `const` method in C++)
- `&mut self` = mutable reference (can modify the struct)
- `Self` = the type of the struct itself

### `Option<T>`

Rust's way of saying "maybe there's a value, maybe there isn't" — like a nullable pointer but safer:

```rust
fn peek(&self) -> Option<char> {
  self.chars.get(self.pos).copied()
}
```

Returns `Some(c)` if there's a character, or `None` if we're at the end.

### `Result<T, E>`

Either a success value `Ok(T)` or an error `Err(E)`:

```rust
fn tokenize(&mut self) -> Result<Vec<Token>, String>
```

Callers propagate errors with `?`:

```rust
let tokens = lexer.tokenize() ?;  // returns early if Err
```

### `Box<Expr>`

A pointer to heap-allocated data. Used for recursive types (like expressions containing sub-expressions):

```rust
enum Expr {
  BinaryOp {
    left: Box<Expr>,   // points to another Expr on the heap
    right: Box<Expr>,
    op: BinOp,
  },
  IntLit(i128, Span),
  // ...
}
```

If we didn't use `Box`, the type would be infinitely large (it'd contain itself).

### `#[derive(Debug, Clone)]`

Auto-implements debug printing and cloning for a type:

```rust
#[derive(Debug, Clone)]
struct Span {
  file: String,
  line: usize,
  col: usize,
}
```

- `Debug` lets you print it with `{:?}`
- `Clone` gives you a `.clone()` method to make a copy

### `HashMap<K, V>`

A hash table (dictionary / map):

```rust
let mut map: HashMap<String, Type> = HashMap::new();
map.insert("x".to_string(), Type::I32);
```

### `match`

Like C `switch` but way more powerful — can destructure, bind variables, and must be exhaustive:

```rust
match c {
'/' if self.peek_ahead(1) == Some('/') => { /* line comment */ }
'/' if self.peek_ahead(1) == Some('*') => { /* block comment */ }
'+' => { Token::new(TokenKind::Plus, self.span()) }
c if c.is_ascii_digit() => self.read_number(c),
_ => return Err("unexpected character"),
}
```

### `&` and `&mut` (references)

- `&T` = immutable reference (read-only, borrow)
- `&mut T` = mutable reference
- Rust's borrow checker enforces: either one `&mut` OR many `&`, never both at once. This prevents data races at compile
  time.

---

## File-by-file breakdown

### `src/types.rs` — The Type System

**What it does:** Defines all the types that Nitid supports and how they map to C types.

```rust
enum Type {
  I8,
  I16,
  I32,
  I64,
  I128,
  I256,   // signed integers
  U8,
  U16,
  U32,
  U64,
  U128,
  U256,   // unsigned integers
  F8,
  F16,
  F32,
  F64,                // floats
  String,
  String16,
  String32,       // strings
  Bool,
  Void,
}
```

Two key methods:

- `Type::from_str(s)` — converts `"i32"` or `"int"` into `Type::I32`. Used by the parser.
- `type.c_str()` — converts `Type::I32` into `"int"` (the C equivalent). Used by the code generator.

**Why it's its own file:** Both the parser (reading types), semantic analyzer (checking types), and codegen (writing C
types) need these definitions. Shared single source of truth.

---

### `src/ast.rs` — The Abstract Syntax Tree

**What it does:** Defines the data structures that represent parsed Nitid code as a tree. The parser produces one of
these, the semantic analyzer walks it, and the code generator walks it again to produce C.

Key structures:

- `Span` — file/line/column location info, used for error messages
- `Program` — the root node: a package name, list of imports, and list of declarations
- `Decl` — either a function declaration (`FnDecl`) or a variable declaration (`VarDecl`)
- `FnDecl` — function name, parameters, return types, body (a list of statements)
- `Param` — a parameter type + one or more names (allows `int a, b, c`)
- `VarDecl` — a variable declaration: optional type, names, optional initializer
- `Stmt` — a statement: variable decl, expression, return, block, if/else, while
- `Expr` — an expression: literals, identifiers, function calls, binary ops, assignments
- `BinOp` — binary operators (add, sub, comparisons, logical, bitwise)

**Why enums for Decl, Stmt, Expr:** Because a "declaration" can only be one thing at a time — it's either a function or
a variable. Rust enums encode this perfectly: you can't accidentally mix fields from both.

**Rust concept – `Box`:** `Expr::BinaryOp` has `left: Box<Expr>` and `right: Box<Expr>`. This is necessary because
`Expr` is recursive (an expression can contain sub-expressions). Without `Box`, Rust wouldn't know how much memory to
allocate for an `Expr`.

---

### `src/lexer.rs` — The Lexer

**What it does:** Turns raw source text (a string) into a stream of `Token`s.

```
"fn main() { return 42; }"
    │
    ▼
[Fn, Ident("main"), LParen, RParen, LBrace, Return, IntLit("42"), Semicolon, RBrace]
```

**How it works:**

1. The `Lexer` struct holds the input characters and a position cursor.
2. `tokenize()` loops through characters, skipping whitespace and comments.
3. For each character, it decides what token to produce:
    - Single-char tokens (`;`, `(`, `+`, etc.) are matched directly.
    - Two-char tokens (`==`, `!=`, `<=`, `->`, `:=`, `&&`, `||`, etc.) peek ahead.
    - `"` starts a string literal (supports escape sequences like `\n`, `\t`).
    - `'` starts a char literal.
    - Digits start a number (integer or float, decimal or hex).
    - Letters/underscore start an identifier or keyword.
    - `//` starts a line comment, `/* */` starts a block comment.

**TokenKind enum** — lists every possible token. Keywords (`fn`, `return`, `if`, `while`, `let`, `var`, `true`, `false`,
`package`, `import`, `as`) are recognized by `is_keyword()`. Type names (`i32`, `string`, `bool`, etc.) are recognized
by `is_type()` and produce `TokenKind::Type(s)` instead of `TokenKind::Ident(s)`.

**Rust concept – `Vec<T>`:** A growable array (like `std::vector` in C++ or `ArrayList` in Java). `Vec<Token>` is a list
of tokens.

---

### `src/parser.rs` — The Parser

**What it does:** Takes the flat list of tokens from the lexer and builds a tree structure (the AST).

**Parsing approach: Recursive Descent**

Each grammar rule becomes a function. Functions call each other recursively, following the language grammar:

```
parse_program()
  ├── parse_import()       // import statements
  ├── parse_fn_decl()      // function declarations
  └── parse_stmt()         // statements (also top-level)
        ├── parse_return_stmt()
        ├── parse_if_stmt()
        ├── parse_while_stmt()
        └── parse_expr()
              └── parse_assignment()
                    └── parse_or()
                          └── parse_and()
                                └── parse_bit_or()
                                      └── parse_bit_xor()
                                            └── parse_bit_and()
                                                  └── parse_equality()
                                                        └── parse_comparison()
                                                              └── parse_shift()
                                                                    └── parse_term()
                                                                          └── parse_factor()
                                                                                └── parse_unary()
                                                                                      └── parse_primary()
```

This chain **encodes operator precedence** — the deeper the function, the higher the precedence. So `parse_term()`
(add/sub) calls `parse_factor()` (mul/div), which means `*` binds tighter than `+`. Exactly like the C expression
precedence table.

- `parse_unary()` handles `-expr`, `!expr`, `~expr` (unary minus, logical not, bitwise not).
- `parse_primary()` handles literals, identifiers, function calls, and parenthesized expressions.

**Lookahead for declarations:** `parse_stmt()` has a tricky job — it needs to distinguish between:

- `int x;` (variable declaration)
- `int x = 5;` (variable declaration with init)
- `x = 5;` (assignment)
- `x := 5;` (declaration with type inference)

It uses **lookahead** (saves position, reads ahead, restores position) to figure out which case it's in.

**Implicit main function:** If there are statements at the top level that aren't inside any function, the parser wraps
them in an implicit `fn main(int argc, string argv) -> int { ... }`. This means you can write scripts without declaring
`main`.

---

### `src/sema.rs` — Semantic Analysis

**What it does:** Walks the AST and checks rules that can't be expressed in the grammar alone:

- Variables must be declared before use
- No duplicate variable names in the same scope
- Return values must match the function's declared return types
- Type compatibility in assignments and binary operations
- Built-in functions `println` and `printf` are recognized

**Scopes:**

The `Scope` struct is a chain of hash maps, like nested blocks in C:

```rust
struct Scope {
  vars: HashMap<String, Type>,
  parent: Option<Box<Scope>>,
}
```

- `Scope::new()` creates a root scope
- `Scope::child(parent)` creates a nested scope — lookups check the current scope first, then walk up to parent
- When a block ends, the child scope is discarded (variables in it "go out of scope")

**What `analyze` checks:**

For each function:

1. Records parameter types and return types (builds a function signatures table)
2. Creates a scope with parameter variables
3. Analyzes each statement in the function body:
    - `VarDecl` — checks type compatibility between declaration and initializer, declares variables in scope
    - `Expr` — infers the expression's type (recursively)
    - `Return` — checks the count and (approximately) types of return values match
    - `If` / `While` — recursively analyzes sub-blocks (creating child scopes)

**Type inference:** `infer_expr_type()` figures out what type an expression produces:

- Integer literal → `I32`
- Float literal → `F64`
- String literal → `String`
- `true`/`false` → `Bool`
- Variable reference → the type it was declared with
- Binary op → result type depends on the operator (`==` gives `Bool`, `+` matches operands)

**`:=` (declare-and-assign):** When you write `x := 42`, the type is inferred from the right-hand side. The analyzer
prints a warning but doesn't error.

**Free functions:** `analyze_var_decl_free` and `infer_expr_type_free` are standalone functions (not methods on `Sema`).
They're used for both global-level and function-level code. In Rust, you can have functions that are not attached to any
struct — they're just regular functions.

---

### `src/codegen.rs` — The Code Generator

**What it does:** Walks the AST and produces C source code as a string. This is the final stage.

Key methods:

- `generate(&program)` — the main entry point. Produces:
    - One `.c` file per input `.nt` file
    - A `CMakeLists.txt` for building everything with CMake
    - Copies runtime files (`nitid_string.h`, `nitid_string.c`)

- `emit_fn_decl(f, ...)` — generates the C function signature. Special case: `main` gets the standard
  `int main(int argc, char **argv)` signature regardless of what the Nitid code said.

**Multi-return value handling:**

Nitid functions can return multiple values (like Go or Python). C can't do this natively, so the code generator uses
**output parameters**:

 ```nitid
fn foo() -> (int, int) { return 1, 2; }
a, b := foo();
```

Becomes C:

```c
void foo(int *res0, int *res1) {
    *res0 = 1;
    *res1 = 2;
}
// caller:
int a, b;
foo(&a, &b);
```

The code generator tracks which functions have multi-return and handles this transparently.

**Built-in functions:**

- `println(s)` → `printf("%s\n", s)` or `printf("literal\n")`
- `printf(...)` → `printf(...)` (pass-through)

**CMake generation:** `emit_cmake()` produces a minimal `CMakeLists.txt` that compiles the output C file and the runtime
string library together.

**Type mapping:** `infer_expr_type_str()` maps Nitid expression types to C type strings:

- Integer → `int`
- Float → `double`
- String → `nitid_string`
- Char → `uint8_t`
- Bool → `bool`

---

### `src/main.rs` — The CLI Entry Point

**What it does:** Ties the pipeline together and handles command-line arguments via the `clap` library.

```rust
#[derive(Parser)]
struct Cli {
  files: Vec<String>,           // input .nt files
  c_dir: String,                // output C directory (default: "c_src")
  emit_c: bool,                 // print generated C to stdout
  run: bool,                    // also compile and run
  cc: Option<String>,           // C compiler to use
  output: Option<String>,       // output binary name
}
```

**Pipeline in `main()`:**

1. Creates `c_src/runtime/` output directory
2. For each input file:
   a. Read the file b. Parse it → AST c. Semantic analysis d. Code generation → C files
3. Copies runtime library files (`nitid_string.h/.c`) into output
4. Writes all `.c` files and `CMakeLists.txt`
5. If `--run` is passed:
   a. Runs `cmake -S c_src -B c_src/build`
   b. Runs `cmake --build c_src/build`
   c. Runs the resulting binary

**Rust concept – `mod`:** The `mod` declarations at the top (`mod ast; mod lexer; ...`) tell Rust to include each source
file as a module. `main.rs` is the crate root.

**Rust concept – `Result<(), String>`:** The return type of `main`. `Ok(())` means success. `Err("message")` is an
error, which Rust prints and exits with a non-zero code.

**Rust concept – `clap::Parser`:** A library that auto-generates CLI argument parsing from struct annotations.
`#[arg(short = 'o')]` means the `--output` flag also accepts `-o`.

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

## Quick Tour of Rust for reading this codebase

| If you know...             | This Rust concept                        |
|----------------------------|------------------------------------------|
| `int x = 5;`               | `let x: i32 = 5;` (immutable by default) |
| `int x = 5;` (mutable)     | `let mut x: i32 = 5;`                    |
| `const` function in C++    | `fn foo(&self) -> Type`                  |
| `void` function in C++     | `fn foo(&mut self)` (no return)          |
| `std::vector<int>`         | `Vec<i32>`                               |
| `std::map<K, V>`           | `HashMap<K, V>`                          |
| `nullptr`                  | `None` (from `Option`)                   |
| `T*` (nullable)            | `Option<Box<T>>`                         |
| `union` / `enum` + union   | `enum` with variants and data            |
| `string` (C++ std::string) | `String` (owned) or `&str` (borrowed)    |
| Exceptions                 | `Result<T, E>` with `?` operator         |
| `static_cast<int>`         | `as` keyword: `x as i32`                 |
| `#include`                 | `mod` / `use`                            |
| `// comment`               | `// comment` (same)                      |
| `/* */` block comment      | `/* */` or `///` doc comment             |
| `assert(x == 5)`           | `assert_eq!(x, 5)`                       |

### Common patterns you'll see

```rust
// Creating a new instance
let mut lexer = Lexer::new(input, file);

// Calling a method
lexer.tokenize() ?;  // ? propagates errors

// Pattern matching
match value {
Some(x) => do_something(x),
None => handle_missing(),
}

// Destructuring a struct
let Lexer { chars, pos,..} = lexer;

// Iterating
for token in & tokens { /* ... */ }

// Type ascription
let x: Vec<Token> = Vec::new();
```

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
[`api/nitid/index.html`](api/nitid/index.html). Build it with `cargo doc --no-deps` and copy the result under the book
output (see the `Makefile` at the project root).
