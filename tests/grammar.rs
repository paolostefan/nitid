/// Guard tests for the editor support shipped in `editors/`.
///
/// The TextMate grammar is *derived by hand* from the lexer's vocabulary,
/// which makes it a second copy of the language's reserved words. These
/// tests fail the build when the two drift apart:
///
/// - every keyword in [`nitid::lexer::KEYWORD_TABLE`] must be highlighted,
/// - every type in [`nitid::lexer::TYPES`] must be highlighted,
/// - the grammar must be structurally valid JSON with the right scope name,
/// - the bundle must be shaped so JetBrains' TextMate importer accepts it.
///
/// The last one is not obvious and cost real debugging time. JetBrains
/// classifies a user bundle with `BundleType.detectBundleType`, which
/// short-circuits on the **directory name**: anything ending in `.tmBundle`
/// (case-insensitively) is read as a *classic* TextMate bundle and must carry
/// an `info.plist`; a `package.json` is then never even looked at, and the IDE
/// reports "Cannot read the following bundle". Only a directory *without* that
/// suffix reaches the `package.json` branch that this bundle uses.
///
/// Only the tiny bits of JSON we need are parsed here (string extraction
/// plus brace balance), to avoid adding a dev-dependency.
use std::path::{Path, PathBuf};

use nitid::lexer::{KEYWORD_TABLE, Lexer, TYPES, TokenKind};

/// Bundle root, relative to the repository root.
const BUNDLE_PATH: &str = "editors/nitid-syntax";
const GRAMMAR: &str = include_str!("../editors/nitid-syntax/syntaxes/nitid.tmLanguage.json");
const MANIFEST: &str = include_str!("../editors/nitid-syntax/package.json");

// ── JSON helpers (minimal, hand-rolled) ────────────────────────

/// Validate the overall JSON shape: balanced `{}`/`[]` outside of strings,
/// terminated strings, and no trailing commas.
fn assert_valid_json_shape(src: &str, label: &str) {
  let mut depth: i32 = 0;
  let mut in_string = false;
  let mut escaped = false;
  let mut prev_significant: Option<char> = None;

  for c in src.chars() {
    if in_string {
      if escaped {
        escaped = false;
      } else if c == '\\' {
        escaped = true;
      } else if c == '"' {
        in_string = false;
        prev_significant = Some('"');
        continue;
      } else {
        continue;
      }
    } else {
      match c {
        '"' => {
          in_string = true;
          prev_significant = Some('"');
          continue;
        }
        '{' | '[' => depth += 1,
        '}' | ']' => {
          depth -= 1;
          assert!(depth >= 0, "{}: unbalanced closing brace", label);
        }
        _ => {}
      }
    }
    if !c.is_whitespace() {
      assert!(
        !(prev_significant == Some(',') && (c == '}' || c == ']')),
        "{}: trailing comma before '{}'",
        label,
        c
      );
      prev_significant = Some(c);
    }
  }

  assert!(!in_string, "{}: unterminated string literal", label);
  assert_eq!(depth, 0, "{}: unbalanced braces", label);
  assert_eq!(
    prev_significant,
    Some('}'),
    "{}: file must end with a closing brace",
    label
  );
}

/// Extract every `"match": "..."` regex from the grammar, unescaped.
fn match_patterns(src: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut rest = src;
  while let Some(pos) = rest.find("\"match\"") {
    rest = &rest[pos + "\"match\"".len()..];
    let Some(colon) = rest.find(':') else {
      break;
    };
    rest = rest[colon + 1..].trim_start();
    if !rest.starts_with('"') {
      continue;
    }
    rest = &rest[1..];
    let mut regex = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
      match c {
        '\\' => {
          let Some(next) = chars.next() else { break };
          match next {
            // JSON escape sequences that are meaningful in JSON,
            // but not in an Oniguruma regex literal.
            'n' => regex.push('\n'),
            't' => regex.push('\t'),
            'r' => regex.push('\r'),
            other => regex.push(other),
          }
        }
        '"' => break,
        other => regex.push(other),
      }
    }
    out.push(regex);
    rest = chars.as_str();
  }
  out
}

/// True when `word` appears in `pattern` as a standalone word — the character
/// before and after each occurrence must not be an identifier character.
/// Lets us match alternations like `\b(if|else|while)\b` and `\b(...|void)\b`.
/// Zero-width `\b` / `\B` assertions are stripped first, since they are not
/// identifier characters and would otherwise hide a word boundary.
fn pattern_mentions_word(pattern: &str, word: &str) -> bool {
  let cleaned = pattern.replace("\\b", "").replace("\\B", "");
  let is_word_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
  let bytes: Vec<char> = cleaned.chars().collect();
  let needle: Vec<char> = word.chars().collect();

  for start in 0..bytes.len() {
    if bytes[start..].starts_with(&needle[..]) {
      let before_ok = start == 0 || !is_word_char(bytes[start - 1]);
      let after = start + needle.len();
      let after_ok = after >= bytes.len() || !is_word_char(bytes[after]);
      if before_ok && after_ok {
        return true;
      }
    }
  }
  false
}

fn any_pattern_mentions(patterns: &[String], word: &str) -> bool {
  patterns.iter().any(|p| pattern_mentions_word(p, word))
}

/// Read a top-level string field, e.g. `json_string_field(src, "scopeName")`.
fn json_string_field(src: &str, key: &str) -> Option<String> {
  let needle = format!("\"{}\"", key);
  let pos = src.find(&needle)? + needle.len();
  let rest = src[pos..].trim_start();
  let rest = rest.strip_prefix(':')?.trim_start();
  let body = rest.strip_prefix('"')?;
  let mut value = String::new();
  let mut chars = body.chars();
  while let Some(c) = chars.next() {
    match c {
      '\\' => value.push(chars.next()?),
      '"' => return Some(value),
      other => value.push(other),
    }
  }
  None
}

// ── Tests ──────────────────────────────────────────────────────

#[test]
fn grammar_is_structurally_valid_json() {
  assert_valid_json_shape(GRAMMAR, "nitid.tmLanguage.json");
}

#[test]
fn bundle_manifest_is_structurally_valid_json() {
  assert_valid_json_shape(MANIFEST, "package.json");
}

#[test]
fn jetbrains_will_pick_the_vscode_reader_for_this_bundle() {
  // Mirrors `BundleType.detectBundleType` in the IntelliJ TextMate plugin.
  let dir = bundle_dir();
  let name = dir
      .file_name()
      .expect("bundle dir must have a name")
      .to_string_lossy()
      .to_lowercase();

  assert!(
    !name.ends_with(".tmbundle"),
    "the bundle directory must not end in .tmBundle: JetBrains would read it as a classic \
         TextMate bundle, demand an info.plist, and report \"Cannot read the following bundle\""
  );

  let root_files: Vec<String> = std::fs::read_dir(&dir)
      .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
      .filter_map(|e| e.ok())
      .filter(|e| e.path().is_file())
      .map(|e| e.file_name().to_string_lossy().to_string())
      .collect();

  assert!(
    root_files.contains(&"package.json".to_string()),
    "package.json must sit at the bundle root, otherwise JetBrains reports \
         \"Cannot read the following bundle\". Found: {:?}",
    root_files
  );
}

#[test]
fn grammar_declares_nitid_scope_and_file_type() {
  assert_eq!(
    json_string_field(GRAMMAR, "scopeName").as_deref(),
    Some("source.nitid"),
    "grammar must declare scopeName source.nitid"
  );
  assert_eq!(
    json_string_field(GRAMMAR, "name").as_deref(),
    Some("Nitid"),
    "grammar must declare a human readable name"
  );
  assert!(
    GRAMMAR.contains("\"fileTypes\": [\"nt\"]"),
    "grammar must register the .nt file type"
  );
}

#[test]
fn grammar_highlights_every_keyword() {
  let patterns = match_patterns(GRAMMAR);
  assert!(
    patterns.len() > 10,
    "expected the grammar to contain many match rules, found {}",
    patterns.len()
  );
  let missing: Vec<&str> = KEYWORD_TABLE
      .iter()
      .map(|(kw, _)| *kw)
      .filter(|kw| !any_pattern_mentions(&patterns, kw))
      .collect();
  assert!(
    missing.is_empty(),
    "keywords missing from nitid.tmLanguage.json: {:?}",
    missing
  );
}

#[test]
fn grammar_highlights_every_type() {
  let patterns = match_patterns(GRAMMAR);
  let missing: Vec<&str> = TYPES
      .iter()
      .copied()
      .filter(|ty| !any_pattern_mentions(&patterns, ty))
      .collect();
  assert!(
    missing.is_empty(),
    "types missing from nitid.tmLanguage.json: {:?}",
    missing
  );
}

#[test]
fn grammar_highlights_the_documented_builtins() {
  // Kept in sync with src/docs/src/specs/30.builtins.md.
  let patterns = match_patterns(GRAMMAR);
  for builtin in ["print", "println", "printf"] {
    assert!(
      any_pattern_mentions(&patterns, builtin),
      "builtin {} missing from nitid.tmLanguage.json",
      builtin
    );
  }
}

#[test]
fn bundle_manifest_claims_nt_and_links_the_grammar() {
  assert!(
    MANIFEST.contains("\".nt\""),
    "the bundle manifest must claim the .nt extension"
  );
  assert_eq!(
    json_string_field(MANIFEST, "scopeName").as_deref(),
    Some("source.nitid"),
    "the bundle manifest must point the grammar at scope source.nitid"
  );

  let grammar = bundle_dir().join("syntaxes/nitid.tmLanguage.json");
  assert!(
    grammar.exists(),
    "the bundle manifest points at a missing grammar: {}",
    grammar.display()
  );
  let linked = std::fs::read_to_string(&grammar)
      .unwrap_or_else(|e| panic!("cannot read {}: {}", grammar.display(), e));
  assert_eq!(linked, GRAMMAR, "manifest grammar path is stale");
}

#[test]
fn bundle_manifest_references_existing_files() {
  for referenced in referenced_paths(MANIFEST) {
    let target: PathBuf = bundle_dir().join(&referenced);
    assert!(
      target.exists(),
      "package.json references {} which does not exist",
      target.display()
    );
  }
}

/// Root of the shipped bundle.
fn bundle_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join(BUNDLE_PATH)
}

/// Collect the `"./..."` paths referenced by the manifest.
fn referenced_paths(manifest: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut rest = manifest;
  while let Some(pos) = rest.find("\"./") {
    rest = &rest[pos + "\"./".len()..];
    match rest.find('"') {
      Some(end) => {
        out.push(rest[..end].to_string());
        rest = &rest[end + 1..];
      }
      None => break,
    }
  }
  out
}

#[test]
fn keyword_table_has_no_duplicates() {
  let mut seen: Vec<&str> = KEYWORD_TABLE.iter().map(|(kw, _)| *kw).collect();
  let total = seen.len();
  seen.sort_unstable();
  seen.dedup();
  assert_eq!(seen.len(), total, "KEYWORD_TABLE contains duplicates");
}

#[test]
fn keyword_table_round_trips_through_the_lexer() {
  for (keyword, expected) in KEYWORD_TABLE {
    let mut lexer = Lexer::new(keyword, "grammar_test.nt");
    let tokens = lexer
        .tokenize()
        .unwrap_or_else(|e| panic!("lexing keyword `{}` failed: {:?}", keyword, e));
    assert_eq!(
      tokens.len(),
      1,
      "keyword `{}` did not produce one token",
      keyword
    );
    assert_eq!(
      tokens[0].kind, *expected,
      "keyword `{}` lexed to the wrong token kind",
      keyword
    );
    assert!(
      !matches!(tokens[0].kind, TokenKind::Ident(_)),
      "keyword `{}` was lexed as a plain identifier",
      keyword
    );
  }
}

#[test]
fn type_table_round_trips_through_the_lexer() {
  for ty in TYPES {
    let mut lexer = Lexer::new(ty, "grammar_test.nt");
    let tokens = lexer
        .tokenize()
        .unwrap_or_else(|e| panic!("lexing type `{}` failed: {:?}", ty, e));
    assert_eq!(tokens.len(), 1, "type `{}` did not produce one token", ty);
    assert_eq!(
      tokens[0].kind,
      TokenKind::Type(ty.to_string()),
      "type `{}` was not lexed as a Type token",
      ty
    );
  }
}
