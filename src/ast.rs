/// Abstract Syntax Tree (AST) node definitions for the Nitid language.
///
/// This module defines every node type that the parser produces and
/// the semantic analyser and codegen consume.  The AST is the
/// "middle representation" of the transpilation pipeline.
///
/// # Pipeline
/// `source text → [lexer] → tokens → [parser] → AST → [sema] → AST → [codegen] → C source`
///
/// # Missing
/// Most nodes carry a `Span` for error reporting, but the parser
/// frequently fills it with dummy values (`Span::new("", 0, 0)`).
/// Real source-location tracking is only partially implemented.

use crate::types::Type;

/// Source location: file name, line number, and column.
///
/// Used throughout the AST for producing actionable error messages.
#[derive(Debug, Clone)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

impl Span {
    /// Create a new source-location span.
    pub fn new(file: &str, line: usize, col: usize) -> Self {
        Self { file: file.to_string(), line, col }
    }
}

/// The root node: one `Program` per input `.nt` file.
///
/// Fields
/// * `package` — declared package name (defaults to `"main"`).
/// * `imports` — `import` statements (parsed but **not yet resolved**).
/// * `decls` — top-level function and variable declarations.
/// * `has_dangling` — `true` when statements appear outside any `fn` block
///   (the transpiler wraps them in an implicit `main`).
#[derive(Debug, Clone)]
pub struct Program {
    pub package: String,
    pub imports: Vec<Import>,
    pub decls: Vec<Decl>,
    pub file: String,
    pub has_dangling: bool,
}

/// An `import` statement.
///
/// Parsed but **not yet resolved** — there is no module system that
/// actually loads the imported package.
///
/// Example: `import foo as bar;` → `Import { name: "foo", alias: Some("bar") }`
#[derive(Debug, Clone)]
pub struct Import {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

/// A top-level declaration.
#[derive(Debug, Clone)]
pub enum Decl {
    FnDecl(FnDecl),
    VarDecl(VarDecl),
    StructDecl(StructDecl),
    ImplBlock(ImplBlock),
    EnumDecl(EnumDecl),
}

impl Decl {
    /// Return the span of this declaration.
    pub fn span(&self) -> &Span {
        match self {
            Decl::FnDecl(f) => &f.span,
            Decl::VarDecl(v) => &v.span,
            Decl::StructDecl(s) => &s.span,
            Decl::ImplBlock(i) => &i.span,
            Decl::EnumDecl(e) => &e.span,
        }
    }
}

/// A struct type definition.
///
/// Example:
/// ```c++
/// struct Person {
///   name: string;
///   age: u8;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
    pub packed: bool,
    pub align: Option<u64>,
    pub span: Span,
}

/// A single field in a struct definition.
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub typ: Type,
    pub span: Span,
}

/// An `impl` block attaching methods to a struct type.
///
/// Example:
/// ```c++
/// impl Person {
///   fn greet() -> string { ... }
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub struct_name: String,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

/// An enum type definition.
///
/// Example:
/// ```c++
/// enum AccessMode {
///     NONE = 0,
///     READONLY,
///     WRITEONLY,
///     READWRITE
/// };
/// ```
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub typ: Option<Type>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// A single variant in an enum definition.
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<Expr>,
    pub span: Span,
}

/// A function declaration.
///
/// Fields
/// * `name` — function identifier.
/// * `params` — parameters (each param can share one type across several names).
/// * `returns` — return types (empty = void, multiple values are returned via
///   output pointer arguments in C).
/// * `body` — statement list.
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub returns: Vec<Type>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// A single function parameter, carrying one type and one or more names.
///
/// Nitid allows: `fn foo(int a, b)` where both `a` and `b` are `int`.
#[derive(Debug, Clone)]
pub struct Param {
    pub typ: Type,
    pub names: Vec<String>,
    pub span: Span,
}

/// A variable declaration (top-level or statement-level).
///
/// Fields
/// * `typ` — explicit type annotation (`None` when `:=` type inference is used).
/// * `names` — one or more declared variable names.
/// * `init` — optional initializer expression.
/// * `array_size` — for array declarations, the size expression (None = inferred from init).
/// * `is_fixed` — true for `fixed` keyword (fixed-size array).
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub typ: Option<Type>,
    pub names: Vec<String>,
    pub init: Option<Expr>,
    pub span: Span,
    pub array_size: Option<u64>,
    pub is_fixed: bool,
}

/// Every kind of statement in Nitid.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// Variable declaration (e.g. `int x = 5;` or `x := 5;`).
    VarDecl(VarDecl),
    /// Expression used as a statement (e.g. `foo();`).
    Expr(Expr),
    /// Return statement (may return multiple values).
    Return(Vec<Expr>, Span),
    /// A braced block of statements, introducing a new scope.
    Block(Vec<Stmt>),
    /// If / else if / else conditional.
    If {
        cond: Box<Expr>,
        then_block: Vec<Stmt>,
        else_block: Option<Vec<Stmt>>,
        span: Span,
    },
    /// While loop.
    While {
        cond: Box<Expr>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// C-style for loop: `for (init; cond; inc) { body }`.
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Box<Expr>>,
        inc: Option<Box<Expr>>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Range for without index: `for (item : iter) { body }`.
    ForIn {
        var: String,
        iter: Box<Expr>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Range for with index: `for (idx, item : iter) { body }`.
    ForInIndex {
        idx_var: String,
        item_var: String,
        iter: Box<Expr>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Break statement (only valid inside a loop).
    Break(Span),
    /// Continue statement (only valid inside a loop).
    Continue(Span),
}

/// Every kind of expression.
#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i128, Span),
    FloatLit(f64, Span),
    StringLit(String, Span),
    CharLit(u8, Span),
    BoolLit(bool, Span),
    /// Variable reference.
    Ident(String, Span),
    /// Function call.
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Binary operator application.
    BinaryOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
        span: Span,
    },
    /// Assignment (`=`).
    Assign {
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// Declaration-and-assignment (`:=`).
    /// Syntactic sugar for `var name = expr` with inferred type.
    DeclAssign {
        names: Vec<String>,
        right: Box<Expr>,
        span: Span,
    },
    /// Array literal: `{ expr, expr, ... }`.
    ArrayLit(Vec<Expr>, Span),
    /// Index expression: `target[index]`.
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Postfix increment (`i++`).
    PostIncrement {
        target: Box<Expr>,
        span: Span,
    },
    /// Postfix decrement (`i--`).
    PostDecrement {
        target: Box<Expr>,
        span: Span,
    },
    /// Field access on a struct: `obj.field`.
    FieldAccess {
        target: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Method call on a struct: `obj.method(args)`.
    MethodCall {
        target: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Struct literal: `Circle{ color: "red", radius: 1.5 }`.
    StructLit {
        struct_name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
}

/// Binary operators, ordered by precedence (lowest first in the parser).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    // Arithmetic
    Add, Sub, Mul, Div, Mod,
    // Comparison
    Eq, Ne, Lt, Gt, Le, Ge,
    // Logical
    And, Or,
    // Bitwise / shift
    BitAnd, BitOr, BitXor, Shl, Shr,
}
