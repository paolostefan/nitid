# IDE integration (JetBrains & LSP)

**Goal:** editor support for `.nt` without rewriting the Nitid front-end in the IDE. Parsing, spans, and diagnostics
stay in the Rust compiler; JetBrains (and other editors) consume them via a thin client.

**Principle:** one analysis pipeline (compiler crate → CLI/LSP). Avoid a duplicate Grammar-Kit/Kotlin parser until the
language is stable.

**Depends on:** stable source spans (already used for errors) [1]. Cross-file navigation and completion improve a lot
after **v0.1 Module System** [1].

**Does not depend on:** memory safety (v0.2), concurrency, or a full stdlib [1].

---

## T0 — File type & syntax coloring (no semantic parse)

| Step | Deliverable                                                                                   |
|------|-----------------------------------------------------------------------------------------------|
| T0.1 | Document `.nt` as the canonical extension                                                     |
| T0.2 | TextMate grammar (`nitid.tmLanguage.json`): keywords, comments, strings, numbers, basic types |
| T0.3 | JetBrains: associate `*.nt` via TextMate Bundles or a tiny plugin that ships the bundle       |
| T0.4 | Short README: “open `.nt` in IDEA / VS Code”                                                  |

**Exit criteria:** `.nt` files open with readable highlighting in IntelliJ-based IDEs.

---

## T1 — Machine-readable diagnostics

| Step | Deliverable                                                                            |
|------|----------------------------------------------------------------------------------------|
| T1.1 | `nitid check <file> --format json` (or `--json`)                                       |
| T1.2 | Each diagnostic: file, severity, message, range (`start`/`end` line & character)       |
| T1.3 | Ranges aligned with **LSP UTF-16** offsets (important for Unicode string literals) [1] |
| T1.4 | Golden tests for JSON diagnostics                                                      |

**Exit criteria:** a single-file type/parse error is consumable by an external tool without scraping human text.

---

## T2 — Language Server MVP (`nitid-ls`)

| Step | Deliverable                                                                                 |
|------|---------------------------------------------------------------------------------------------|
| T2.1 | Binary `nitid-ls` in the compiler workspace (e.g. `tower-lsp` / `lsp-server` + `lsp-types`) |
| T2.2 | `initialize` capabilities: text sync                                                        |
| T2.3 | `textDocument/didOpen`, `didChange`, `didSave`                                              |
| T2.4 | `publishDiagnostics` from the existing lexer/parser/sema pipeline [1]                       |
| T2.5 | Full reparse per change is OK initially; debounce ~200–300 ms                               |
| T2.6 | Run analysis off the UI thread; support cancellation where possible                         |

**Exit criteria:** open a `.nt` buffer, get red squiggles for real compiler errors on edit/save.

---

## T3 — Thin JetBrains plugin (LSP client)

| Step | Deliverable                                                                             |
|------|-----------------------------------------------------------------------------------------|
| T3.1 | Plugin skeleton (IntelliJ Platform Plugin Template)                                     |
| T3.2 | Register Nitid file type for `*.nt`                                                     |
| T3.3 | Start `nitid-ls` (PATH or settings path) via platform LSP API and/or LSP4IJ             |
| T3.4 | Settings: server path, check on type vs on save, extra args                             |
| T3.5 | Install from disk / optional Marketplace later; do not require bundling native libs yet |

**Exit criteria:** stock IntelliJ/CLion user enables the plugin, points at `nitid-ls`, sees diagnostics with no manual
JSON wiring.

---

## T4 — Navigation & completion (after modules)

*Target once **v0.1** import/package resolution works [1].*

| Step | Deliverable                                                                                      |
|------|--------------------------------------------------------------------------------------------------|
| T4.1 | Per-file symbol table exposed to the LS                                                          |
| T4.2 | `textDocument/definition` (functions, structs, fields, methods, enum variants) [1]               |
| T4.3 | `textDocument/documentSymbol` (structure view)                                                   |
| T4.4 | `textDocument/hover` (type + short signature)                                                    |
| T4.5 | `textDocument/completion` (keywords + in-scope symbols; then `Foo.` qualified after imports) [1] |
| T4.6 | Multi-file: resolve `import` / aliases via package graph (v0.1.1–v0.1.6) [1]                     |

**Exit criteria:** jump to definition across two packages; completion works for local and imported names.

---

## T5 — Editor polish

| Step | Deliverable                                                                      |
|------|----------------------------------------------------------------------------------|
| T5.1 | `nitid fmt` + LSP `textDocument/formatting`                                      |
| T5.2 | Optional `textDocument/references` (project index)                               |
| T5.3 | Run configuration templates (compile/run example); link flags later for SDL/etc. |
| T5.4 | VS Code extension as a second thin LSP client (same server)                      |

**Exit criteria:** format-on-save and one-click run for a simple `.nt` program.

---

## Explicit non-goals (near term)

- Full PSI / Grammar-Kit rewrite of the Nitid parser in Kotlin
- JNI embedding of the Rust parser inside the plugin (unless packaging becomes easy later)
- Debugger story tied to generated C (defer)
- Semantic IDE features blocked on RAII / borrow checker (v0.2+) [1]

---

## Suggested implementation order (checklist)

1. TextMate grammar + `.nt` association
2. `nitid check --format json` + span convention documented
3. `nitid-ls`: diagnostics only
4. JetBrains plugin: file type + launch server + settings
5. After v0.1 modules: definition, hover, completion, cross-file [1]
6. Formatter + optional Marketplace release

---

## Design rules

- **Single source of truth:** lexer/parser/sema in Rust [1]; IDE never owns a second grammar for semantics
- **Optional fast path:** TextMate or tree-sitter only for *color*; truth remains the compiler
- **Two layers later (if ever):** LSP for everyone; native PSI only if JetBrains-deep refactor/inspect is required and
  syntax is frozen
- **Architecture fit:** keep graphics/runtime libs out of the language server; LS talks to front-end analysis only [1]

---

## Roadmap placement (suggestion)

| Tooling slice                                    | Place near                                         |
|--------------------------------------------------|----------------------------------------------------|
| T0–T3 (highlight, JSON check, LS MVP, JB plugin) | Parallel to **v0.1** or right after CLI stabilizes |
| T4 (goto/completion cross-file)                  | After **v0.1.0 Module System** exit criteria [1]   |
| T5 (fmt, refs, run configs)                      | Around **v0.4** polish / whenever fmt exists       |

---

### Optional one-liner for the main roadmap index

> **IDE / LSP:** TextMate → `nitid check --json` → `nitid-ls` diagnostics → thin JetBrains LSP plugin →
> definition/completion after modules (v0.1). No duplicate IDE parser.