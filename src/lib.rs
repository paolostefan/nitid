#![allow(dead_code)]

//! # Nitid Transpiler Library
//!
//! Exposes the full compilation pipeline so that tests and external
//! tools can drive it programmatically.
//!
//! The exported [`compile`] and [`compile_file`] functions run the
//! entire four-phase pipeline:
//!
//! 1. **Lex**   — [`crate::lexer::Lexer`] tokenizes the source.
//! 2. **Parse** — [`crate::parser::Parser`] builds an AST.
//! 3. **Sema**  — [`crate::sema::Sema`] performs semantic analysis.
//! 4. **Codegen** — [`crate::codegen::Codegen`] emits C source + CMake.
//!
//! # Limitations
//! * Import resolution is declared in the grammar but **not implemented**.
//! * The semantic analyser does **not** enforce
//!   no-implicit-casts or no-uninitialized-variables in practice
//!   (it issues warnings but does not reject the program).
//! * Memory management, race-condition prevention, and buffer-overflow
//!   protection are **not** implemented.

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

use std::fs;

/// Read a file from `path` and transpile it.
///
/// This is a convenience wrapper around [`compile`].
pub fn compile_file(
    path: &str,
    c_src_dir: &str,
) -> Result<(ast::Program, Vec<codegen::CFile>, String), String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))?;
    compile(path, &content, c_src_dir)
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
    let mut sema_ctx = sema::Sema::new();
    sema_ctx.analyze(&mut program)?;
    let mut cg = codegen::Codegen::new();
    let (c_files, cmake) = cg.generate(&program, c_src_dir)?;
    Ok((program, c_files, cmake))
}
