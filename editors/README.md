# Editor support

`.nt` is the canonical extension for Nitid source files.

This directory holds **syntax coloring only** — no parsing, no diagnostics, no
completion. The Rust front-end remains the single source of truth for the
language; the TextMate grammar is a hand-maintained *view* of the lexer
vocabulary, kept honest by `tests/grammar.rs`.

## Layout

Everything lives in **one** folder, `nitid-syntax/`, in the VS Code extension
layout — the shape JetBrains IDEs read too:

| Path | Purpose |
|------|---------|
| `nitid-syntax/package.json` | manifest: claims `.nt`, points at the grammar, declares the editor config. **Required** at the bundle root. |
| `nitid-syntax/syntaxes/nitid.tmLanguage.json` | the grammar itself, `scopeName: source.nitid` |
| `nitid-syntax/language-configuration.json` | brackets, comment markers, auto-closing pairs, identifier pattern |

**Do not name the folder `*.tmBundle`.** JetBrains classifies a user bundle with
`BundleType.detectBundleType`, which checks the *directory name* first: anything
ending in `.tmBundle` is read as a classic TextMate bundle (needs `info.plist`),
and the `package.json` branch is never reached. See Troubleshooting.

## IntelliJ IDEA / CLion / RustRover / WebStorm

No plugin needed — the grammar ships as a TextMate bundle.

1. Make sure the bundled **TextMate Bundles** plugin is enabled:
   `Settings | Plugins | Installed | TextMate Bundles`.
2. `Settings | Editor | TextMate Bundles`, click **+**.
3. Select the `nitid-syntax` directory.
4. Apply / OK. Reopen already-open `.nt` files.

The manifest declares `".nt"` as a language extension, so `.nt` stops being
plain text and the color scheme's TextMate mapping is applied.

> Do **not** use `Settings | Plugins | Install Plugin from Disk` here. That
> expects a plugin descriptor (`plugin.xml`) and fails differently.

## VS Code

The extension has no code, just the manifest and the grammar.

- **From the UI:** `Ctrl+Shift+P` → **Extensions: Install from Location…** → pick `nitid-syntax`, then reload the
  window. (`--install-extension` on the CLI only takes a `.vsix` or a marketplace id, not a folder.)
- **To share a `.vsix`:** `npx @vscode/vsce package --out nitid.vsix` from inside `nitid-syntax/`, then
  `code --install-extension nitid.vsix`.

Or open the folder directly: `File | Open Folder… → editors/nitid-syntax`, then open any `.nt` file.

Neovim users: point `vim.filetype.add` / `syntax` at
`syntaxes/nitid.tmLanguage.json`, or use `nvim-treesitter` later — both read
plain TextMate grammars.

## Troubleshooting

**`Cannot read the following bundle: /path/to/<name>`**

JetBrains' `BundleType.detectBundleType` decides how to read a user bundle:

```kotlin
if (bundleName.endsWith(".tmBundle", ignoreCase = true)) TEXTMATE   // name decides alone
else {
  val children = resourceReader.list("").toSet()
  when {
    "package.json" in children                                       -> VSCODE   // <- this bundle
    children.any { it.endsWith(".tmLanguage") || it.endsWith(".tmPreferences") } -> SUBLIME
    children.any { it.endsWith("info.plist") }                       -> TEXTMATE
    else                                                             -> UNDEFINED
  }
}
```

`UNDEFINED` is the "Cannot read the following bundle" error. Three ways to hit
it, in the order people hit them:

1. **The directory is named `*.tmBundle`.** Then only the classic reader runs
   and the directory must contain `info.plist`; the `package.json` beside it is
   ignored. Rename the folder (ours is `nitid-syntax/`) *or* add a real
   `info.plist`. This is the one that bites even when everything else is right.
2. **No `package.json` at the root.** Sub-level, or named differently, does not
   count — detection only lists the root's immediate children.
3. **`info.json` was shipped instead of `info.plist`.** Different file, and
   JetBrains does not read it in any branch.

Check the shape without an IDE:

```sh
cargo test --test grammar jetbrains   # asserts name + root package.json
python3 -m json.tool editors/nitid-syntax/package.json > /dev/null
```

Other things that produce a blank-looking editor:

- **Already-open file, no reload.** Close and reopen the `.nt` tab.
- **Bundle added, but the old broken entry is still listed.** Remove it with
  the `-` button in `Editor | TextMate Bundles`, then re-add the directory.
- **Colors missing on one token only.** That is the grammar, not the bundle:
  see `tests/grammar.rs` and the scope list in the grammar file.
- **Plugin disabled.** `Settings | Plugins | Installed | TextMate Bundles` must
  be enabled; the whole feature disappears from settings otherwise.

## Keeping the grammar honest

`cargo test --test grammar` fails when:

- a keyword added to `KEYWORD_TABLE` (src/lexer.rs) is missing from the grammar,
- a type added to `TYPES` is missing,
- a documented builtin (`print`, `println`, `printf`) is missing,
- the grammar or the manifest is malformed JSON,
- the manifest references a file that no longer exists,
- the bundle directory is renamed to `*.tmBundle` or the root `package.json`
  disappears (JetBrains would stop reading the bundle at all).

So: add keywords to the lexer table first, then to the grammar. Never the
other way round.

## Known gaps

- **Grouped digits:** `1 000 000` and `0xff ff ff ff` are documented number
  literals (see the language spec), and the lexer returns them as a single
  token. The grammar has no way to span the spaces, so it colors each group
  separately — which is how they read anyway.
- **Identifiers** are intentionally left at the default text color; there is no
  `let`/`const` distinction to exploit yet. Names starting with an uppercase
  letter are tinted as types, by convention rather than by the spec.
- `print`, `println` and `printf` are colored as builtins only when they are
  *called* (followed by `(`), so a user function of the same name still reads
  as a plain declaration.
- Colors are cosmetic: an incorrect highlight never means your program is
  wrong. Real diagnostics arrive with the language server
  (see `src/docs/src/IDE-integration.md`, T1/T2).
