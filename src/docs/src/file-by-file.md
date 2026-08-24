# File-by-file breakdown

## `src/types.rs` — The Type System

**What it does:** Defines all the types that Nitid supports and how they map to C types.

```rust
enum Type {
  I8,
  I16,
  I32,
  I64,
  I128,     // signed integers
  U8,
  U16,
  U32,
  U64,
  U128,     // unsigned integers
  F32,
  F64,      // floats
  String,
  String16,
  String32, // strings
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

## `src/ast.rs` — The Abstract Syntax Tree

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

## `src/lexer.rs` — The Lexer

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

## `src/parser.rs` — The Parser

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

## `src/sema.rs` — Semantic Analysis

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

## `src/codegen.rs` — The Code Generator

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

## `src/main.rs` — The CLI Entry Point

**What it does:** Ties the pipeline together and handles command-line arguments via the `clap` library.

```rust
#[derive(Parser)]
struct Cli {
  files: Vec<String>,     // input .nt files
  c_dir: String,          // output C directory (default: "c_src")
  emit_c: bool,           // print generated C to stdout
  run: bool,              // also compile and run
  cc: Option<String>,     // C compiler to use
  output: Option<String>, // output binary name
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
