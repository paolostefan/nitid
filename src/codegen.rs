/// C code generator (transpiler backend).
///
/// Phase 4 of the transpilation pipeline: walks the analyzed AST and
/// emits equivalent C source code together with a CMakeLists.txt.
///
/// # Strategy
/// * Nitid `string` → `nitid_string` (a custom runtime type).
/// * Multiple return values → output pointer parameters appended to
///   the C function signature.
/// * Top-level "dangling" statements are wrapped in `main` by the
///   parser; this pass just emits them.
/// * Type inference for `:=` uses a simple heuristic
///   ([`infer_init_type`]) rather than consulting the semantic
///   analyzer's results.
///
/// # Limitations
/// * Only one `.c` file is produced per input `.nt` file (no
///   separate compilation / linking of multiple translation units).
/// * The CMake output assumes a single source file + the runtime.
/// * Many advanced types (I256, U256, F8, F16, String16, String32)
///   will produce C code that references unknown types.
/// * The runtime string library (`nitid_string`) is minimal.
use crate::ast::*;
use crate::types::{Type, is_string_type};
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

/// A C source file ready to be written to disk.
pub struct CFile {
    pub path: String,
    pub content: String,
}

/// Code generation context.
///
/// Tracks:
/// * Function signatures (parameters and return types) for every
///   declared function.
/// * A scope stack for variable declarations.
/// * Accumulated warnings.
pub struct Codegen {
    /// Map from function name → return type list.
    fn_returns: HashMap<String, Vec<Type>>,
    /// Map from function name → parameter type list.
    fn_params: HashMap<String, Vec<Type>>,
    warnings: Vec<String>,
    /// Scope stack for local variable declarations.
    scopes: Vec<HashMap<String, Type>>,
    /// Known struct type names.
    struct_names: HashSet<String>,
    /// Struct definitions (name, fields, packed, align) for emission.
    struct_defs: Vec<(String, Vec<StructField>, bool, Option<u64>)>,
    /// Enum definitions (name, variants) for emission.
    enum_defs: Vec<(String, Vec<EnumVariant>)>,
    /// Imported package names, for resolving qualified calls like Math.foo()
    package_names: HashSet<String>,
}

impl Codegen {
    /// Create a new code generator with empty scopes and no collected warnings.
    pub fn new() -> Self {
        Self {
            fn_returns: HashMap::new(),
            fn_params: HashMap::new(),
            warnings: Vec::new(),
            scopes: vec![HashMap::new()],
            struct_names: HashSet::new(),
            struct_defs: Vec::new(),
            enum_defs: Vec::new(),
            package_names: HashSet::new(),
        }
    }

    // ── Entry point ──────────────────────────────────────────

    /// Generate C source and a CMakeLists.txt for `program`.
    ///
    /// # Returns
    /// `(c_files, cmake_text)` where `c_files` is a vector of
    /// (path, content) pairs for the generated `.c` files and
    /// `cmake_text` is the content of `CMakeLists.txt`.
    pub fn generate(
        &mut self,
        program: &Program,
        c_src_dir: &str,
        package_names: HashSet<String>,
        foreign_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    ) -> Result<(Vec<CFile>, String), String> {
        let c_code = self.generate_c(program, package_names, foreign_sigs);

        let base_name = Path::new(&program.file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");

        let mut c_files = Vec::new();
        c_files.push(CFile {
            path: format!("{}/{}.c", c_src_dir, base_name),
            content: c_code,
        });

        let stems: Vec<String> = c_files
            .iter()
            .map(|cf| {
                Path::new(&cf.path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        let cmake = self.emit_cmake(program, &stems);
        Ok((c_files, cmake))
    }

    pub fn generate_c(
        &mut self,
        program: &Program,
        package_names: HashSet<String>,
        foreign_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    ) -> String {
        self.package_names = package_names;

        // Pass 0: collect struct and enum definitions.
        for decl in &program.decls {
            match decl {
                Decl::StructDecl(s) => {
                    self.struct_names.insert(s.name.clone());
                    self.struct_defs
                        .push((s.name.clone(), s.fields.clone(), s.packed, s.align));
                }
                Decl::EnumDecl(e) => {
                    self.enum_defs.push((e.name.clone(), e.variants.clone()));
                }
                _ => {}
            }
        }

        // Pass 1: collect all function and method signatures.
        for decl in &program.decls {
            match decl {
                Decl::FnDecl(f) => {
                    let param_types: Vec<Type> = f
                        .params
                        .iter()
                        .flat_map(|p| std::iter::repeat(p.typ.clone()).take(p.names.len()))
                        .collect();
                    self.fn_params.insert(f.name.clone(), param_types);
                    self.fn_returns.insert(f.name.clone(), f.returns.clone());
                }
                Decl::ImplBlock(imp) => {
                    let struct_name = &imp.struct_name;
                    for method in &imp.methods {
                        let mangled = format!("{}_{}", struct_name, method.name);
                        let param_types: Vec<Type> = method
                            .params
                            .iter()
                            .flat_map(|p| std::iter::repeat(p.typ.clone()).take(p.names.len()))
                            .collect();
                        let mut all_params = vec![Type::Struct(struct_name.clone())];
                        all_params.extend(param_types);
                        self.fn_params.insert(mangled.clone(), all_params);
                        self.fn_returns.insert(mangled, method.returns.clone());
                    }
                }
                _ => {}
            }
        }

        let mut c_code = String::new();

        // Standard C headers and Nitid runtime include.
        c_code.push_str("#include <stdio.h>\n");
        c_code.push_str("#include \"runtime/nitid_types.h\"\n");
        c_code.push_str("#include \"runtime/nitid_string.h\"\n");
        c_code.push_str("#include \"runtime/nitid_string16.h\"\n");
        c_code.push_str("#include \"runtime/nitid_string32.h\"\n");
        c_code.push_str("#include \"runtime/nitid_array.h\"\n");
        c_code.push('\n');

        // Prototypes for functions defined in other files.
        let mut for_sign: Vec<(&String, &(Vec<Type>, Vec<Type>))> = foreign_sigs.iter().collect();
        for_sign.sort(); // deterministic order

        for (name, (params, returns)) in for_sign {
            let ret = if returns.len() == 1 && returns[0] != Type::Void {
                returns[0].c_str()
            } else {
                Cow::from("void")
            };

            let plist = params
                .iter()
                .map(|t| t.c_str())
                .collect::<Vec<_>>()
                .join(", ");
            c_code.push_str(&format!("{} {}({});\n", ret, name, plist));
        }
        c_code.push('\n');

        // Enum type definitions.
        let enum_emissions: Vec<(String, String)> = self
            .enum_defs
            .iter()
            .map(|(name, variants)| {
                let mut body = String::new();
                for v in variants {
                    if let Some(ref val) = v.value {
                        // We can't call self.emit_expr here due to borrow rules,
                        // so we use a simple structural emission for enum values.
                        let val_str = enum_value_to_c(val);
                        body.push_str(&format!(" {} = {},", v.name, val_str));
                    } else {
                        body.push_str(&format!(" {},", v.name));
                    }
                }
                (name.clone(), body)
            })
            .collect();
        for (name, body) in &enum_emissions {
            c_code.push_str(&format!("typedef enum {{{} }} {};\n\n", body, name));
        }

        // Forward declarations for struct types.
        for (name, fields, packed, align) in &self.struct_defs {
            let attr = match (packed, align) {
                (true, Some(a)) => format!(" __attribute__((packed, aligned({})))", a),
                (true, None) => " __attribute__((packed))".to_string(),
                (false, Some(a)) => format!(" __attribute__((aligned({})))", a),
                (false, None) => String::new(),
            };
            c_code.push_str(&format!("typedef struct{} {} {{", attr, name));
            for f in fields {
                match &f.typ {
                    Type::TyFixedArray(elem, n) => {
                        c_code.push_str(&format!(" {} {}[{}];", elem.c_str(), f.name, n));
                    }
                    _ => {
                        c_code.push_str(&format!(" {} {};", f.typ.c_str(), f.name));
                    }
                }
            }
            c_code.push_str(&format!(" }} {};\n\n", name));
        }

        // Forward declarations for functions that return values
        for decl in &program.decls {
            match decl {
                // Decl::FnDecl(f) => {
                //     if f.returns.len() > 1 || (f.returns.len() == 1 && f.returns[0] != Type::Void) {
                //         c_code.push_str(&self.emit_fn_decl(f, false));
                //         c_code.push_str(";\n");
                //     }
                // }
                Decl::ImplBlock(imp) => {
                    for method in &imp.methods {
                        if method.returns.len() > 1
                            || (method.returns.len() == 1 && method.returns[0] != Type::Void)
                        {
                            c_code.push_str(&self.emit_method_decl(imp, method, false));
                            c_code.push_str(";\n");
                        }
                    }
                }
                _ => {}
            }
        }

        // Emit all declarations and function definitions.
        for decl in &program.decls {
            match decl {
                Decl::FnDecl(f) => {
                    c_code.push_str(&self.emit_fn_decl(f, false));
                    c_code.push_str(" {\n");
                    c_code.push_str(&self.emit_fn_body(f));
                    c_code.push_str("}\n\n");
                }
                Decl::ImplBlock(imp) => {
                    for method in &imp.methods {
                        c_code.push_str(&self.emit_method_decl(imp, method, false));
                        c_code.push_str(" {\n");
                        c_code.push_str(&self.emit_method_body(imp, method));
                        c_code.push_str("}\n\n");
                    }
                }
                Decl::VarDecl(v) => {
                    c_code.push_str(&self.emit_var_decl(v));
                }
                Decl::StructDecl(_) | Decl::EnumDecl(_) => {} // already emitted as forward decl
            }
        }

        c_code
    }

    /// Generate a `CMakeLists.txt` that compiles the emitted C code
    /// together with the Nitid runtime.
    pub fn emit_cmake(&self, program: &Program, file_stems: &[String]) -> String {
        let proj_name = if program.package == "main" {
            "program"
        } else {
            &program.package
        };

        let mut cmake = String::new();
        cmake.push_str("cmake_minimum_required(VERSION 3.10)\n");
        cmake.push_str(&format!("project({} C)\n\n", proj_name));
        cmake.push_str("set(CMAKE_C_STANDARD 17)\n");
        cmake.push_str("set(CMAKE_C_STANDARD_REQUIRED ON)\n\n");

        cmake.push_str("add_library(nitid_runtime STATIC\n");
        cmake.push_str("            ${CMAKE_CURRENT_SOURCE_DIR}/runtime/nitid_array.c\n");
        cmake.push_str("            ${CMAKE_CURRENT_SOURCE_DIR}/runtime/nitid_string.c\n");
        cmake.push_str("            ${CMAKE_CURRENT_SOURCE_DIR}/runtime/nitid_string16.c\n");
        cmake.push_str("            ${CMAKE_CURRENT_SOURCE_DIR}/runtime/nitid_string32.c\n");
        cmake.push_str(")\n");

        cmake.push_str("target_include_directories(nitid_runtime PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/runtime)\n");

        cmake.push_str(&format!("add_executable({}\n", proj_name));
        for stem in file_stems {
            cmake.push_str(&format!("    {}.c\n", stem));
        }
        cmake.push_str(")\n\n");

        cmake.push_str(
      "target_include_directories(${PROJECT_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/runtime)\n"
    );
        cmake.push_str("target_link_libraries(${PROJECT_NAME} PRIVATE nitid_runtime)\n");

        cmake
    }

    // ── Function emission ────────────────────────────────────

    /// Emit a C function declaration (or forward declaration).
    ///
    /// `main` receives the standard `(int argc, char **argv)` signature.
    /// Multi-return functions get additional `*resN` output parameters.
    fn emit_fn_decl(&self, f: &FnDecl, _forward: bool) -> String {
        let mut s = String::new();
        let returns = &f.returns;

        // Special-case main.
        if f.name == "main" {
            // TODO: change main args when needed
            s.push_str("int main(void)");
            return s;
        }

        // Return type.
        if returns.len() == 1 {
            s.push_str(&format!("{} ", returns[0].c_str()));
        } else {
            s.push_str("void ");
        }
        s.push_str(&f.name);
        s.push('(');

        // Parameters.
        let mut param_list = Vec::new();
        for p in &f.params {
            for name in &p.names {
                param_list.push(format!("{} {}", p.typ.c_str(), name));
            }
        }

        // For multi-return functions, append output pointer parameters.
        if returns.len() > 1 {
            for (i, ret_type) in returns.iter().enumerate() {
                param_list.push(format!("{} *res{}", ret_type.c_str(), i));
            }
        }

        s.push_str(&param_list.join(", "));
        s.push(')');
        s
    }

    /// Emit the body of a function.
    ///
    /// Handles implicit variable declarations for parameters, and
    /// adds `return 0;` for `main` if no explicit return exists.
    fn emit_fn_body(&mut self, f: &FnDecl) -> String {
        self.push_scope();
        for param in &f.params {
            for name in &param.names {
                self.declare_var(name, param.typ.clone());
            }
        }
        let mut s = String::new();
        for stmt in &f.body {
            s.push_str(&self.emit_stmt(stmt, &f.name));
        }
        // main must return int — add implicit return if missing.
        if f.name == "main" && !f.body.iter().any(|s| matches!(s, Stmt::Return(..))) {
            s.push_str("    return 0;\n");
        }
        self.pop_scope();
        s
    }

    /// Emit a method declaration: `ret_type struct_name_method_name(struct struct_name* self, ...)`.
    fn emit_method_decl(&self, imp: &ImplBlock, method: &FnDecl, _forward: bool) -> String {
        let struct_name = &imp.struct_name;
        let mangled = format!("{}_{}", struct_name, method.name);
        let mut s = String::new();
        let returns = &method.returns;

        // Return type.
        if returns.len() == 1 {
            s.push_str(&format!("{} ", returns[0].c_str()));
        } else {
            s.push_str("void ");
        }
        s.push_str(&mangled);
        s.push('(');

        let mut param_list = Vec::new();
        // First parameter: self pointer (typedef'd, no `struct` prefix needed)
        param_list.push(format!("{}* self", struct_name));
        for p in &method.params {
            for name in &p.names {
                param_list.push(format!("{} {}", p.typ.c_str(), name));
            }
        }

        if returns.len() > 1 {
            for (i, ret_type) in returns.iter().enumerate() {
                param_list.push(format!("{} *res{}", ret_type.c_str(), i));
            }
        }

        s.push_str(&param_list.join(", "));
        s.push(')');
        s
    }

    /// Emit the body of a method.
    fn emit_method_body(&mut self, imp: &ImplBlock, method: &FnDecl) -> String {
        self.push_scope();
        // Declare self as the struct type (pointer)
        self.declare_var("self", Type::Struct(imp.struct_name.clone()));
        for param in &method.params {
            for name in &param.names {
                self.declare_var(name, param.typ.clone());
            }
        }
        let mangled = format!("{}_{}", imp.struct_name, method.name);
        let mut s = String::new();
        for stmt in &method.body {
            s.push_str(&self.emit_stmt(stmt, &mangled));
        }
        self.pop_scope();
        s
    }

    // ── Statement emission ───────────────────────────────────

    /// Emit a single statement as C code.
    fn emit_stmt(&mut self, stmt: &Stmt, current_fn: &str) -> String {
        match stmt {
            Stmt::VarDecl(v) => self.emit_var_decl(v),
            Stmt::Expr(e) => {
                let mut s = String::from("    ");
                s.push_str(&self.emit_expr(e, current_fn));
                s.push_str(";\n");
                s
            }
            Stmt::Return(values, _) => self.emit_return(values, current_fn),
            Stmt::Block(body) => {
                self.push_scope();
                let mut s = String::from("{\n");
                for stmt in body {
                    s.push_str(&self.emit_stmt(stmt, current_fn));
                }
                s.push_str("}\n");
                self.pop_scope();
                s
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                let mut s = format!("if ({}) {{\n", self.emit_expr(cond, current_fn));
                for stmt in then_block {
                    s.push_str(&self.emit_stmt(stmt, current_fn));
                }
                if let Some(else_b) = else_block {
                    s.push_str("} else {\n");
                    for stmt in else_b {
                        s.push_str(&self.emit_stmt(stmt, current_fn));
                    }
                }
                s.push_str("}\n");
                s
            }
            Stmt::While { cond, body, .. } => {
                let mut s = format!("while ({}) {{\n", self.emit_expr(cond, current_fn));
                for stmt in body {
                    s.push_str(&self.emit_stmt(stmt, current_fn));
                }
                s.push_str("}\n");
                s
            }
            Stmt::For {
                init,
                cond,
                inc,
                body,
                ..
            } => {
                let mut s = String::from("for (");
                // Init
                if let Some(init_stmt) = init {
                    match init_stmt.as_ref() {
                        Stmt::VarDecl(v) => {
                            if let Some(ref typ) = v.typ {
                                s.push_str(&format!("{} {}", typ.c_str(), v.names.join(", ")));
                                if let Some(ref init_expr) = v.init {
                                    s.push_str(&format!(
                                        " = {}",
                                        self.emit_expr(init_expr, current_fn)
                                    ));
                                }
                            } else if let Some(ref init_expr) = v.init {
                                let t = self.infer_expr_type_str(init_expr);
                                s.push_str(&format!("{} {}", t, v.names[0]));
                                s.push_str(&format!(
                                    " = {}",
                                    self.emit_expr(init_expr, current_fn)
                                ));
                            }
                        }
                        Stmt::Expr(e) => {
                            s.push_str(&self.emit_expr(e, current_fn));
                        }
                        _ => {}
                    }
                }
                s.push_str("; ");
                // Cond
                if let Some(cond_expr) = cond {
                    s.push_str(&self.emit_expr(cond_expr, current_fn));
                }
                s.push_str("; ");
                // Inc
                if let Some(inc_expr) = inc {
                    s.push_str(&self.emit_expr(inc_expr, current_fn));
                }
                s.push_str(") {\n");
                for stmt in body {
                    s.push_str(&self.emit_stmt(stmt, current_fn));
                }
                s.push_str("}\n");
                s
            }
            Stmt::ForIn {
                var, iter, body, ..
            } => {
                let iter_str = self.emit_expr(iter, current_fn);
                let elem_type = self.infer_array_elem_type(iter);
                let suffix = self.array_type_suffix(&elem_type);
                let c_type = elem_type.c_str();
                let mut s = format!("{{ /* for ({} : {}) */\n", var, iter_str);
                self.declare_var(var, elem_type);
                s.push_str(&format!("    nitid_array _nitid_iter = {};\n", iter_str));
                s.push_str("    size_t _nitid_len = nitid_array_size(_nitid_iter);\n");
                s.push_str("    for (size_t _nitid_i = 0; _nitid_i < _nitid_len; _nitid_i++) {\n");
                s.push_str(&format!(
                    "        {} {} = nitid_array_get_{}(_nitid_iter, _nitid_i);\n",
                    c_type, var, suffix
                ));
                for stmt in body {
                    s.push_str(&self.emit_stmt(stmt, current_fn));
                }
                s.push_str("    }\n");
                s.push_str("}\n");
                s
            }
            Stmt::ForInIndex {
                idx_var,
                item_var,
                iter,
                body,
                ..
            } => {
                let iter_str = self.emit_expr(iter, current_fn);
                let elem_type = self.infer_array_elem_type(iter);
                let suffix = self.array_type_suffix(&elem_type);
                let c_type = elem_type.c_str();
                let mut s = format!("{{ /* for ({}, {} : {}) */\n", idx_var, item_var, iter_str);
                self.declare_var(idx_var, Type::I32);
                self.declare_var(item_var, elem_type);
                s.push_str(&format!("    nitid_array _nitid_iter = {};\n", iter_str));
                s.push_str("    size_t _nitid_len = nitid_array_size(_nitid_iter);\n");
                s.push_str("    for (size_t _nitid_i = 0; _nitid_i < _nitid_len; _nitid_i++) {\n");
                s.push_str(&format!("        size_t {} = _nitid_i;\n", idx_var));
                s.push_str(&format!(
                    "        {} {} = nitid_array_get_{}(_nitid_iter, _nitid_i);\n",
                    c_type, item_var, suffix
                ));
                for stmt in body {
                    s.push_str(&self.emit_stmt(stmt, current_fn));
                }
                s.push_str("    }\n");
                s.push_str("}\n");
                s
            }
            Stmt::Break(_) => String::from("    break;\n"),
            Stmt::Continue(_) => String::from("    continue;\n"),
        }
    }

    // ── Variable declaration emission ────────────────────────

    /// Heuristic: guess the C type of an expression based on its AST
    /// kind (ignores semantic analysis results).
    fn infer_init_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::IntLit(..) => Type::I32,
            Expr::FloatLit(..) => Type::F64,
            Expr::StringLit(..) => Type::String,
            Expr::CharLit(..) => Type::U8,
            Expr::BoolLit(..) => Type::Bool,
            Expr::ArrayLit(elems, _) => {
                let elem = if elems.is_empty() {
                    Type::I32
                } else {
                    self.infer_init_type(&elems[0])
                };
                Type::TyArray(Box::new(elem), None)
            }
            Expr::StructLit { struct_name, .. } => Type::Struct(struct_name.clone()),
            // Enum member identifiers are unknown to codegen; fall through to I32 (C enum = int).
            _ => Type::I32,
        }
    }

    /// Emit an initializer expression, using the declared type to
    /// determine the correct array element type when inference would
    /// default to I32 (e.g. integer literals in a `u8[]` variable).
    fn emit_typed_init(&mut self, init: &Expr, declared_type: &Type) -> String {
        match (declared_type, init) {
            (Type::TyFixedArray(_decl_elem, _), Expr::ArrayLit(elems, _)) => {
                let elems_str: Vec<String> = elems.iter().map(|e| self.emit_expr(e, "")).collect();
                format!("{{ {} }}", elems_str.join(", "))
            }
            // Dynamically-sized array with declared initial length: pad the
            // compound literal to the declared size so the runtime zero-fills
            // and stores every element on the heap.
            (Type::TyArray(decl_elem, Some(count)), Expr::ArrayLit(elems, _)) => {
                let elems_str: Vec<String> = elems.iter().map(|e| self.emit_expr(e, "")).collect();
                let suffix = self.array_type_suffix(decl_elem);
                let c_type = decl_elem.c_str();
                format!(
                    "nitid_array_from_lit_{}({}, ({}[{}]){{\n        {}\n    }})",
                    suffix,
                    count,
                    c_type,
                    count,
                    elems_str.join(",\n        ")
                )
            }
            (Type::TyArray(decl_elem, _), Expr::ArrayLit(elems, _)) => {
                let elems_str: Vec<String> = elems.iter().map(|e| self.emit_expr(e, "")).collect();
                let suffix = self.array_type_suffix(decl_elem);
                let c_type = decl_elem.c_str();
                format!(
                    "nitid_array_from_lit_{}({}, ({}[]){{\n        {}\n    }})",
                    suffix,
                    elems.len(),
                    c_type,
                    elems_str.join(",\n        ")
                )
            }
            _ => self.emit_expr(init, ""),
        }
    }

    /// Return the array dimension suffix for a fixed-size array type,
    /// e.g. `[5]` for `TyFixedArray(_, 5)`, empty string otherwise.
    fn array_decl_suffix(&self, typ: &Type) -> String {
        match typ {
            Type::TyFixedArray(_, n) => format!("[{}]", n),
            _ => String::new(),
        }
    }

    /// Emit a variable declaration as C code.
    fn emit_var_decl(&mut self, v: &VarDecl) -> String {
        let mut s = String::new();

        if let Some(ref typ) = v.typ {
            let type_str = typ.c_str();
            let suffix = self.array_decl_suffix(typ);
            if let Some(ref init) = v.init {
                if v.names.len() == 1 {
                    self.declare_var(&v.names[0], typ.clone());
                    s.push_str(&format!(
                        "    {} {}{} = {};\n",
                        type_str,
                        v.names[0],
                        suffix,
                        if is_string_type(typ) {
                            self.emit_string_as_value(init, typ, "")
                        } else {
                            self.emit_typed_init(init, typ)
                        }
                    ));
                } else {
                    // Multiple names with one initializer — declare all,
                    // but only the first gets the initializer.
                    for name in &v.names {
                        self.declare_var(name, typ.clone());
                        s.push_str(&format!("    {} {}{};\n", type_str, name, suffix));
                    }
                }
            } else {
                // Struct-like types (e.g. nitid_array) need zero-init to avoid
                // garbage pointers.  Scalar types are fine uninitialized.
                let decls: Vec<String> = v
                    .names
                    .iter()
                    .map(|n| match typ {
                        Type::TyFixedArray(..) => {
                            format!("{}{} = {{0}}", n, suffix)
                        }
                        // Dynamically-sized array with initial length: the
                        // runtime allocates and zeroes `count` elements.
                        Type::TyArray(elem, Some(count)) => {
                            format!(
                                "{} = nitid_array_zeros(sizeof({}), {})",
                                n,
                                elem.c_str(),
                                count
                            )
                        }
                        // Unsized dynamic array: empty, but carry the element
                        // size so runtime calls like resize() know how to
                        // address memory; remaining fields are zeroed.
                        Type::TyArray(elem, None) => {
                            format!("{} = {{.elem_size = sizeof({})}}", n, elem.c_str())
                        }
                        _ => n.clone(),
                    })
                    .collect();
                for name in &v.names {
                    self.declare_var(name, typ.clone());
                }
                s.push_str(&format!("    {} {};\n", type_str, decls.join(", ")));
            }
        } else if let Some(ref init) = v.init {
            // Type inference via `:=`.
            if v.names.len() > 1 {
                s.push_str(&self.emit_multi_assign(&v.names, init, ""));
            } else {
                let init_type = self.infer_init_type(init);
                let inferred = self.infer_expr_type_str(init);
                self.declare_var(&v.names[0], init_type.clone());
                s.push_str(&format!(
                    "    {} {} = {};\n",
                    inferred,
                    v.names[0],
                    if init_type == Type::String {
                        self.emit_string_as_value(init, &init_type, "")
                    } else {
                        self.emit_expr(init, "")
                    }
                ));
            }
        } else {
            // No type and no initializer — default to `int`.
            for name in &v.names {
                self.declare_var(name, Type::I32);
            }
            s.push_str(&format!("    {} {};\n", "int", v.names.join(", ")));
        }

        s
    }

    // ── Return statement emission ────────────────────────────

    /// Emit a return statement.
    ///
    /// Multi-return functions write through output pointer parameters
    /// instead of using C's `return`.
    fn emit_return(&mut self, values: &[Expr], current_fn: &str) -> String {
        let returns = self.fn_returns.get(current_fn);
        let mut s = String::new();

        match returns {
            Some(ret_types) if ret_types.len() > 1 => {
                for (i, val) in values.iter().enumerate() {
                    s.push_str(&format!(
                        "    *res{} = {};\n",
                        i,
                        self.emit_expr(val, current_fn)
                    ));
                }
            }
            Some(ret_types) if ret_types.len() == 1 && ret_types[0] != Type::Void => {
                s.push_str(&format!(
                    "    return {};\n",
                    self.emit_expr(&values[0], current_fn)
                ));
            }
            _ => {
                s.push_str("    return;\n");
            }
        }
        s
    }

    // ── Multi-assignment emission ────────────────────────────

    /// Emit multi-return assignment: `a, b := foo()`.
    ///
    /// Declares temporary variables for each return value and calls
    /// the function with `&var` output arguments.
    fn emit_multi_assign(&mut self, names: &[String], init: &Expr, current_fn: &str) -> String {
        let mut s = String::new();

        if let Expr::Call { name, .. } = init {
            if let Some(ret_types) = self.fn_returns.get(name) {
                let ret_types = ret_types.clone();
                for (_i, (name, ret_type)) in names.iter().zip(ret_types.iter()).enumerate() {
                    self.declare_var(name, ret_type.clone());
                    s.push_str(&format!("    {} {};\n", ret_type.c_str(), name));
                }
                let args_str = self.emit_call_args(init, current_fn);
                let addr_args: Vec<String> = names.iter().map(|n| format!("&{}", n)).collect();
                let all_args = if args_str.is_empty() {
                    addr_args.join(", ")
                } else {
                    format!("{}, {}", args_str, addr_args.join(", "))
                };
                s.push_str(&format!("    {}({});\n", name, all_args));
            }
        }
        s
    }

    // ── Expression emission ──────────────────────────────────

    /// Emit an expression as a C string.
    fn emit_expr(&mut self, expr: &Expr, current_fn: &str) -> String {
        match expr {
            Expr::IntLit(val, _) => val.to_string(),
            Expr::FloatLit(val, _) => val.to_string(),
            Expr::StringLit(val, _) => self.emit_string_lit(val),
            Expr::CharLit(val, _) => self.emit_char_lit(*val),
            Expr::BoolLit(val, _) => {
                if *val {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Expr::Ident(name, _) => name.clone(),
            Expr::Call { name, args, .. } => self.emit_call(name, args, current_fn),
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                // Check for string-typed operands → emit runtime function calls
                if let Some(lt) = self.typeof_expr(left) {
                    if is_string_type(&lt) {
                        let l = self.emit_expr(left, current_fn);
                        let r = self.emit_expr(right, current_fn);
                        let pfx = match &lt {
                            Type::String => "nitid_string",
                            Type::String16 => "nitid_string16",
                            Type::String32 => "nitid_string32",
                            _ => unreachable!(),
                        };
                        return match op {
                            BinOp::Add => format!("{}_concat(&{}, &{})", pfx, l, r),
                            BinOp::Eq => format!("{}_eq(&{}, &{})", pfx, l, r),
                            BinOp::Ne => format!("{}_ne(&{}, &{})", pfx, l, r),
                            BinOp::Lt => format!("{}_lt(&{}, &{})", pfx, l, r),
                            BinOp::Gt => format!("{}_gt(&{}, &{})", pfx, l, r),
                            BinOp::Le => format!("{}_le(&{}, &{})", pfx, l, r),
                            BinOp::Ge => format!("{}_ge(&{}, &{})", pfx, l, r),
                            _ => {
                                let op_str = self.binop_to_str(op);
                                format!("{} {} {}", l, op_str, r)
                            }
                        };
                    }
                }
                let l = self.emit_binary_child(left, current_fn);
                let r = self.emit_binary_child(right, current_fn);
                let op_str = self.binop_to_str(op);
                format!("{} {} {}", l, op_str, r)
            }
            Expr::Assign { left, right, .. } => {
                let left_str = self.emit_expr(left, current_fn);
                let right_str = match left.as_ref() {
                    Expr::Ident(name, _) => match self.lookup_var_type(name) {
                        Some(ref typ) if is_string_type(typ) => {
                            self.emit_string_as_value(right, typ, current_fn)
                        }
                        _ => self.emit_expr(right, current_fn),
                    },
                    _ => self.emit_expr(right, current_fn),
                };
                format!("{} = {}", left_str, right_str)
            }
            Expr::DeclAssign { names, right, .. } => {
                if names.len() > 1 {
                    self.emit_multi_assign(names, right, current_fn)
                } else {
                    let init_type = self.infer_init_type(right);
                    let inferred = self.infer_expr_type_str(right);
                    self.declare_var(&names[0], init_type.clone());
                    format!(
                        "{} {} = {}",
                        inferred,
                        names[0],
                        if init_type == Type::String {
                            self.emit_string_as_value(right, &init_type, current_fn)
                        } else {
                            self.emit_expr(right, current_fn)
                        }
                    )
                }
            }
            Expr::ArrayLit(elems, _) => {
                let elems_str: Vec<String> = elems
                    .iter()
                    .map(|e| self.emit_expr(e, current_fn))
                    .collect();
                let elem_type = if elems.is_empty() {
                    Type::I32
                } else {
                    self.infer_init_type(&elems[0])
                };
                let suffix = self.array_type_suffix(&elem_type);
                let c_type = elem_type.c_str();
                format!(
                    "nitid_array_from_lit_{}({}, ({}[]){{\n        {}\n    }})",
                    suffix,
                    elems.len(),
                    c_type,
                    elems_str.join(",\n        ")
                )
            }
            Expr::Index { target, index, .. } => {
                let target_str = self.emit_expr(target, current_fn);
                // Check for string indexing → codepoint-aware at_cp
                if let Some(t) = self.typeof_expr(target) {
                    match t {
                        Type::String => {
                            return format!(
                                "nitid_string_at_cp(&{}, {})",
                                target_str,
                                self.emit_expr(index, current_fn)
                            );
                        }
                        Type::String16 => {
                            return format!(
                                "nitid_string16_at_cp(&{}, {})",
                                target_str,
                                self.emit_expr(index, current_fn)
                            );
                        }
                        Type::String32 => {
                            return format!(
                                "nitid_string32_at_cp(&{}, {})",
                                target_str,
                                self.emit_expr(index, current_fn)
                            );
                        }
                        _ => {}
                    }
                }
                let is_sized = self
                    .infer_array_info(target)
                    .map(|(_, sized)| sized)
                    .unwrap_or(false);
                if is_sized {
                    // Normalize negative literal indices to their wrapped positive form
                    let index_str = self
                        .normalize_literal_index(index, target)
                        .unwrap_or_else(|| self.emit_expr(index, current_fn));
                    format!("{}[{}]", target_str, index_str)
                } else {
                    let elem_type = self.infer_array_elem_type(target);
                    let suffix = self.array_type_suffix(&elem_type);
                    format!(
                        "nitid_array_get_{}({}, {})",
                        suffix,
                        target_str,
                        self.emit_expr(index, current_fn)
                    )
                }
            }
            Expr::PostIncrement { target, .. } => {
                format!("{}++", self.emit_expr(target, current_fn))
            }
            Expr::PostDecrement { target, .. } => {
                format!("{}--", self.emit_expr(target, current_fn))
            }
            Expr::FieldAccess { target, field, .. } => {
                let target_str = self.emit_expr(target, current_fn);
                let op = if matches!(target.as_ref(), Expr::Ident(name, _) if name == "self") {
                    "->"
                } else {
                    "."
                };
                format!("{}{}{}", target_str, op, field)
            }
            Expr::MethodCall {
                target,
                method,
                args,
                ..
            } => {
                // Import: qualified package call, e.g. Math.Multiply(25,14) .
                if let Expr::Ident(pkg_name, _) = target.as_ref() {
                    if self.package_names.contains(pkg_name) {
                        let args_str = self.emit_call_args_joined(args, current_fn);
                        return format!("{}({})", method, args_str);
                    }
                }

                let target_str = self.emit_expr(target, current_fn);
                let args_str: Vec<String> =
                    args.iter().map(|a| self.emit_expr(a, current_fn)).collect();
                let struct_name = match target.as_ref() {
                    Expr::Ident(name, _) => self.lookup_var_type(name).and_then(|t| match t {
                        Type::Struct(s) => Some(s),
                        _ => None,
                    }),
                    _ => None,
                };
                if let Some(ref sname) = struct_name {
                    let mangled = format!("{}_{}", sname, method);
                    let mut all_args = Vec::new();
                    // For self, pass the pointer (or address of)
                    if matches!(target.as_ref(), Expr::Ident(name, _) if name == "self") {
                        all_args.push(target_str);
                    } else {
                        all_args.push(format!("&{}", target_str));
                    }
                    all_args.extend(args_str);
                    format!("{}({})", mangled, all_args.join(", "))
                } else if method == "size" {
                    let var_type = match target.as_ref() {
                        Expr::Ident(name, _) => self.lookup_var_type(name),
                        _ => None,
                    };
                    match var_type {
                        Some(Type::TyFixedArray(_, n)) => format!("({})", n),
                        Some(Type::TyArray(..)) => format!("nitid_array_size({})", target_str),
                        _ => format!("/* unknown method {}.{} */", target_str, method),
                    }
                } else if method == "resize" {
                    // Sema guarantees a dynamic array with exactly one
                    // integer argument.
                    match target.as_ref() {
                        Expr::Ident(name, _) => match args_str.first() {
                            Some(arg) => format!("nitid_array_resize(&{}, {})", name, arg),
                            None => format!("/* invalid resize on {} */", name),
                        },
                        _ => "/* unsupported resize target */".to_string(),
                    }
                } else {
                    format!("/* unknown method {}.{} */", target_str, method)
                }
            }
            Expr::StructLit {
                struct_name,
                fields,
                ..
            } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(name, val)| {
                        let is_sized_array = self
                            .lookup_struct_field_type(struct_name, name)
                            .map(|t| matches!(t, Type::TyFixedArray(..)))
                            .unwrap_or(false);
                        if is_sized_array {
                            if let Expr::ArrayLit(elems, _) = val {
                                let elems_str: Vec<String> = elems
                                    .iter()
                                    .map(|e| self.emit_expr(e, current_fn))
                                    .collect();
                                return format!(".{} = {{ {} }}", name, elems_str.join(", "));
                            }
                        }
                        let val_str = self.emit_expr(val, current_fn);
                        format!(".{} = {}", name, val_str)
                    })
                    .collect();
                format!("({}){{ {} }}", struct_name, fields_str.join(", "))
            }
        }
    }

    /// Normalize a literal index expression: negative values are
    /// wrapped from the array end (`-1` → `size - 1`).  Returns
    /// `None` when the index is not a compile-time literal.
    fn normalize_literal_index(&self, index: &Expr, target: &Expr) -> Option<String> {
        let sz = match self.typeof_expr(target) {
            Some(Type::TyFixedArray(_, n)) => n,
            _ => return None,
        };
        match index {
            Expr::IntLit(val, _) if *val < 0 => {
                let abs = (-val) as u64;
                if abs <= sz {
                    Some((sz - abs).to_string())
                } else {
                    None
                }
            }
            Expr::BinaryOp {
                left,
                op: BinOp::Sub,
                right,
                ..
            } => {
                if let Expr::IntLit(0, _) = left.as_ref() {
                    if let Expr::IntLit(val, _) = right.as_ref() {
                        let abs = *val as u64;
                        if abs <= sz {
                            return Some((sz - abs).to_string());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Emit a sub-expression that appears as a child of a binary
    /// operation.  Wrap in parens if the child is itself a binary
    /// op (needed for correct C precedence when parent has lower
    /// precedence than child, e.g. `(a + b) * c`).
    /// Infer the type of an expression for codegen purposes.
    /// Returns `None` when the type cannot be determined.
    fn typeof_expr(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::StringLit(..) => Some(Type::String),
            Expr::CharLit(..) => Some(Type::U8),
            Expr::BoolLit(..) => Some(Type::Bool),
            Expr::IntLit(..) => Some(Type::I32),
            Expr::FloatLit(..) => Some(Type::F64),
            Expr::Ident(name, _) => self.lookup_var_type(name),
            Expr::FieldAccess { target, field, .. } => {
                if let Expr::Ident(obj, _) = target.as_ref() {
                    if let Some(Type::Struct(sname)) = self.lookup_var_type(obj) {
                        return self.lookup_struct_field_type(&sname, field);
                    }
                }
                None
            }
            Expr::Call { name, .. } => self.fn_returns.get(name).and_then(|ret| {
                if ret.len() == 1 {
                    Some(ret[0].clone())
                } else {
                    None
                }
            }),
            Expr::BinaryOp { left, op, .. } => {
                let lt = self.typeof_expr(left)?;
                if is_string_type(&lt) {
                    match op {
                        BinOp::Add => Some(lt),
                        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                            Some(Type::Bool)
                        }
                        _ => Some(lt),
                    }
                } else {
                    Some(lt)
                }
            }
            Expr::Index { target, .. } => {
                let t = self.typeof_expr(target);
                match t {
                    Some(Type::String) | Some(Type::String16) | Some(Type::String32) => {
                        Some(Type::U32)
                    }
                    Some(Type::TyArray(elem, _)) => Some(*elem),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Convert a binary operator to its C string representation.
    fn binop_to_str(&self, op: &BinOp) -> &'static str {
        match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
        }
    }

    fn emit_binary_child(&mut self, expr: &Expr, current_fn: &str) -> String {
        let s = self.emit_expr(expr, current_fn);
        if matches!(expr, Expr::BinaryOp { .. }) {
            format!("({})", s)
        } else {
            s
        }
    }

    // ── Call emission ────────────────────────────────────────

    /// Emit a function call.
    ///
    /// `println` and `printf` are special-cased to map to C's
    /// `printf` with appropriate formatting.
    fn emit_call(&mut self, name: &str, args: &[Expr], current_fn: &str) -> String {
        if name == "println" {
            self.emit_println(args, current_fn)
        } else if name == "printf" {
            self.emit_printf(args, current_fn)
        } else {
            let returns = self.fn_returns.get(name);
            match returns {
                Some(ret_types) if ret_types.len() > 1 => {
                    // Multi-return call in expression context — just emit
                    // the call with args (output params handled by DeclAssign).
                    let args_str = self.emit_call_args_joined(args, current_fn);
                    format!("{}({})", name, args_str)
                }
                _ => {
                    let args_str = self.emit_call_args_joined(args, current_fn);
                    format!("{}({})", name, args_str)
                }
            }
        }
    }

    /// Join all call arguments into a comma-separated string.
    fn emit_call_args_joined(&mut self, args: &[Expr], current_fn: &str) -> String {
        args.iter()
            .map(|a| self.emit_expr(a, current_fn))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Extract arguments from a `Call` expression (used by multi-assign).
    fn emit_call_args(&mut self, expr: &Expr, current_fn: &str) -> String {
        if let Expr::Call { args, .. } = expr {
            args.iter()
                .map(|a| self.emit_expr(a, current_fn))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            String::new()
        }
    }

    // ── Built-in print ───────────────────────────────────────

    /// Emit a `println` call as `printf("...\n")`.
    fn emit_println(&mut self, args: &[Expr], current_fn: &str) -> String {
        if args.len() == 1 {
            if let Expr::StringLit(val, _) = &args[0] {
                let escaped = val
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\t', "\\t");
                return format!("printf(\"{}\\n\")", escaped);
            }
            if let Expr::Ident(name, _) = &args[0] {
                if let Some(typ) = self.lookup_var_type(name) {
                    if typ == Type::String {
                        return format!("printf(\"%s\\n\", {}.data)", name);
                    }
                    if typ == Type::String16 {
                        let conv = format!("nitid_string16_to_utf8(&{})", name);
                        return format!(
                            "{{ nitid_string _nitid_utf8 = {}; printf(\"%s\\n\", _nitid_utf8.data); nitid_string_free(&_nitid_utf8); }}",
                            conv
                        );
                    }
                    if typ == Type::String32 {
                        let conv = format!("nitid_string32_to_utf8(&{})", name);
                        return format!(
                            "{{ nitid_string _nitid_utf8 = {}; printf(\"%s\\n\", _nitid_utf8.data); nitid_string_free(&_nitid_utf8); }}",
                            conv
                        );
                    }
                }
            }
            // Handle string-typed expressions (concat, etc.)
            if let Some(typ) = self.typeof_expr(&args[0]) {
                if typ == Type::String {
                    let expr_str = self.emit_expr(&args[0], current_fn);
                    return format!(
                        "{{ nitid_string _nitid_str = {}; printf(\"%s\\n\", _nitid_str.data); nitid_string_free(&_nitid_str); }}",
                        expr_str
                    );
                }
                if typ == Type::String16 {
                    let expr_str = self.emit_expr(&args[0], current_fn);
                    return format!(
                        "{{ nitid_string16 _nitid_s16 = {}; nitid_string _nitid_utf8 = nitid_string16_to_utf8(&_nitid_s16); printf(\"%s\\n\", _nitid_utf8.data); nitid_string_free(&_nitid_utf8); nitid_string16_free(&_nitid_s16); }}",
                        expr_str
                    );
                }
                if typ == Type::String32 {
                    let expr_str = self.emit_expr(&args[0], current_fn);
                    return format!(
                        "{{ nitid_string32 _nitid_s32 = {}; nitid_string _nitid_utf8 = nitid_string32_to_utf8(&_nitid_s32); printf(\"%s\\n\", _nitid_utf8.data); nitid_string_free(&_nitid_utf8); nitid_string32_free(&_nitid_s32); }}",
                        expr_str
                    );
                }
            }
        }
        let args_str: Vec<String> = args.iter().map(|a| self.emit_expr(a, current_fn)).collect();
        if args_str.len() == 1 {
            format!("printf(\"%s\\n\", {})", args_str[0])
        } else {
            format!("printf({})", args_str.join(", "))
        }
    }

    /// Emit a `printf` call.
    fn emit_printf(&mut self, args: &[Expr], current_fn: &str) -> String {
        let args_str: Vec<String> = args
            .iter()
            .map(|a| -> String {
                // If the printf arg is an i128/u128 var name, in C it must be type cast to "long long"
                let emitted = self.emit_expr(a, current_fn);
                let typ = self.lookup_var_type(&emitted);
                // eprintln!("emit_printf: var= {}, type={:?}", emitted, typ);
                if matches!(a, Expr::Ident(..)) {
                    if typ == Some(Type::I128) {
                        return format!("(long long){}", emitted);
                    }
                    if typ == Some(Type::U128) {
                        return format!("(unsigned long long){}", emitted);
                    }
                }
                return emitted;
            })
            .collect();

        // .map(|a| self.emit_expr(a, current_fn)).collect();
        format!("printf({})", args_str.join(", "))
    }

    // ── String literal emission ──────────────────────────────

    /// Escape and quote a string literal for C.
    fn emit_string_lit(&self, val: &str) -> String {
        let escaped = val
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('\r', "\\r")
            .replace('\0', "\\0");
        format!("\"{}\"", escaped)
    }

    /// Emit a char literal as a valid C character constant.
    fn emit_char_lit(&self, val: u8) -> String {
        match val {
            0 => "'\\0'".to_string(),
            b'\n' => "'\\n'".to_string(),
            b'\t' => "'\\t'".to_string(),
            b'\r' => "'\\r'".to_string(),
            b'\\' => "'\\\\'".to_string(),
            b'\'' => "'\\''".to_string(),
            0x20..=0x7e => format!("'{}'", val as char),
            _ => format!("'\\x{:02x}'", val),
        }
    }

    /// Guess a C type string for an expression (used for `:=` inference).
    fn infer_expr_type_str(&self, expr: &Expr) -> String {
        match expr {
            Expr::IntLit(..) => "int".to_string(),
            Expr::FloatLit(..) => "double".to_string(),
            Expr::StringLit(..) => "nitid_string".to_string(),
            Expr::CharLit(..) => "uint8_t".to_string(),
            Expr::BoolLit(..) => "bool".to_string(),
            Expr::ArrayLit(..) => "nitid_array".to_string(),
            Expr::Index { target, .. } => self.infer_array_elem_type(target).c_str().to_string(),
            Expr::StructLit { struct_name, .. } => struct_name.clone(),
            _ => "int".to_string(),
        }
    }

    /// Map a Type to the suffix used in nitid_array_from_lit_<suffix>
    /// and nitid_array_get_<suffix> runtime function names.
    fn array_type_suffix(&self, t: &Type) -> &'static str {
        match t {
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::I128 => "i128",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::U128 => "u128",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Bool => "bool",
            _ => "i32",
        }
    }

    /// Attempt to infer the element type of an array expression.
    fn infer_array_elem_type(&self, expr: &Expr) -> Type {
        self.infer_array_info(expr)
            .map(|(elem, _)| elem)
            .unwrap_or(Type::I32)
    }

    /// Infer both element type and whether the array is sized
    /// (has a compile-time known size).
    fn infer_array_info(&self, expr: &Expr) -> Option<(Type, bool)> {
        match expr {
            Expr::Ident(name, _) => self.lookup_var_type(name).and_then(|t| match t {
                Type::TyFixedArray(elem, _) => Some((*elem, true)),
                Type::TyArray(elem, _) => Some((*elem, false)),
                _ => None,
            }),
            Expr::FieldAccess { target, field, .. } => {
                let target_type = match target.as_ref() {
                    Expr::Ident(name, _) => self.lookup_var_type(name),
                    _ => None,
                }?;
                match target_type {
                    Type::Struct(sname) => {
                        self.lookup_struct_field_type(&sname, field)
                            .and_then(|t| match t {
                                Type::TyFixedArray(elem, _) => Some((*elem, true)),
                                Type::TyArray(elem, _) => Some((*elem, false)),
                                _ => None,
                            })
                    }
                    _ => None,
                }
            }
            Expr::ArrayLit(elems, _) => {
                if elems.is_empty() {
                    None
                } else {
                    Some((self.infer_init_type(&elems[0]), false))
                }
            }
            _ => None,
        }
    }

    // ── Scope management ─────────────────────────────────────

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: &str, typ: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), typ);
        }
    }

    fn lookup_var_type(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    /// Look up the type of a struct field.
    fn lookup_struct_field_type(&self, struct_name: &str, field_name: &str) -> Option<Type> {
        for (name, fields, _, _) in &self.struct_defs {
            if name == struct_name {
                return fields
                    .iter()
                    .find(|f| f.name == field_name)
                    .map(|f| f.typ.clone());
            }
        }
        None
    }

    // ── String runtime helpers ───────────────────────────────

    fn emit_string_as_value(&mut self, expr: &Expr, typ: &Type, current_fn: &str) -> String {
        match expr {
            Expr::StringLit(val, _) => {
                let lit = self.emit_string_lit(val);
                match typ {
                    Type::String => format!("nitid_string_from({})", lit),
                    Type::String16 => format!("nitid_string16_from_utf8({})", lit),
                    Type::String32 => format!("nitid_string32_from_utf8({})", lit),
                    _ => lit,
                }
            }
            other => self.emit_expr(other, current_fn),
        }
    }

    // ── CMake emission ───────────────────────────────────────

    // ── Enum helpers ──────────────────────────────────────────
}

/// Convert an enum variant value expression to a C integer literal string.
fn enum_value_to_c(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(val, _) => val.to_string(),
        Expr::BinaryOp {
            left,
            op: BinOp::Sub,
            right,
            ..
        } => {
            if let Expr::IntLit(0, _) = left.as_ref() {
                if let Expr::IntLit(rval, _) = right.as_ref() {
                    return format!("{}", -rval);
                }
            }
            "0".to_string()
        }
        _ => "0".to_string(),
    }
}
