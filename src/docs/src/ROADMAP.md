# Nitid Implementation Roadmap

**Current state:** ~4200 LOC Rust + ~140 LOC C runtime. Lexer/parser/sema/codegen support scalar types, if/while,
multi-return, `for` loops (C-style + range-based), `break`/`continue`, postfix `++`/`--`, source-span errors,
no-implicit-cast enforcement, array types (fixed/dynamic), array literals, array indexing (incl. negative), range-based
array traversal, structs with methods, enums (typed/untyped with overflow checking), and a `nitid_array` runtime with
bounds-checked access.

---

## Implemented features

### Composite Types

| Feature                       | What's involved                                                                                                                                                                                                                                                                                           |
|-------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **Arrays**                    | `Type::TyArray(Box<Type>)`, `Expr::ArrayLit`, `Expr::Index`. Parser handles `Type[...]`, `fixed` keyword, declaration-level `[size]`. Sema validates element-type uniformity, non-array indexing, numeric index. Codegen emits `nitid_array_from_lit` / `nitid_array_get`. Runtime: `nitid_array` struct. |
| **For-range iteration**       | `for (item : array)` → `Stmt::ForIn`, `for (idx, item : array)` → `Stmt::ForInIndex`. Sema checks iterable is array. Codegen emits counter loop with `nitid_array_size` / `nitid_array_get`.                                                                                                              |
| **Structs**                   | `struct` keyword, parser for `struct Foo { field: Type, ... }`. AST: `Decl::StructDecl`, `ImplBlock`, `Expr::FieldAccess`, `Expr::MethodCall`, `Expr::StructLit`. Sema: field/method resolution, `self` injection. Codegen: C `struct` typedef, methods as `struct_method(&obj, args)`.                   |
| **Enums**                     | `enum` keyword, `Decl::EnumDecl`, `Type::Enum`. Parser handles `enum Name : type { A = val, ... }`. Sema validates integral types, value overflow, auto-increment, duplicate variants. Codegen emits `typedef enum`.                                                                                      |
| **Methods / dot-call syntax** | `obj.method(args)` — parser, sema, and codegen handle `.` access, method lookup, `self` pointer receiver. Codegen emits `StructName_method(&obj, args)`.                                                                                                                                                  |

---

### Strings handling

| Feature                                | What's involved                                                                                                                                                                                    |
|----------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **UTF-8 validation & Unicode escapes** | Lexer validates string literal bytes as well-formed UTF-8; rejects invalid sequences. Support `\uXXXX` and `\UXXXXXXXX` escape sequences (incl. surrogate rejection).                              |
| **String16 / String32 runtime**        | Full `nitid_string16` / `nitid_string32` C runtime with encoding conversion between all three UTF forms.                                                                                           |
| **Encoding-safe string ops**           | Code-point (not byte) indexing for all string types. Concatenation (`+`) and comparison (`==`, `!=`, `<`, `>`) operators codegen'd to runtime functions. Bounds-checked access on all three types. |

---

## Features to be implemented

### Phase 1 — Module System

| #   | Done | Feature                          | What's involved                                                                                                                                                                                                      |
|-----|------|----------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 1.1 | ✅   | **File-level import resolution** | Given `import Foo`, find files with `package Foo;` in search paths, tokenize + parse them, merge declarations into a package symbol table.                                                                           |
| 1.2 | ✅   | **Qualified access**             | `Foo.someFunc()` — parser needs to handle `Ident "." Ident` call syntax. Sema resolves against imported package.                                                                                                     |
| 1.3 | ✅   | **Import aliasing**              | `import Foo as f` → `f.someFunc()`. Already parsed, just not wired.                                                                                                                                                  |
| 1.4 | ✅   | **Multi-file compilation**       | Dependency graph via recursive DFS (`load_package`). One `.c` per source file, foreign prototypes, CMake lists all files.                                                                                            |
| 1.5 | ✅   | **Name conflict detection**      | Duplicate symbols across imports → error (keyed by real package name, not alias).                                                                                                                                    |
| 1.6 |      | **Mangled C names for imports**  | Prefix imported function names with package name in C output (`Math_multiply`). Eliminates flat-namespace collisions. `foreign_sigs` stores `(mangled_c_name, params, returns)` instead of bare `(params, returns)`. |

**Exit criteria:** `import Math; Math.sqrt(16)` works across two `.nt` files. Two packages can export identically-named
functions without C linker conflicts.

---

### Phase 2 — Memory Safety

| #   | Feature                            | What's involved                                                                                                                          |
|-----|------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------|
| 2.1 | **Scoped allocation / RAII**       | Introduce `scope` blocks (`scope { ... }`) where allocations are freed at block exit. Codegen emits `alloca` or arena with auto-cleanup. |
| 2.2 | **Safe reference types**           | `ref T` as a language type. Runtime tracks ref count or borrow region. Deref generates bounds check.                                     |
| 2.3 | **Null-pointer prevention**        | Option type pattern: `T?` cannot be dereferenced without `is` check (or Rust-style `match` when that exists).                            |
| 2.4 | **Buffer overflow runtime checks** | Wire every `a[i]` access through a bounds check. For arrays: `i < len`. For strings: `i < len`. Panic on violation.                      |

**Exit criteria:** A dangling-pointer or out-of-bounds access produces a runtime panic instead of UB.

---

### Phase 3 — Concurrency

| #   | Feature                       | Approach                                                                                                         |
|-----|-------------------------------|------------------------------------------------------------------------------------------------------------------|
| 3.1 | **Thread spawn**              | `spawn f(args)` → C `pthread_create`.                                                                            |
| 3.2 | **Channels**                  | `chan<T>` type, `send` / `recv` built-ins → C pipe or mpsc queue.                                                |
| 3.3 | **Mutex / sync primitives**   | `mutex` type wrapping `pthread_mutex_t`, scoped locking.                                                         |
| 3.4 | **Race-condition prevention** | Most complex. Borrow-checker-like analysis or runtime data-race detection (TSan instrumentation in generated C). |

**Exit criteria:** Two threads communicate over a channel without data races.

---

### Phase 4 — Standard Library & Polish

| #   | Feature                         | Notes                                                     |
|-----|---------------------------------|-----------------------------------------------------------|
| 4.1 | **`match` / `switch`**          | Pattern matching on enums and values.                     |
| 4.2 | **Standard lib: `math`**        | Trig, log, pow.                                           |
| 4.3 | **Standard lib: `collections`** | Vec, HashMap, string builder.                             |
| 4.4 | **Standard lib: `io`**          | File read/write, networking.                              |
| 4.5 | **Standard lib: `cli`**         | Simplified CLI arguments parsing                          |
| 4.5 | **`match` with destructuring**  | Advanced pattern matching (struct fields, enum variants). |

---

## Phase 5 — Nice to have

| #   | Done | Feature                        | What's involved                                                                                                                                                                                                                            |
|-----|------|--------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 5.1 |      | **Struct tags**                | Optional string metadata after each field (`age: u8 "json:\"age\""`). Lexer/parser accept tag tokens. Sema stores tags in symbol table. No runtime effect; exposed via reflection / compile-time API. See [spec](specs/4.structs.md#tags). |
| 5.2 |      | **Include file to variable**   | Ability to embed files, like what Golang does with a single-file `go:embed` directive.                                                                                                                                                     |
| 5.3 |      | **Full runtime for I256/U256** | Software-emulated 256-bit integer in C. Add `nitid_i256` / `nitid_u256` to runtime. Operator overloads for arithmetic.                                                                                                                     |
| 5.4 |      | **F8 / F16 runtime**           | 8/16-bit floats (likely `_Float16` if compiler supports, or soft-float wrapper).                                                                                                                                                           |

---

## Key architectural decisions to make

1. **Transpilation to multi-file C** — currently everything goes to one `.c`. With imports and structs, need one `.c`
   per package.

2. **Runtime library design** — currently has `nitid_string` and `nitid_array` `.c/.h` pairs. Plan grows to:
   `nitid_i256`,
   `nitid_f16`, `nitid_string16/32`, `nitid_channel`, `nitid_mutex`, `nitid_ref`. Consider a single `nitid_runtime.h`
   umbrella header.

3. **`main` auto-wrap** — currently best-effort. With packages and imports, this needs to be more deliberate (only wrap
   dangling code in the file that declares `package main` or has no `main` function).
