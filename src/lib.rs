#![allow(dead_code)]

//! # Nitid Transpiler Library
//!
//! Exposes the full compilation pipeline so that tests and external
//! tools can drive it programmatically.
//!
//! The exported [`compile`] and [`compile_file`] functions run the
//! entire four-phase pipeline:
//!
//! 1. **Lex**   — [`lexer::Lexer`] tokenizes the source.
//! 2. **Parse** — [`parser::Parser`] builds an AST.
//! 3. **Sema**  — [`sema::Sema`] performs semantic analysis.
//! 4. **Codegen** — [`codegen::Codegen`] emits C source + CMake.
//!
//! # Limitations
//! * Import resolution is declared in the grammar but **not implemented**.
//! * The semantic analyzer does **not** enforce
//!   no-implicit-casts or no-uninitialized-variables in practice
//!   (it issues warnings but does not reject the program).
//! * Memory management, race-condition prevention, and buffer-overflow
//!   protection are **not** implemented.
use std::string::String;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;

/// AST node types.
pub mod ast;

/// C code emitter.
pub mod codegen;

/// Lexer (tokenizer).
pub mod lexer;

/// Recursive-descent parser.
pub mod parser;

/// Semantic analyser.
pub mod sema;

/// Type definitions and mapping to C types.
pub mod types;

/// Simplified function signature for cross-file resolution.
#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub param_types: Vec<types::Type>,
    pub return_types: Vec<types::Type>,
    pub param_names: Vec<String>,
}

/// All declarations from a package, flattened across files.
///
/// This is the "scope" that Sema uses to resolve imported names.
/// One PackageContext per imported package.
#[derive(Debug, Clone)]
pub struct PackageContext {
    /// Functions: name -> (param types, return types, param names)
    pub functions: HashMap<String, FunctionSig>,
    /// Struct definitions: name -> list of (field name, field type)
    pub structs: HashMap<String, Vec<(String, types::Type)>>,
    /// Enum definitions: name -> list of (variant name, optional value)
    pub enums: HashMap<String, Vec<(String, Option<i128>)>>,
}


/// Read a file from `path` and transpile it.
///
/// This is a convenience wrapper around [`compile`].
pub fn compile_file(
    path: &str,
    c_src_dir: &str,
) -> Result<(ast::Program, Vec<codegen::CFile>, String), String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read '{}': {}", path, e))?;
    compile(path, &content, c_src_dir)
}

/// Read a `.nt` file and parse it into an AST
///
/// Runs only the lexer+parser - no semantic analysis or codegen.
/// This is used to import package files.
pub fn parse_file(path: &str) -> Result<ast::Program, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read '{}': {}", path, e))?;
    parser::Parser::parse(&content, path)
}

/// Parse all `.nt` files in a package dir (recursively).
///
/// Returns a Vec of parsed Programs, one per file.
///Files are parsed in alphabetical order for determinism.
pub fn parse_package_dir(
    dir: &std::path::Path,
    expected_package: &str,
) -> Result<Vec<ast::Program>, String> {
    let mut files = collect_nt_files(dir)?;
    files.sort(); // deterministic order

    let mut programs = Vec::new();
    for file_path in &files {
        let path_str = file_path.to_string_lossy().to_string();
        let program = parse_file(&path_str)?;

        // Spec rule 3: verify package declaration matches.
        if program.package != expected_package {
            return Err(format!(
                "{}: Package declaration '{}' does not match expected package '{}'",
                path_str, program.package, expected_package
            ));
        }
        eprintln!(
            "[import] Parsed {}/{}",
            expected_package,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        );
        programs.push(program);
    }

    Ok(programs)
}

/// Recursively collect all `.nt` files in a directory.
fn collect_nt_files(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    let entries = dir
        .read_dir()
        .map_err(|e| format!("Failed to read directory '{}': {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirs (subdirectory inheritance assumed)
            files.extend(collect_nt_files(&path)?);
        } else if path.extension().map_or(false, |ext| ext == "nt") {
            files.push(path);
        }
    }

    Ok(files)
}

/// Resolve an import name to a package directory path.
///
/// Given the importing file's path and an import name (e.g. "Math"),
/// search for a directory named `{name}/` containing `.nt` files.
///
/// Search order:
/// 1. same dir as the importing file
/// 2. parent dir of the importing file
/// 3. None (not found)
fn resolve_package_dir(importing_file: &str, package_name: &str) -> Option<std::path::PathBuf> {
    let importing_dir = std::path::Path::new(importing_file).parent()?;

    // Search 1: sibling directory
    let candidate = importing_dir.join(package_name);
    if candidate.is_dir() && has_nt_files(&candidate) {
        return Some(candidate);
    }

    // Search 2: parent directory
    let parent = importing_dir.parent()?;
    let candidate = parent.join(package_name);
    if candidate.is_dir() && has_nt_files(&candidate) {
        return Some(candidate);
    }

    None
}

/// Check if a directory contains any `.nt` files.
fn has_nt_files(dir: &std::path::Path) -> bool {
    dir.read_dir().ok().map_or(false, |entries| {
        entries
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map_or(false, |ext| ext == "nt"))
    })
}

/// Resolve and parse all imports in a program.
///
/// For each `import Foo;`, finds the Foo package directory,
/// parses all `.nt` files inside it, and returns them keyed
/// by package name.
pub fn load_imports(
    program: &ast::Program,
) -> Result<HashMap<String, Vec<ast::Program>>, String> {
    let mut imported = HashMap::new();
    let mut visited = std::collections::HashSet::new();
    load_imports_inner(program, &mut imported, &mut visited)
}

fn load_imports_inner(
  program: &ast::Program,
  imported: &mut HashMap<String, Vec<ast::Program>>,
  visited: &mut std::collections::HashSet<String>
) -> Result<HashMap<String, Vec<ast::Program>>, String> {
  for imp in &program.imports {

    if !visited.insert(imp.name.clone()) {
      return Err(format!(
        "{}:{}:{}: Circular import: '{}'",
        imp.span.file, imp.span.line, imp.span.col, imp.name
      ));
    }

    if imported.contains_key(&imp.name) {
      return Err(format!(
        "{}:{}:{}: Duplicate import '{}'",
        imp.span.file, imp.span.line, imp.span.col, imp.name
      ));
    }

    let pkg_dir = resolve_package_dir(&program.file, &imp.name).ok_or_else(|| {
      format!(
        "{}:{}:{}: Could not find package '{}' (no directory named '{}' found)",
        imp.span.file, imp.span.line, imp.span.col, imp.name, imp.name
      )
    })?;

    let programs = parse_package_dir(&pkg_dir, &imp.name)?;
    eprintln!(
      "[import] Loaded package '{}' ({} files)",
      imp.name,
      programs.len()
    );

    // import alias (if any) or package name
    let key = imp.alias.as_ref().unwrap_or(&imp.name).clone();
    imported.insert(key, programs);
  }

  Ok(imported.clone())
}

/// Build a PackageContext from a list of parsed programs (one package)
///
/// Extracts all function, struct and enum declarations from every file in the package into a flat,
/// unified view.
pub fn build_package_context(programs: &[ast::Program]) -> PackageContext {
    let mut ctx = PackageContext {
        functions: HashMap::new(),
        structs: HashMap::new(),
        enums: HashMap::new(),
    };

    for program in programs {
        for decl in &program.decls {
            match decl {
                ast::Decl::FnDecl(f) => {
                    let param_types: Vec<types::Type> = f
                        .params
                        .iter()
                        .flat_map(|p| std::iter::repeat(p.typ.clone()).take(p.names.len()))
                        .collect();
                    let param_names: Vec<String> =
                        f.params.iter().flat_map(|p| p.names.clone()).collect();
                    ctx.functions.insert(
                        f.name.clone(),
                        FunctionSig {
                            param_types,
                            return_types: f.returns.clone(),
                            param_names,
                        },
                    );
                }
                ast::Decl::StructDecl(s) => {
                    let fields: Vec<(String, types::Type)> = s
                        .fields
                        .iter()
                        .map(|f| (f.name.clone(), f.typ.clone()))
                        .collect();
                    ctx.structs.insert(s.name.clone(), fields);
                }
                ast::Decl::EnumDecl(e) => {
                    let variants: Vec<(String, Option<i128>)> = e
                        .variants
                        .iter()
                        .map(|v| {
                            let val = match &v.value {
                                Some(ast::Expr::IntLit(n, _)) => Some(*n),
                                _ => None,
                            };
                            (v.name.clone(), val)
                        })
                        .collect();
                    ctx.enums.insert(e.name.clone(), variants);
                }
                // ImplBlock methods are handled via struct_methods in sema.as
                _ => {}
            }
        }
    }
    ctx
}

/// Combine multiple PackageContexts into one.
///
/// Used when a file imports several packages. Later contexts shadow earlier ones
/// on name conflicts (last-one-wins).
pub fn merge_contexts(contexts: &[PackageContext]) -> PackageContext {
    let mut merged = PackageContext {
        functions: HashMap::new(),
        structs: HashMap::new(),
        enums: HashMap::new(),
    };

    for ctx in contexts {
        merged
            .functions
            .extend(ctx.functions.iter().map(|(k, v)| (k.clone(), v.clone())));
        merged
            .structs
            .extend(ctx.structs.iter().map(|(k, v)| (k.clone(), v.clone())));
        merged
            .enums
            .extend(ctx.enums.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    merged
}

/// Run the full transpilation pipeline on `content`.
///
/// # Arguments
/// * `path` — source file path (used for error messages and C output name).
/// * `content` — source text of the `.nt` file.
/// * `c_src_dir` — output directory for generated C files.
///
/// # Returns
/// `(Program, Vec<CFile>, cmake_project_string)` on success,
/// or a human-readable error message.
pub fn compile(
    path: &str,
    content: &str,
    c_src_dir: &str,
) -> Result<(ast::Program, Vec<codegen::CFile>, String), String> {
    let mut program = parser::Parser::parse(content, path)?;

    // Load and parse imports.
    let imports = load_imports(&program)?;

    // Build a per-package context map.
    let pkg_contexts: HashMap<String, PackageContext> = imports
        .iter()
        .map(|(name, programs)| (name.clone(), build_package_context(programs)))
        .collect();

    // Pass the package map to sema.
    let mut sema_ctx = sema::Sema::new();
    sema_ctx.analyze(&mut program, Some(&pkg_contexts))?;

    let mut cg = codegen::Codegen::new();
    let package_names: HashSet<String> = pkg_contexts.keys().cloned().collect();
    let (c_files, cmake) =
        cg.generate(&program, c_src_dir, package_names)?;

    Ok((program, c_files, cmake))
}
