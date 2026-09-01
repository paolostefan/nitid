use crate::PackageContext;
use crate::ast::*;
use crate::types::{Type, is_string_type};
/// Semantic analyser for Nitid.
///
/// Phase 3 of the transpilation pipeline: validates the AST produced
/// by the parser and annotates it with type information.
///
/// # Checks performed
/// * Variable scoping (declare / lookup in nested scopes).
/// * Return-value count matches the function signature.
/// * Binary operator type compatibility (both operands must have the
///   same type for arithmetic / bitwise ops).
/// * Function call arity and existence.
///
/// # Checks NOT yet performed (TODO)
/// * No implicit type casts → **enforced**: literal values are
///   range-checked against the declared numeric type.
/// * Uninitialised variables → **not enforced**: only a warning is
///   issued for `:=` declarations with inferred types.
/// * Overflow / underflow protection.
/// * Dead code detection.
use std::collections::HashMap;

/// Info about a struct definition: its fields and layout attributes.
#[derive(Debug, Clone)]
struct StructInfo {
    fields: Vec<StructField>,
    packed: bool,
    align: Option<u64>,
}

/// Info about an enum definition: its underlying type and variants.
#[derive(Debug, Clone)]
struct EnumInfo {
    underlying_type: Type,
    variants: Vec<EnumVariant>,
}

/// The semantic analyzer context.
///
/// Holds the global scope, a reference to the current function's
/// return types, and accumulates warnings during analysis.
pub struct Sema {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    globals: Scope,
    current_fn_returns: Vec<Type>,
    multi_return_temps: HashMap<String, Vec<String>>,
    /// Stack of uninitialized variable sets. Each level tracks vars
    /// declared (without init) in that scope. Variables are removed
    /// when assigned. At scope exit, remaining vars produce an error.
    uninit_scopes: Vec<HashMap<String, Span>>,
    /// Current loop nesting depth. break/continue only valid when > 0.
    loop_depth: usize,
    /// Collected struct definitions.
    struct_defs: HashMap<String, StructInfo>,
    /// Methods per struct: struct_name -> Vec<(method_name, FnDecl)>.
    struct_methods: HashMap<String, Vec<FnDecl>>,
    /// Whether we are currently analyzing a method body (for self injection).
    current_struct_type: Option<Type>,
    /// Collected enum definitions.
    enum_defs: HashMap<String, EnumInfo>,
    /// Enum member lookup: member_name -> (underlying_type, value).
    enum_members: HashMap<String, (Type, i128)>,
    /// Imported functions signatures (from PackageContext)
    fn_sigs_map: HashMap<String, (Vec<Type>, Vec<Type>)>,
    /// Imported struct definitions
    imported_struct_defs: HashMap<String, StructInfo>,
    /// Per-package functions for qualified access (eg. Math.multiply()).
    /// Key: imported package name. Value: that package's functions
    package_functions: HashMap<String, HashMap<String, (Vec<Type>, Vec<Type>)>>,
}

/// A lexical scope mapping variable names to their types.
///
/// Scopes form a chain: each scope has an optional parent.  Variable
/// lookup walks up the chain.
struct Scope {
    vars: HashMap<String, Type>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    /// Create an empty scope with no parent.
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            parent: None,
        }
    }

    /// Create a child scope with `parent` as the enclosing scope.
    fn child(parent: Scope) -> Self {
        Self {
            vars: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Declare a variable in *this* scope only.
    fn declare(&mut self, name: &str, typ: Type, span: &Span) -> Result<(), String> {
        if self.vars.contains_key(name) {
            return Err(format!(
                "{}:{}:{}: Variable '{}' already declared in this scope",
                span.file, span.line, span.col, name
            ));
        }
        self.vars.insert(name.to_string(), typ);
        Ok(())
    }

    /// Look up a variable, checking this scope first, then parents.
    fn lookup(&self, name: &str) -> Option<Type> {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup(name)))
    }
}

impl Sema {
    /// Create a new semantic analyzer with an empty global scope.
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
            errors: Vec::new(),
            globals: Scope::new(),
            current_fn_returns: Vec::new(),
            multi_return_temps: HashMap::new(),
            uninit_scopes: Vec::new(),
            loop_depth: 0,
            struct_defs: HashMap::new(),
            struct_methods: HashMap::new(),
            current_struct_type: None,
            enum_defs: HashMap::new(),
            enum_members: HashMap::new(),
            fn_sigs_map: HashMap::new(),
            imported_struct_defs: HashMap::new(),
            package_functions: HashMap::new(),
        }
    }

    /// Run semantic analysis on the entire program.
    pub fn analyze(
        &mut self,
        program: &mut Program,
        packages: Option<&HashMap<String, PackageContext>>,
    ) -> Result<(), String> {
        // Pre-populate from imported package context.
        if let Some(pkgs) = packages {
            // Bare-call support: flatten everything
            for ctx in pkgs.values() {
                for (name, sig) in &ctx.functions {
                    self.fn_sigs_map.insert(
                        name.clone(),
                        (sig.param_types.clone(), sig.return_types.clone()),
                    );
                }

                for (name, fields) in &ctx.structs {
                    self.imported_struct_defs.insert(
                        name.clone(),
                        StructInfo {
                            fields: fields
                                .iter()
                                .map(|(n, t)| StructField {
                                    name: n.clone(),
                                    typ: t.clone(),
                                    span: Span::new("", 0, 0),
                                })
                                .collect(),
                            packed: false,
                            align: None,
                        },
                    );
                }
            }

            // Qualified-access support: keep per-package namespaces.
            for (pkg_name, ctx) in pkgs {
                let mut fns = HashMap::new();
                for (name, sig) in &ctx.functions {
                    fns.insert(
                        name.clone(),
                        (sig.param_types.clone(), sig.return_types.clone()),
                    );
                }
                self.package_functions.insert(pkg_name.clone(), fns);
            }
        }
        self.analyze_decls(&mut program.decls)
    }

    // ── Declarations ──────────────────────────────────────────

    /// Analyze all top-level declarations.
    ///
    /// Pre-pass: collect struct definitions.
    /// Pass 1: collect function & method signatures for call resolution.
    /// Pass 2: analyze each declaration body.
    fn analyze_decls(&mut self, decls: &mut [Decl]) -> Result<(), String> {
        // Pre-pass: collect struct and enum definitions.

        // Start with imported struct defs (shadowed by local ones)
        for (name, info) in &self.imported_struct_defs {
            self.struct_defs
                .entry(name.clone())
                .or_insert_with(|| info.clone());
        }

        for decl in decls.iter() {
            match decl {
                Decl::StructDecl(s) => {
                    let name = s.name.clone();
                    if self.struct_defs.contains_key(&name) {
                        return Err(format!(
                            "{}:{}:{}: Duplicate struct definition '{}'",
                            s.span.file, s.span.line, s.span.col, name
                        ));
                    }
                    self.struct_defs.insert(
                        name,
                        StructInfo {
                            fields: s.fields.clone(),
                            packed: s.packed,
                            align: s.align,
                        },
                    );
                }
                Decl::EnumDecl(e) => {
                    let name = e.name.clone();
                    if self.enum_defs.contains_key(&name) {
                        return Err(format!(
                            "{}:{}:{}: Duplicate enum definition '{}'",
                            e.span.file, e.span.line, e.span.col, name
                        ));
                    }
                    // Determine underlying type (default i32).
                    let underlying = e.typ.clone().unwrap_or(Type::I32);
                    // Only integral types allowed.
                    if !is_integral_type(&underlying) {
                        return Err(format!(
                            "{}:{}:{}: Enum '{}' must use an integral type, got '{}'",
                            e.span.file, e.span.line, e.span.col, name, underlying
                        ));
                    }
                    // Check for duplicate variant names and evaluate values.
                    let mut seen = std::collections::HashSet::new();
                    let mut next_val: i128 = 0;
                    for v in &e.variants {
                        if !seen.insert(v.name.clone()) {
                            return Err(format!(
                                "{}:{}:{}: Duplicate variant '{}' in enum '{}'",
                                v.span.file, v.span.line, v.span.col, v.name, name
                            ));
                        }
                        let val = if let Some(ref expr) = v.value {
                            eval_enum_value(expr, &v.span)?
                        } else {
                            next_val
                        };
                        // Check value fits in the underlying type.
                        if !int_lit_fits(val, &underlying) {
                            return Err(format!(
                                "{}:{}:{}: Value {} does not fit in enum '{}' type '{}'",
                                v.span.file, v.span.line, v.span.col, val, name, underlying
                            ));
                        }
                        self.enum_members
                            .insert(v.name.clone(), (Type::Enum(name.clone()), val));
                        next_val = val.checked_add(1).ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Enum '{}' variant values overflow",
                                v.span.file, v.span.line, v.span.col, name
                            )
                        })?;
                    }
                    self.enum_defs.insert(
                        name,
                        EnumInfo {
                            underlying_type: underlying,
                            variants: e.variants.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        // Pass 1: gather all function signatures and method signatures.
        let mut fn_sigs: HashMap<String, (Vec<Type>, Vec<Type>)> = HashMap::new();

        // Start with imported signatures (they're "below" in scope).
        for (name, sig) in &self.fn_sigs_map {
            fn_sigs.insert(name.clone(), sig.clone());
        }

        for decl in decls.iter() {
            match decl {
                Decl::FnDecl(f) => {
                    let param_types: Vec<Type> = f
                        .params
                        .iter()
                        .flat_map(|p| std::iter::repeat(p.typ.clone()).take(p.names.len()))
                        .collect();
                    fn_sigs.insert(f.name.clone(), (param_types, f.returns.clone()));
                }
                Decl::ImplBlock(imp) => {
                    let struct_name = &imp.struct_name;
                    if !self.struct_defs.contains_key(struct_name) {
                        return Err(format!(
                            "{}:{}:{}: Cannot define methods for unknown struct '{}'",
                            imp.span.file, imp.span.line, imp.span.col, struct_name
                        ));
                    }
                    for method in &imp.methods {
                        let m_name = method.name.clone();
                        // Check for duplicate method names within this impl block
                        // (will be caught by the struct_methods insert below)
                        let param_types: Vec<Type> = method
                            .params
                            .iter()
                            .flat_map(|p| std::iter::repeat(p.typ.clone()).take(p.names.len()))
                            .collect();
                        // Register method in fn_sigs with mangled name
                        let mangled = format!("{}_{}", struct_name, m_name);
                        // Method gets an implicit self pointer parameter in C
                        let mut c_params = vec![Type::Struct(struct_name.clone())];
                        c_params.extend(param_types);
                        fn_sigs.insert(mangled, (c_params, method.returns.clone()));
                    }
                    self.struct_methods
                        .insert(struct_name.clone(), imp.methods.clone());
                }
                _ => {}
            }
        }

        // Pass 2: analyze bodies.
        for decl in decls.iter_mut() {
            match decl {
                Decl::FnDecl(f) => {
                    self.current_fn_returns = f.returns.clone();
                    self.current_struct_type = None;
                    let mut scope = Scope::new();
                    for param in &f.params {
                        for name in &param.names {
                            scope.declare(name, param.typ.clone(), &param.span)?;
                        }
                    }
                    self.analyze_block(&mut f.body, &mut scope, &fn_sigs)?;
                }
                Decl::ImplBlock(imp) => {
                    let struct_name = imp.struct_name.clone();
                    let struct_type = Type::Struct(struct_name.clone());
                    for method in &mut imp.methods {
                        self.current_fn_returns = method.returns.clone();
                        self.current_struct_type = Some(struct_type.clone());
                        let mut scope = Scope::new();
                        // Declare self as the struct type
                        let self_span = method.span.clone();
                        scope.declare("self", struct_type.clone(), &self_span)?;
                        for param in &method.params {
                            for name in &param.names {
                                scope.declare(name, param.typ.clone(), &param.span)?;
                            }
                        }
                        self.analyze_block(&mut method.body, &mut scope, &fn_sigs)?;
                    }
                    self.current_struct_type = None;
                }
                Decl::VarDecl(v) => {
                    analyze_var_decl_free(
                        v,
                        &mut self.globals,
                        &fn_sigs,
                        &mut self.warnings,
                        &self.struct_defs,
                        &self.enum_members,
                    )?;
                    let can_zero_init = v
                        .typ
                        .as_ref()
                        .map(|t| matches!(t, Type::TyArray(..) | Type::TyFixedArray(..)))
                        .unwrap_or(false);
                    if v.init.is_none() && !can_zero_init {
                        for name in &v.names {
                            return Err(format!(
                                "{}:{}:{}: Variable '{}' must be initialized",
                                v.span.file, v.span.line, v.span.col, name
                            ));
                        }
                    }
                }
                Decl::StructDecl(_) | Decl::EnumDecl(_) => {} // already handled
            }
        }
        Ok(())
    }

    // ── Blocks & statements ──────────────────────────────────

    /// Analyze a block of statements, creating a nested scope.
    fn analyze_block(
        &mut self,
        stmts: &mut [Stmt],
        scope: &mut Scope,
        fn_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    ) -> Result<(), String> {
        self.push_uninit_scope();
        let mut local_scope = Scope::child(std::mem::replace(scope, Scope::new()));
        for stmt in stmts.iter_mut() {
            self.analyze_stmt(stmt, &mut local_scope, fn_sigs)?;
        }
        *scope = *local_scope.parent.unwrap();
        self.pop_uninit_scope()
    }

    /// Analyze a single statement.
    fn analyze_stmt(
        &mut self,
        stmt: &mut Stmt,
        scope: &mut Scope,
        fn_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    ) -> Result<(), String> {
        match stmt {
            Stmt::VarDecl(v) => {
                analyze_var_decl_free(
                    v,
                    scope,
                    fn_sigs,
                    &mut self.warnings,
                    &self.struct_defs,
                    &self.enum_members,
                )?;
                // Deferred init check: vars without init are tracked
                // per scope and checked at scope exit.
                // Array types can be zero-initialized so skip uninit tracking.
                let can_zero_init = v
                    .typ
                    .as_ref()
                    .map(|t| matches!(t, Type::TyArray(..) | Type::TyFixedArray(..)))
                    .unwrap_or(false);
                if v.init.is_none() && !can_zero_init {
                    for name in &v.names {
                        self.add_uninit(name, &v.span);
                    }
                }
                Ok(())
            }
            Stmt::Expr(e) => {
                self.analyze_expr(e, scope, fn_sigs)?;
                Ok(())
            }
            Stmt::Return(values, span) => {
                // Check that the number of returned values matches the
                // function signature.
                if values.len() != self.current_fn_returns.len() {
                    return Err(format!(
                        "{}:{}:{}: Return {} values but function expects {}",
                        span.file,
                        span.line,
                        span.col,
                        values.len(),
                        self.current_fn_returns.len()
                    ));
                }
                for val in values.iter_mut() {
                    self.analyze_expr(val, scope, fn_sigs)?;
                }
                Ok(())
            }
            Stmt::Block(body) => {
                self.push_uninit_scope();
                let mut inner = Scope::child(std::mem::replace(scope, Scope::new()));
                for stmt in body.iter_mut() {
                    self.analyze_stmt(stmt, &mut inner, fn_sigs)?;
                }
                *scope = *inner.parent.unwrap();
                self.pop_uninit_scope()
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.analyze_expr(cond, scope, fn_sigs)?;
                self.analyze_block(then_block, scope, fn_sigs)?;
                if let Some(else_b) = else_block {
                    self.analyze_block(else_b, scope, fn_sigs)?;
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                self.analyze_expr(cond, scope, fn_sigs)?;
                self.loop_depth += 1;
                self.analyze_block(body, scope, fn_sigs)?;
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::For {
                init,
                cond,
                inc,
                body,
                ..
            } => {
                if let Some(init_stmt) = init {
                    self.analyze_stmt(init_stmt, scope, fn_sigs)?;
                }
                if let Some(cond_expr) = cond {
                    self.analyze_expr(cond_expr, scope, fn_sigs)?;
                }
                if let Some(inc_expr) = inc {
                    self.analyze_expr(inc_expr, scope, fn_sigs)?;
                }
                self.loop_depth += 1;
                self.analyze_block(body, scope, fn_sigs)?;
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::ForIn {
                var,
                iter,
                body,
                span,
            } => {
                let iter_type = self.analyze_expr(iter, scope, fn_sigs)?;
                if !matches!(iter_type, Type::TyArray(..) | Type::TyFixedArray(..)) {
                    return Err(format!(
                        "{}:{}:{}: Cannot iterate over non-array type",
                        span.file, span.line, span.col
                    ));
                }
                self.push_uninit_scope();
                let mut inner = Scope::child(std::mem::replace(scope, Scope::new()));
                inner.declare(var, Type::I32, span)?;
                self.loop_depth += 1;
                for stmt in body.iter_mut() {
                    self.analyze_stmt(stmt, &mut inner, fn_sigs)?;
                }
                self.loop_depth -= 1;
                *scope = *inner.parent.unwrap();
                self.pop_uninit_scope()
            }
            Stmt::ForInIndex {
                idx_var,
                item_var,
                iter,
                body,
                span,
            } => {
                let iter_type = self.analyze_expr(iter, scope, fn_sigs)?;
                if !matches!(iter_type, Type::TyArray(..) | Type::TyFixedArray(..)) {
                    return Err(format!(
                        "{}:{}:{}: Cannot iterate over non-array type",
                        span.file, span.line, span.col
                    ));
                }
                self.push_uninit_scope();
                let mut inner = Scope::child(std::mem::replace(scope, Scope::new()));
                inner.declare(idx_var, Type::I32, span)?;
                inner.declare(item_var, Type::I32, span)?;
                self.loop_depth += 1;
                for stmt in body.iter_mut() {
                    self.analyze_stmt(stmt, &mut inner, fn_sigs)?;
                }
                self.loop_depth -= 1;
                *scope = *inner.parent.unwrap();
                self.pop_uninit_scope()
            }
            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    return Err(format!(
                        "{}:{}:{}: 'break' outside of loop",
                        span.file, span.line, span.col
                    ));
                }
                Ok(())
            }
            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    return Err(format!(
                        "{}:{}:{}: 'continue' outside of loop",
                        span.file, span.line, span.col
                    ));
                }
                Ok(())
            }
        }
    }

    // ── Expressions ──────────────────────────────────────────

    fn analyze_expr(
        &mut self,
        expr: &mut Expr,
        scope: &mut Scope,
        fn_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    ) -> Result<Type, String> {
        let typ = self.infer_expr_type(expr, scope, fn_sigs)?;
        // Track assignments to initialize variables.
        if let Expr::Assign { left, .. } = expr {
            if let Expr::Ident(name, _) = left.as_ref() {
                self.mark_initialized(name);
            }
        }
        if let Expr::PostIncrement { target, .. } | Expr::PostDecrement { target, .. } = expr {
            if let Expr::Ident(name, _) = target.as_ref() {
                self.mark_initialized(name);
            }
        }
        Ok(typ)
    }

    /// Infer the type of an expression.
    ///
    /// This is the core type-checking function.  It recursively walks
    /// the expression tree and:
    /// - Assigns fixed types to literals (int → `I32`, float → `F64`, etc.).
    /// - Resolves identifiers against the scope chain.
    /// - Validates function call arguments and looks up the return type.
    /// - Checks binary-operator type compatibility.
    fn infer_expr_type(
        &self,
        expr: &Expr,
        scope: &Scope,
        fn_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    ) -> Result<Type, String> {
        match expr {
            Expr::IntLit(..) => Ok(Type::I32),
            Expr::FloatLit(..) => Ok(Type::F64),
            Expr::StringLit(..) => Ok(Type::String),
            Expr::CharLit(..) => Ok(Type::U8),
            Expr::BoolLit(..) => Ok(Type::Bool),
            Expr::Ident(name, span) => scope
                .lookup(name)
                .or_else(|| self.enum_members.get(name).map(|(t, _)| t.clone()))
                .ok_or_else(|| {
                    format!(
                        "{}:{}:{}: Undefined variable '{}'",
                        span.file, span.line, span.col, name
                    )
                }),
            Expr::Call { name, args, span } => {
                for arg in args {
                    self.infer_expr_type(arg, scope, fn_sigs)?;
                }
                // Built-in functions.
                if name == "println" || name == "printf" {
                    Ok(Type::Void)
                } else if let Some(typ) = Type::from_str(name) {
                    // Type conversion: `i64(expr)` returns the target type
                    Ok(typ)
                } else {
                    fn_sigs
                        .get(name)
                        .map(|(_, returns)| {
                            if returns.len() == 1 {
                                returns[0].clone()
                            } else {
                                // Multi-return functions have void type in expression
                                // context; the return values are written through
                                // output pointer arguments.
                                Type::Void
                            }
                        })
                        .ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Undefined function '{}'",
                                span.file, span.line, span.col, name
                            )
                        })
                }
            }
            Expr::BinaryOp {
                left,
                right,
                op,
                span,
            } => {
                let lt = self.infer_expr_type(left, scope, fn_sigs)?;
                let rt = self.infer_expr_type(right, scope, fn_sigs)?;
                let lt_r = resolve_enum_type(&lt, &self.enum_defs).clone();
                let rt_r = resolve_enum_type(&rt, &self.enum_defs).clone();
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        if lt_r == rt_r {
                            Ok(lt_r)
                        } else {
                            Err(format!(
                                "{}:{}:{}: Type mismatch in arithmetic",
                                span.file, span.line, span.col
                            ))
                        }
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        Ok(Type::Bool)
                    }
                    BinOp::And | BinOp::Or => Ok(Type::Bool),
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        if lt_r == rt_r {
                            Ok(lt_r)
                        } else {
                            Err(format!(
                                "{}:{}:{}: Type mismatch in bitwise op",
                                span.file, span.line, span.col
                            ))
                        }
                    }
                }
            }
            Expr::Assign { left, .. } => self.infer_expr_type(left, scope, fn_sigs),
            Expr::DeclAssign { right, .. } => self.infer_expr_type(right, scope, fn_sigs),
            Expr::ArrayLit(elems, span) => {
                if elems.is_empty() {
                    return Err(format!(
                        "{}:{}:{}: Empty array literals not supported yet",
                        span.file, span.line, span.col
                    ));
                }
                let elem_type = self.infer_expr_type(&elems[0], scope, fn_sigs)?;
                for e in elems[1..].iter() {
                    let et = self.infer_expr_type(e, scope, fn_sigs)?;
                    if et != elem_type {
                        return Err(format!(
                            "{}:{}:{}: Array element type mismatch",
                            span.file, span.line, span.col
                        ));
                    }
                }
                Ok(Type::TyArray(Box::new(elem_type), None))
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                let target_type = self.infer_expr_type(target, scope, fn_sigs)?;
                let index_type = self.infer_expr_type(index, scope, fn_sigs)?;
                if !is_numeric_type(&index_type) {
                    return Err(format!(
                        "{}:{}:{}: Array index must be numeric",
                        span.file, span.line, span.col
                    ));
                }
                match target_type {
                    // Fixed-size arrays: check compile-time known index bounds.
                    // Negative indices wrap from end (-1 → last element, -n → index 0)
                    Type::TyFixedArray(elem, sz) => {
                        check_index_bounds(&elem, Some(sz), index, span)
                    }
                    // Dynamically-sized arrays can be resized at runtime, so
                    // the declared size is not a compile-time bound; illegal
                    // accesses are caught at runtime instead.
                    Type::TyArray(elem, _) => Ok(*elem),
                    Type::String | Type::String16 | Type::String32 => Ok(Type::U32),
                    _ => Err(format!(
                        "{}:{}:{}: Cannot index non-array type",
                        span.file, span.line, span.col
                    )),
                }
            }
            Expr::PostIncrement { target, span } | Expr::PostDecrement { target, span } => {
                let t = self.infer_expr_type(target, scope, fn_sigs)?;
                if !is_numeric_type(&t) {
                    return Err(format!(
                        "{}:{}:{}: Cannot apply ++ or -- to non-numeric type {:?}",
                        span.file, span.line, span.col, t
                    ));
                }
                Ok(t)
            }
            Expr::FieldAccess {
                target,
                field,
                span,
            } => {
                let target_type = self.infer_expr_type(target, scope, fn_sigs)?;
                match target_type {
                    Type::Struct(ref struct_name) => {
                        let info = self.struct_defs.get(struct_name).ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Unknown struct type '{}'",
                                span.file, span.line, span.col, struct_name
                            )
                        })?;
                        info.fields
                            .iter()
                            .find(|f| f.name == *field)
                            .map(|f| f.typ.clone())
                            .ok_or_else(|| {
                                format!(
                                    "{}:{}:{}: Struct '{}' has no field '{}'",
                                    span.file, span.line, span.col, struct_name, field
                                )
                            })
                    }
                    _ => Err(format!(
                        "{}:{}:{}: Cannot access field on non-struct type",
                        span.file, span.line, span.col
                    )),
                }
            }
            Expr::MethodCall {
                target,
                method,
                args,
                span,
            } => {
                // Imports: qualified package call, e.g. Math.multiply(25, 14).
                // The receiver is an Ident naming an imported package.
                if let Expr::Ident(pkg_name, _) = target.as_ref() {
                    if let Some(pkg_fns) = self.package_functions.get(pkg_name) {
                        // Validate argument types first.
                        for arg in args {
                            self.infer_expr_type(arg, scope, fn_sigs)?;
                        }

                        return pkg_fns
                            .get(method)
                            .map(|(_, returns)| {
                                if returns.len() == 1 {
                                    returns[0].clone()
                                } else {
                                    Type::Void
                                }
                            })
                            .ok_or_else(|| {
                                format!(
                                    "{}:{}:{}: Package '{}' has no function '{}'",
                                    span.file, span.line, span.col, pkg_name, method
                                )
                            });
                    }
                }

                // The receiver is not an imported package's name.
                let target_type = self.infer_expr_type(target, scope, fn_sigs)?;
                match target_type {
                    Type::Struct(ref struct_name) => {
                        let methods = self.struct_methods.get(struct_name).ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Struct '{}' has no methods",
                                span.file, span.line, span.col, struct_name
                            )
                        })?;
                        let method_decl =
                            methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                                format!(
                                    "{}:{}:{}: Struct '{}' has no method '{}'",
                                    span.file, span.line, span.col, struct_name, method
                                )
                            })?;
                        // Check method arity
                        let expected_params: Vec<&Type> = method_decl
                            .params
                            .iter()
                            .flat_map(|p| std::iter::repeat(&p.typ).take(p.names.len()))
                            .collect();
                        if args.len() != expected_params.len() {
                            return Err(format!(
                                "{}:{}:{}: Method '{}' expects {} arguments, got {}",
                                span.file,
                                span.line,
                                span.col,
                                method,
                                expected_params.len(),
                                args.len()
                            ));
                        }
                        // Check argument types (for now just validate they exist)
                        for arg in args {
                            self.infer_expr_type(arg, scope, fn_sigs)?;
                        }
                        // Return the method's return type
                        if method_decl.returns.len() == 1 {
                            Ok(method_decl.returns[0].clone())
                        } else if method_decl.returns.is_empty()
                            || method_decl.returns[0] == Type::Void
                        {
                            Ok(Type::Void)
                        } else {
                            Ok(Type::Void) // multi-return in expression context
                        }
                    }
                    Type::TyArray(..) | Type::TyFixedArray(..) => {
                        check_array_method(method, args, span, &target_type, |e| {
                            self.infer_expr_type(e, scope, fn_sigs)
                        })
                    }
                    _ => Err(format!(
                        "{}:{}:{}: Cannot call method on non-struct type",
                        span.file, span.line, span.col
                    )),
                }
            }
            Expr::StructLit {
                struct_name,
                fields,
                span,
            } => {
                let info = self.struct_defs.get(struct_name).ok_or_else(|| {
                    format!(
                        "{}:{}:{}: Unknown struct type '{}'",
                        span.file, span.line, span.col, struct_name
                    )
                })?;
                // Check field count matches
                if fields.len() != info.fields.len() {
                    return Err(format!(
                        "{}:{}:{}: Struct '{}' has {} fields but literal provides {}",
                        span.file,
                        span.line,
                        span.col,
                        struct_name,
                        info.fields.len(),
                        fields.len()
                    ));
                }
                // Check each field exists and validate types
                for (field_name, field_expr) in fields {
                    let decl_field = info
                        .fields
                        .iter()
                        .find(|f| f.name == *field_name)
                        .ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Struct '{}' has no field '{}'",
                                span.file, span.line, span.col, struct_name, field_name
                            )
                        })?;
                    let init_type = self.infer_expr_type(field_expr, scope, fn_sigs)?;
                    if init_type != decl_field.typ {
                        let ok = is_numeric_type(&init_type)
                            && is_numeric_type(&decl_field.typ)
                            && literal_fits_target(field_expr, &decl_field.typ);
                        let ok = ok || {
                            // Array of numerics: check each element fits target element type.
                            if let (Some((init_elem, _)), Some((decl_elem, _))) =
                                (as_array(&init_type), as_array(&decl_field.typ))
                            {
                                if is_numeric_type(init_elem)
                                    && is_numeric_type(decl_elem)
                                    && init_elem != decl_elem
                                {
                                    if let Expr::ArrayLit(elems, _) = field_expr {
                                        elems.iter().all(|e| literal_fits_target(e, decl_elem))
                                    } else {
                                        false
                                    }
                                } else {
                                    init_elem == decl_elem
                                }
                            } else {
                                false
                            }
                        };
                        if !ok {
                            let sp = field_expr.span();
                            return Err(format!(
                                "{}:{}:{}: Type mismatch for field '{}': expected {:?}, got {:?}",
                                sp.file, sp.line, sp.col, field_name, decl_field.typ, init_type
                            ));
                        }
                    }
                }
                Ok(Type::Struct(struct_name.clone()))
            }
        }
    }

    // ── Uninitialized variable tracking ─────────────────────

    /// Push a new scope for uninitialized variable tracking.
    fn push_uninit_scope(&mut self) {
        self.uninit_scopes.push(HashMap::new());
    }

    /// Pop the current uninit scope; error if any vars remain uninitialized.
    fn pop_uninit_scope(&mut self) -> Result<(), String> {
        if let Some(scope) = self.uninit_scopes.pop() {
            for (name, span) in &scope {
                return Err(format!(
                    "{}:{}:{}: Variable '{}' must be initialized",
                    span.file, span.line, span.col, name
                ));
            }
        }
        Ok(())
    }

    /// Record `name` as declared-but-uninitialized in the current scope.
    fn add_uninit(&mut self, name: &str, span: &Span) {
        if let Some(scope) = self.uninit_scopes.last_mut() {
            scope.insert(name.to_string(), span.clone());
        }
    }

    /// Remove `name` from all uninit scopes (innermost first) —
    /// called when an assignment to `name` is found.
    fn mark_initialized(&mut self, name: &str) {
        for scope in self.uninit_scopes.iter_mut().rev() {
            scope.remove(name);
        }
    }
}

// ── Free functions (used from both Sema and standalone context) ──

/// Analyze a variable declaration, inferring types and checking
/// initializer compatibility.
///
/// This function is called both for global and local variable declarations.
fn analyze_var_decl_free(
    v: &mut VarDecl,
    scope: &mut Scope,
    fn_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    warnings: &mut Vec<String>,
    struct_defs: &HashMap<String, StructInfo>,
    enum_members: &HashMap<String, (Type, i128)>,
) -> Result<(), String> {
    let inferred_type = if let Some(ref init) = v.init {
        Some(infer_expr_type_free(
            init,
            scope,
            fn_sigs,
            struct_defs,
            enum_members,
        )?)
    } else {
        None
    };

    for name in &v.names {
        let typ = if let Some(ref t) = v.typ {
            t.clone()
        } else if let Some(ref t) = inferred_type {
            warnings.push(format!(
                "{}:{}:{}: Variable '{}' declared with ':=', inferred type {:?}",
                v.span.file, v.span.line, v.span.col, name, t
            ));
            t.clone()
        } else {
            return Err(format!(
                "{}:{}:{}: Variable '{}' has no type and no initializer",
                v.span.file, v.span.line, v.span.col, name
            ));
        };

        // Type-check the initializer against the declared type.
        if let Some(ref init) = v.init {
            let init_type = infer_expr_type_free(init, scope, fn_sigs, struct_defs, enum_members)?;
            // Resolve enum types to underlying type for comparison.
            let init_r = match &init_type {
                Type::Enum(_) => &Type::I32,
                _ => &init_type,
            };
            let typ_r = match &typ {
                Type::Enum(_) => &Type::I32,
                _ => &typ,
            };
            if init_r != typ_r {
                if is_numeric_type(init_r) && is_numeric_type(typ_r) {
                    // Numeric-to-numeric: check if the literal value fits.
                    if !literal_fits_target(init, &typ) {
                        return Err(format!(
                            "{}:{}:{}: Value does not fit in '{}' of type {:?}",
                            v.span.file, v.span.line, v.span.col, name, typ
                        ));
                    }
                } else if let (Some((init_elem, _)), Some((decl_elem, _))) =
                    (as_array(&init_type), as_array(&typ))
                {
                    // Array of numerics: check each element literal fits target element type.
                    if is_numeric_type(init_elem)
                        && is_numeric_type(decl_elem)
                        && init_elem != decl_elem
                    {
                        if let Expr::ArrayLit(elems, _) = init {
                            for elem in elems {
                                if !literal_fits_target(elem, decl_elem) {
                                    let sp = elem.span();
                                    return Err(format!(
                                        "{}:{}:{}: Value does not fit in array element type {:?}",
                                        sp.file, sp.line, sp.col, decl_elem
                                    ));
                                }
                            }
                        }
                    } else if init_elem != decl_elem && typ != Type::Void {
                        return Err(format!(
                            "{}:{}:{}: Type mismatch: '{}' is {:?}, initializer is {:?}",
                            v.span.file, v.span.line, v.span.col, name, typ, init_type
                        ));
                    }
                } else if is_string_type(init_r) && is_string_type(typ_r) {
                    // String literals / string-typed values can be assigned to any string type
                } else if typ != Type::Void {
                    return Err(format!(
                        "{}:{}:{}: Type mismatch: '{}' is {:?}, initializer is {:?}",
                        v.span.file, v.span.line, v.span.col, name, typ, init_type
                    ));
                }
            }
        }

        scope.declare(name, typ, &v.span)?;
    }
    Ok(())
}

/// Infer the type of an expression without a surrounding `Sema` context.
///
/// Duplicates the logic in `Sema::infer_expr_type` but is a standalone
/// function so it can be called from `analyze_var_decl_free` without
/// borrowing issues.
/// Check a method call on an array-typed target.
///
/// `infer_arg` infers the type of an argument expression; it is supplied
/// by the caller because array methods are validated from two different
/// inference contexts (`Sema::infer_expr_type` and the free-standing
/// `infer_expr_type_free`).
fn check_array_method<F>(
    method: &str,
    args: &[Expr],
    span: &Span,
    array_type: &Type,
    mut infer_arg: F,
) -> Result<Type, String>
where
    F: FnMut(&Expr) -> Result<Type, String>,
{
    match method {
        "size" => {
            if !args.is_empty() {
                return Err(format!(
                    "{}:{}:{}: Array method 'size' expects 0 arguments, got {}",
                    span.file,
                    span.line,
                    span.col,
                    args.len()
                ));
            }
            Ok(Type::I32)
        }
        "resize" => {
            // Only dynamically-sized arrays can be resized; fixed-size
            // arrays (`fixed` keyword) are backed by plain C arrays.
            if matches!(array_type, Type::TyFixedArray(..)) {
                return Err(format!(
                    "{}:{}:{}: Cannot resize fixed-size array",
                    span.file, span.line, span.col
                ));
            }
            if args.len() != 1 {
                return Err(format!(
                    "{}:{}:{}: Array method 'resize' expects 1 argument, got {}",
                    span.file,
                    span.line,
                    span.col,
                    args.len()
                ));
            }
            let arg_type = infer_arg(&args[0])?;
            if !is_integral_type(&arg_type) {
                return Err(format!(
                    "{}:{}:{}: Array method 'resize' expects an integer argument, got {}",
                    span.file, span.line, span.col, arg_type
                ));
            }
            // A negative literal can never be a valid length. Unary minus
            // is desugared to `0 - lit`, which extract_index_literal folds.
            if let Some(val) = extract_index_literal(&args[0]) {
                if val < 0 {
                    return Err(format!(
                        "{}:{}:{}: Array method 'resize' expects a non-negative size, got {}",
                        span.file, span.line, span.col, val
                    ));
                }
            }
            Ok(Type::Void)
        }
        _ => Err(format!(
            "{}:{}:{}: Array type has no method '{}'",
            span.file, span.line, span.col, method
        )),
    }
}

fn infer_expr_type_free(
    expr: &Expr,
    scope: &Scope,
    fn_sigs: &HashMap<String, (Vec<Type>, Vec<Type>)>,
    struct_defs: &HashMap<String, StructInfo>,
    enum_members: &HashMap<String, (Type, i128)>,
) -> Result<Type, String> {
    match expr {
        Expr::IntLit(..) => Ok(Type::I32),
        Expr::FloatLit(..) => Ok(Type::F64),
        Expr::StringLit(..) => Ok(Type::String),
        Expr::CharLit(..) => Ok(Type::U8),
        Expr::BoolLit(..) => Ok(Type::Bool),
        Expr::Ident(name, span) => scope
            .lookup(name)
            .or_else(|| enum_members.get(name).map(|(t, _)| t.clone()))
            .ok_or_else(|| {
                format!(
                    "{}:{}:{}: Undefined variable '{}'",
                    span.file, span.line, span.col, name
                )
            }),
        Expr::Call { name, args, span } => {
            for arg in args {
                infer_expr_type_free(arg, scope, fn_sigs, struct_defs, enum_members)?;
            }
            if name == "println" || name == "printf" {
                Ok(Type::Void)
            } else if let Some(typ) = Type::from_str(name) {
                Ok(typ)
            } else {
                fn_sigs
                    .get(name)
                    .map(|(_, returns)| {
                        if returns.len() == 1 {
                            returns[0].clone()
                        } else {
                            Type::Void
                        }
                    })
                    .ok_or_else(|| {
                        format!(
                            "{}:{}:{}: Undefined function '{}'",
                            span.file, span.line, span.col, name
                        )
                    })
            }
        }
        Expr::BinaryOp {
            left,
            right,
            op,
            span,
        } => {
            let lt = infer_expr_type_free(left, scope, fn_sigs, struct_defs, enum_members)?;
            let rt = infer_expr_type_free(right, scope, fn_sigs, struct_defs, enum_members)?;
            // Resolve enum types to their underlying type for comparison.
            let lt_r = match &lt {
                Type::Enum(_) => Type::I32,
                _ => lt.clone(),
            };
            let rt_r = match &rt {
                Type::Enum(_) => Type::I32,
                _ => rt.clone(),
            };
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    if lt_r == rt_r {
                        Ok(lt_r)
                    } else {
                        Err(format!(
                            "{}:{}:{}: Type mismatch in arithmetic",
                            span.file, span.line, span.col
                        ))
                    }
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                    Ok(Type::Bool)
                }
                BinOp::And | BinOp::Or => Ok(Type::Bool),
                BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                    if lt_r == rt_r {
                        Ok(lt_r)
                    } else {
                        Err(format!(
                            "{}:{}:{}: Type mismatch in bitwise op",
                            span.file, span.line, span.col
                        ))
                    }
                }
            }
        }
        Expr::Assign { left, .. } => {
            infer_expr_type_free(left, scope, fn_sigs, struct_defs, enum_members)
        }
        Expr::DeclAssign { right, .. } => {
            infer_expr_type_free(right, scope, fn_sigs, struct_defs, enum_members)
        }
        Expr::ArrayLit(elems, span) => {
            if elems.is_empty() {
                return Err(format!(
                    "{}:{}:{}: Empty array literals not supported yet",
                    span.file, span.line, span.col
                ));
            }
            let elem_type =
                infer_expr_type_free(&elems[0], scope, fn_sigs, struct_defs, enum_members)?;
            for e in elems[1..].iter() {
                let et = infer_expr_type_free(e, scope, fn_sigs, struct_defs, enum_members)?;
                if et != elem_type {
                    return Err(format!(
                        "{}:{}:{}: Array element type mismatch",
                        span.file, span.line, span.col
                    ));
                }
            }
            Ok(Type::TyArray(Box::new(elem_type), None))
        }
        Expr::Index {
            target,
            index,
            span,
        } => {
            let target_type =
                infer_expr_type_free(target, scope, fn_sigs, struct_defs, enum_members)?;
            let index_type =
                infer_expr_type_free(index, scope, fn_sigs, struct_defs, enum_members)?;
            if !is_numeric_type(&index_type) {
                return Err(format!(
                    "{}:{}:{}: Array index must be numeric",
                    span.file, span.line, span.col
                ));
            }
            match target_type {
                // Fixed-size arrays: check compile-time known index bounds.
                // Negative indices wrap from end (-1 → last element, -n → index 0)
                Type::TyFixedArray(elem, sz) => check_index_bounds(&elem, Some(sz), index, span),
                // Dynamically-sized arrays can be resized at runtime, so
                // the declared size is not a compile-time bound; illegal
                // accesses are caught at runtime instead.
                Type::TyArray(elem, _) => Ok(*elem),
                Type::String | Type::String16 | Type::String32 => Ok(Type::U32),
                _ => Err(format!(
                    "{}:{}:{}: Cannot index non-array type",
                    span.file, span.line, span.col
                )),
            }
        }
        Expr::PostIncrement { target, .. } | Expr::PostDecrement { target, .. } => {
            infer_expr_type_free(target, scope, fn_sigs, struct_defs, enum_members)
        }
        Expr::FieldAccess {
            target,
            field,
            span,
        } => {
            let target_type =
                infer_expr_type_free(target, scope, fn_sigs, struct_defs, enum_members)?;
            match target_type {
                Type::Struct(ref struct_name) => {
                    let info = struct_defs.get(struct_name).ok_or_else(|| {
                        format!(
                            "{}:{}:{}: Unknown struct type '{}'",
                            span.file, span.line, span.col, struct_name
                        )
                    })?;
                    info.fields
                        .iter()
                        .find(|f| f.name == *field)
                        .map(|f| f.typ.clone())
                        .ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Struct '{}' has no field '{}'",
                                span.file, span.line, span.col, struct_name, field
                            )
                        })
                }
                _ => Err(format!(
                    "{}:{}:{}: Cannot access field on non-struct type",
                    span.file, span.line, span.col
                )),
            }
        }
        Expr::MethodCall {
            target,
            method,
            args,
            span,
        } => {
            let target_type =
                infer_expr_type_free(target, scope, fn_sigs, struct_defs, enum_members)?;
            match target_type {
                Type::Struct(_) => {
                    for arg in args {
                        infer_expr_type_free(arg, scope, fn_sigs, struct_defs, enum_members)?;
                    }
                    Ok(Type::Void)
                }
                Type::TyArray(..) | Type::TyFixedArray(..) => {
                    check_array_method(method, args, span, &target_type, |e| {
                        infer_expr_type_free(e, scope, fn_sigs, struct_defs, enum_members)
                    })
                }
                _ => Err(format!(
                    "{}:{}:{}: Cannot call method on non-struct type",
                    span.file, span.line, span.col
                )),
            }
        }
        Expr::StructLit {
            struct_name,
            fields,
            span,
        } => {
            let info = struct_defs.get(struct_name).ok_or_else(|| {
                format!(
                    "{}:{}:{}: Unknown struct type '{}'",
                    span.file, span.line, span.col, struct_name
                )
            })?;
            if fields.len() != info.fields.len() {
                return Err(format!(
                    "{}:{}:{}: Struct '{}' has {} fields but literal provides {}",
                    span.file,
                    span.line,
                    span.col,
                    struct_name,
                    info.fields.len(),
                    fields.len()
                ));
            }
            for (field_name, field_expr) in fields {
                let decl_field = info
                    .fields
                    .iter()
                    .find(|f| f.name == *field_name)
                    .ok_or_else(|| {
                        format!(
                            "{}:{}:{}: Struct '{}' has no field '{}'",
                            span.file, span.line, span.col, struct_name, field_name
                        )
                    })?;
                let init_type =
                    infer_expr_type_free(field_expr, scope, fn_sigs, struct_defs, enum_members)?;
                if init_type != decl_field.typ {
                    let ok = is_numeric_type(&init_type)
                        && is_numeric_type(&decl_field.typ)
                        && literal_fits_target(field_expr, &decl_field.typ);
                    let ok = ok || {
                        if let (Some((init_elem, _)), Some((decl_elem, _))) =
                            (as_array(&init_type), as_array(&decl_field.typ))
                        {
                            if is_numeric_type(init_elem)
                                && is_numeric_type(decl_elem)
                                && init_elem != decl_elem
                            {
                                if let Expr::ArrayLit(elems, _) = field_expr {
                                    elems.iter().all(|e| literal_fits_target(e, decl_elem))
                                } else {
                                    false
                                }
                            } else {
                                init_elem == decl_elem
                            }
                        } else {
                            false
                        }
                    };
                    if !ok {
                        let sp = field_expr.span();
                        return Err(format!(
                            "{}:{}:{}: Type mismatch for field '{}': expected {:?}, got {:?}",
                            sp.file, sp.line, sp.col, field_name, decl_field.typ, init_type
                        ));
                    }
                }
            }
            Ok(Type::Struct(struct_name.clone()))
        }
    }
}

/// Evaluate an enum variant value expression.
///
/// Handles integer literals and negated integer literals (from `-1` desugaring).
fn eval_enum_value(expr: &Expr, span: &Span) -> Result<i128, String> {
    match expr {
        Expr::IntLit(val, _) => Ok(*val),
        Expr::BinaryOp {
            left,
            op: BinOp::Sub,
            right,
            ..
        } => {
            if let Expr::IntLit(0, _) = left.as_ref() {
                if let Expr::IntLit(rval, _) = right.as_ref() {
                    return rval.checked_neg().ok_or_else(|| {
                        format!(
                            "{}:{}:{}: Enum value overflow",
                            span.file, span.line, span.col
                        )
                    });
                }
            }
            Err(format!(
                "{}:{}:{}: Enum variant value must be an integer literal",
                span.file, span.line, span.col
            ))
        }
        _ => Err(format!(
            "{}:{}:{}: Enum variant value must be an integer literal",
            span.file, span.line, span.col
        )),
    }
}

/// Extract a literal integer from an index expression, handling the
/// parser's unary-minus desugaring (`-n` → `0 - n` → `-n`).
///
/// Returns `None` if the index is not a compile-time known literal.
fn extract_index_literal(index: &Expr) -> Option<i128> {
    match index {
        Expr::IntLit(val, _) => Some(*val),
        Expr::BinaryOp {
            left,
            op: BinOp::Sub,
            right,
            ..
        } => {
            if let Expr::IntLit(0, _) = left.as_ref() {
                if let Expr::IntLit(val, _) = right.as_ref() {
                    return Some(-val);
                }
            }
            None
        }
        _ => None,
    }
}

/// Decompose an array type into its element type and optional declared
/// size. Covers both dynamically-sized (`TyArray`) and fixed-size
/// (`TyFixedArray`) arrays.
fn as_array(t: &Type) -> Option<(&Type, Option<u64>)> {
    match t {
        Type::TyArray(elem, size) => Some((elem, *size)),
        Type::TyFixedArray(elem, n) => Some((elem, Some(*n))),
        _ => None,
    }
}

/// Check compile-time bounds for a literal index against a declared
/// array size. Negative indices wrap from the end (-1 → last element).
fn check_index_bounds(
    elem: &Type,
    size: Option<u64>,
    index: &Expr,
    span: &Span,
) -> Result<Type, String> {
    if let Some(sz) = size {
        if let Some(val) = extract_index_literal(index) {
            if val < 0 {
                let pos = (-val) as u64;
                if pos > sz {
                    return Err(format!(
                        "{}:{}:{}: Array index {} out of bounds for size {}",
                        span.file, span.line, span.col, val, sz
                    ));
                }
            } else if (val as u64) >= sz {
                return Err(format!(
                    "{}:{}:{}: Array index {} out of bounds for size {}",
                    span.file, span.line, span.col, val, sz
                ));
            }
        }
    }
    Ok(elem.clone())
}

/// Returns `true` if `t` is any numeric type (integer or float).
fn is_numeric_type(t: &Type) -> bool {
    matches!(
        t,
        Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::I128
            | Type::I256
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::U128
            | Type::U256
            | Type::F8
            | Type::F16
            | Type::F32
            | Type::F64
    ) || matches!(t, Type::Enum(_))
}

/// Returns `true` if `t` is an integral type (integer only, no floats).
fn is_integral_type(t: &Type) -> bool {
    matches!(
        t,
        Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::I128
            | Type::I256
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::U128
            | Type::U256
    )
}

/// Resolve an enum type to its underlying type; pass through other types unchanged.
fn resolve_enum_type<'a>(t: &'a Type, enum_defs: &'a HashMap<String, EnumInfo>) -> &'a Type {
    match t {
        Type::Enum(name) => enum_defs.get(name).map(|e| &e.underlying_type).unwrap_or(t),
        _ => t,
    }
}

/// Check if a literal (or simple negated literal) expression's value
/// can be assigned to `target` without overflow.
fn literal_fits_target(expr: &Expr, target: &Type) -> bool {
    // Resolve enum type to underlying type.
    let target = match target {
        Type::Enum(_) => &Type::I32,
        _ => target,
    };
    match expr {
        Expr::IntLit(val, _) => int_lit_fits(*val, target),
        Expr::FloatLit(val, _) => float_lit_fits(*val, target),
        Expr::BinaryOp {
            left,
            op: BinOp::Sub,
            right,
            ..
        } => {
            // Detect `0 - lit` (unary minus desugared by the parser).
            if let Expr::IntLit(0, _) = left.as_ref() {
                if let Expr::IntLit(rval, _) = right.as_ref() {
                    return int_lit_fits(-rval, target);
                }
            }
            true
        }
        _ => true,
    }
}

/// Check if an integer literal value fits in the target type.
fn int_lit_fits(val: i128, target: &Type) -> bool {
    match target {
        Type::I8 => val >= i8::MIN as i128 && val <= i8::MAX as i128,
        Type::I16 => val >= i16::MIN as i128 && val <= i16::MAX as i128,
        Type::I32 => val >= i32::MIN as i128 && val <= i32::MAX as i128,
        Type::I64 => val >= i64::MIN as i128 && val <= i64::MAX as i128,
        Type::I128 => true,
        Type::I256 => true,
        Type::U8 => val >= 0 && val <= u8::MAX as i128,
        Type::U16 => val >= 0 && val <= u16::MAX as i128,
        Type::U32 => val >= 0 && val <= u32::MAX as i128,
        Type::U64 => val >= 0 && val <= u64::MAX as i128,
        Type::U128 => val >= 0,
        Type::U256 => val >= 0,
        Type::F8 | Type::F16 | Type::F32 | Type::F64 => true,
        _ => false,
    }
}

/// Check if a float literal value fits in the target type.
fn float_lit_fits(_val: f64, target: &Type) -> bool {
    match target {
        Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::I128
        | Type::I256
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::U128
        | Type::U256 => {
            // Float literal does not implicitly narrow to integer.
            false
        }
        Type::F8 | Type::F16 | Type::F32 | Type::F64 => true,
        _ => false,
    }
}
