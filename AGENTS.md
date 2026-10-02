# AGENTS.md — working on Nitid

Nitid is **both** a language and its transpiler: a Rust crate that reads `.nt` sources and emits C, plus a small C
runtime library (`runtime/`) that the generated C links against. There is no interpreter and no IR — every feature
touches the four pipeline phases directly.

When something is unclear, the docs are authoritative and hand-maintained: `README.md` (goals + status tables),
`src/docs/src/specs/*` (language spec), `src/docs/src/ROADMAP.md` (planned work), `src/docs/src/how-it-works.md`
and `file-by-file.md` (internals).

## Commands

```bash
cargo build                                  # debug binary at target/debug/nitid
cargo run -- samples/hello.nt                # transpile -> c_src/
cargo run -- samples/hello.nt --emit-c       # print generated C to stdout
cargo run -- samples/hello.nt --run          # transpile, cmake build, execute
cargo run -- samples/hello.nt --c-dir out    # custom output dir

cargo test                                   # all Rust tests
cargo test --test valid                      # samples/*.nt must compile
cargo test --test errors                     # samples/errors/*.nt must fail as documented
cargo test --test packages                   # multi-file package/module behaviour
cargo test --test grammar                    # lexer vocabulary vs. editor TextMate grammar
cargo test --no-fail-fast                    # run every suite even if one fails

cmake -S runtime -B runtime/build -DBUILD_TESTS=ON && cmake --build runtime/build
./runtime/build/tests/nitid_runtime_tests   # C unit tests for nitid_string/array/string16/string32

make                                         # docs: cargo doc + mdbook build into docs/ (committed output)
```

CLI flags: `--c-dir <dir>` (default `c_src`), `--emit-c`, `--run`, `--cc <compiler>`, `-o <binary>`.

`c_src/`, `target/`, `tmp/`, `runtime/*/build*` are gitignored generated output. `docs/` is **committed** generated
output — rerun `make` when you change anything under `src/docs/`.

## Layout

| Path | Role |
|------|------|
| `src/lexer.rs` | Source text → `Vec<Token>`. `TokenKind`, `Span`, `KEYWORD_TABLE`, `TYPES`. |
| `src/parser.rs` | Tokens → AST. Recursive descent; one function per grammar level. |
| `src/ast.rs` | `Program`, `Decl`, `Stmt`, `Expr`, `Span`, … shared by phases 2–4. |
| `src/types.rs` | `Type` enum + C type mapping (`c_str()`), array/pointer wrappers. |
| `src/sema.rs` | Scopes, type inference (`infer_expr_type`), type checks, diagnostics. |
| `src/codegen.rs` | AST → C. `generate`, `generate_c`, `emit_cmake`. |
| `src/diagnostic.rs` | `Diagnostic { phase, span, message }` + `Phase` — **the** error currency. |
| `src/lib.rs` | Public API: `compile()`, `parse_file`, `parse_package_dir`, `load_imports`, `PackageContext`. Import resolution, symbol merging, C name mangling. |
| `src/main.rs` | clap CLI; orchestrates pipeline, copies `runtime/*.{c,h}` into the output dir, writes `CMakeLists.txt`, optional `--run`. |
| `runtime/` | C runtime: `types.h`, `string.{c,h}`, `string16.*`, `string32.*`, `array.*`, plus `tests/`. |
| `samples/` | `.nt` corpus = test input. `samples/errors/` = expected failures. `samples/package/` = module-system cases. |
| `tests/` | `valid.rs`, `errors.rs`, `packages.rs`, `grammar.rs`, `common/mod.rs` helpers. |
| `editors/nitid-syntax/` | TextMate grammar (`source.nitid`) + `package.json` for JetBrains/VS Code. |
| `src/docs/` | mdBook sources; `src/docs/src/SUMMARY.md` defines the page order. |

## Pipeline invariants

`nitid::compile(path, content, c_src_dir) -> Result<(Program, Vec<CFile>, String), Diagnostic>` runs
**lex → parse → import resolution → sema → codegen**. Tests call it directly; never shell out.

- **Errors are `Diagnostic`, never `String`.** Every phase returns `Diagnostic::new(Phase::…, Span, msg)`. The phase
  drives the `In <phase> phase` prefix. Tests match on `Diagnostic::message` only (not the `Display` form), so keep
  messages stable or update the sample headers.
- **Codegen must not fail.** `lib.rs:451` does `generate(...).expect("codegen does not fail")`. Emit diagnostics from
  lex/parse/sema instead; keep codegen total.
- **Keywords live in exactly one table.** Add a reserved word to `KEYWORD_TABLE` (`lexer.rs`) and to
  `editors/nitid-syntax/syntaxes/nitid.tmLanguage.json`; `tests/grammar.rs` fails the build otherwise. Same for new
  builtin types in `lexer::TYPES` (note: `lexer::TYPES` is narrower than `types::Type` — no `i256/u256/f8/f16`, they are
  not lexable yet).
- **Module/import logic is in `lib.rs`, not `sema.rs`.** `resolve_package_dir` looks for a sibling then parent
  directory named after the package; `load_imports` does DFS with cycle detection; `build_package_context` /
  `merge_contexts` flatten a package into `PackageContext` (functions/structs/enums); imported C symbols are mangled
  `Pkg_func` (`lib.rs:435`). Codegen learns about imports through `package_names` + `foreign_sigs`.
- **Multi-return** is desugared in codegen to trailing `*res0, *res1` out-params; only works in `a, b := f()` form,
  not as a sub-expression (known gap).
- **Type inference is duplicated**: `sema::infer_expr_type` (checks) and `codegen::infer_init_type` / heuristics
  (emit). If you change one, look at the other — they are not wired together.
- **`main` is special**: the parser wraps dangling top-level statements in an implicit `fn main`; codegen forces the
  C `int main(void)` signature; `main.rs` rejects dangling code in more than one input file.
- Struct layout is already C-ABI (declaration order preserved); `packed` / `align(N)` emit accordingly.

## Adding a language feature

Touch these, in order — the feature is not done until the sample compiles:

1. `src/lexer.rs` — new `TokenKind` variant, `KEYWORD_TABLE` entry if reserved, lexing rule, `TYPES` if a type name.
2. `src/ast.rs` — node in `Expr` / `Stmt` / `Decl`, carrying a real `Span`.
3. `src/parser.rs` — parse function; wire it into the `parse_or → … → parse_primary` chain so precedence is correct,
   and into `parse_stmt` lookahead if it changes statement shape.
4. `src/sema.rs` — type inference for the new expression, scope/name checks, new `Diagnostic` on misuse.
5. `src/codegen.rs` — emit C; extend `types.rs` `c_str()` if a new C mapping is needed; update `emit_cmake` only if a
   new runtime source file must be compiled in.
6. `runtime/` — add the C type/functions if the feature needs runtime support; export it in `runtime/CMakeLists.txt`
   and copy it in `main.rs`; add `runtime/tests/test_nitid_*.c` coverage.
7. `editors/nitid-syntax/…/nitid.tmLanguage.json` — highlighting for new keywords/types/builtins.
8. `src/docs/src/specs/*` (+ `SUMMARY.md` if a new chapter) and the README status tables.
9. `samples/<feature>.nt` (must compile) and, for rejections, `samples/errors/<case>.nt` with a header.

## Tests are the sample corpus

Nothing in `samples/` is hardcoded — the suites **discover** files by scanning directories.

- `samples/*.nt` — must compile cleanly (`tests/valid.rs`, via `common::run_ok_batch`).
- `samples/errors/*.nt` — must **fail**; declare how, in a comment header:
  ```nitid
  // expect: <exact message>          // message must equal this
  // expect-contains: <substring>     // message must contain this (use when text embeds paths/lines)
  // error raised in src/sema.rs:179  // documentation only, not asserted
  ```
  First matching header line wins; no header means "any error is acceptable".
- `samples/package/<version-feature>/main.nt` — multi-file cases; the directory name should match the roadmap item
  it covers (`v0.1.1-basic-import/`, …). Some package tests are inline in `tests/packages.rs` and need no sample.

**Known pre-existing failure**: `samples/unsafe.nt` does not parse
(`mut *p = &x;` → *expected type or identifier, found star*), so `cargo test --test valid` fails on a clean
checkout. Every other suite passes (12 grammar, 22 package, 2 error). Do not assume you broke it; do not "fix" the
sample unless the task is about that syntax.

## Nitid language cheat sheet

Enough to read and write samples without reading every spec chapter:

```nitid
package main;                       // first code line of a file; also the CMake project name
import Bubble as b;                 // resolves to a ./Bubble/ directory of .nt files; b.func()

fn f(int a, b, string s) -> (i32, i32) {   // grouped params, multi-return
  return a, a * 2;                        // -> C out-params
}
fn noParens { }                        // parens optional when there are no params

a, b := f(14, "x");                   // only multi-assign form that works
v := 42;                              // := infers the type
int w = 5; w = 6;                     // declared assignment

for (i : arr) { }                    // range; for (idx, item : arr) with index; for (init; cond; step) C-style
arr[-1]                               // negative indexing counts from the end
arr.resize(200); arr.size()          // u64; `fixed` arrays are non-resizable

struct Circle { color: string; radius: f32; };   // `name: type;` order
impl Circle { fn area() -> f32 { return self.radius * self.radius; } };
c := Circle{ color: "red", radius: 1.5 };        // methods desugar to Circle_area(&c)

enum Mode : u8 { NONE, READ, WRITE };           // untyped enum == i32; compile-time overflow checks

s := "hi"; s + s; s[0];             // UTF-8 string; + concatenates, == compares, [] yields a u32 codepoint
"&x"                                 // address-of; `mut *p` / `mutable T *p` for writable pointers
unsafe { *p = 1; }                   // deref and FFI calls require unsafe
extern "C" fn SDL_Init(u32) -> int; // FFI declaration; `nil` maps to NULL
println(s); printf("%d\n", 1);       // the only builtins
```

Spec chapters: `1` syntax + EBNF, `2` functions, `3` arrays, `4` structs, `5` enums, `6` strings/Unicode,
`20` packages, `21` module layout rules, `30` builtins. Sections marked 📝 are drafts — do not implement against them
as if they were settled.

## Conventions

- **Rust style is mixed and intentional**: 4-space (rustfmt default) in `ast.rs`, `lexer.rs`, `lib.rs`, `main.rs`,
  `types.rs`, `tests/common/mod.rs`, `tests/errors.rs`, `tests/packages.rs`; 2-space in `codegen.rs`, `parser.rs`,
  `sema.rs`, `diagnostic.rs`, `tests/grammar.rs`, `tests/valid.rs`. **Match the file you edit.** There is no
  `rustfmt.toml` and `cargo fmt` is *not* clean on this repo (~11k diff lines) — never run it repo-wide.
- Doc comments (`///`) on public items are thorough and explain *why*; keep that voice. Section separators such as
  `// ── Entry point ──` are used inside large files.
- Docs are English, no preprocessor-style directives in `.nt`.
- Commit messages: emoji prefix plus terse subject — `✨ <feature>`, `📖 docs`, `🚚 rename`, `🎨 format`, or
  `Chore: <thing>`. Only commit when asked.
- No secrets, no CI config in-tree: verification is `cargo test` (plus the CMake runtime suite) and nothing is
  expected to run in CI.
