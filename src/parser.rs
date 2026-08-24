/// Recursive-descent parser for the Nitid language.
///
/// Phase 2 of the transpilation pipeline.  Consumes a stream of
/// [`Token`]s from the lexer and produces an [`ast::Program`].
///
/// # Grammar (informal)
/// ```text
/// program       ::= package? import* (fn_decl | stmt)* EOF
/// package       ::= "package" ident ";"
/// import        ::= "import" ident ("as" ident)? ";"
/// fn_decl       ::= "fn" ident param-list ("->" type-list)? "{" stmt* "}"
/// param-list    ::= "(" (type ident ("," ident)* ("," param)*)* ")"
/// stmt          ::= var-decl | decl-assign | expr ";"
///                  | "return" expr-list? ";"
///                  | "if" expr "{" stmt* "}" ("else" (if | "{" stmt* "}"))?
///                  | "while" expr "{" stmt* "}"
///                  | "{" stmt* "}"
/// expr          ::= assignment
/// assignment    ::= or-expr ("=" assignment)?
/// or-expr       ::= and-expr ("||" and-expr)*
/// and-expr      ::= bit-or ("&&" bit-or)*
/// ... (standard C-like precedence, lowest to highest)
/// ```
///
/// # Operator precedence (lowest to highest)
/// 1. `=` (assignment)
/// 2. `||`
/// 3. `&&`
/// 4. `|` (bitwise or)
/// 5. `^` (bitwise xor)
/// 6. `&` (bitwise and)
/// 7. `==` `!=`
/// 8. `<` `>` `<=` `>=`
/// 9. `<<` `>>`
/// 10. `+` `-`
/// 11. `*` `/` `%`
/// 12. unary `-` `!` `~`
/// 13. primary (literals, identifiers, calls, parenthesised)
///
/// # Limitations
/// - Imports are parsed but the module resolution is **not implemented**.
/// - Many `Span` values are dummy `Span::new("", 0, 0)` — source-location
///   tracking for error messages is only partially wired through.
/// - `for` loops (C-style and range) are fully implemented.
use std::collections::HashSet;
use crate::ast::*;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::types::Type;

/// Convenience alias — every parse method returns either a value or an error string.
type ParseResult<T> = Result<T, String>;

/// Recursive-descent parser.
///
/// Maintains a position cursor over the token stream and provides
/// helper methods for look-ahead, consumption, and error reporting.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    file: String,
    enum_names: HashSet<String>,
}

impl Parser {
    // ── Construction ──────────────────────────────────────────

    /// Create a new parser that consumes the given token stream.
    pub fn new(tokens: Vec<Token>, file: &str) -> Self {
        Self { tokens, pos: 0, file: file.to_string(), enum_names: HashSet::new() }
    }

    /// Convenience: lex + parse in one call.
    pub fn parse(input: &str, file: &str) -> ParseResult<Program> {
        let mut lexer = Lexer::new(input, file);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens, file);
        let program = parser.parse_program(file)?;
        Ok(program)
    }

    // ── Cursor helpers ────────────────────────────────────────

    /// Peek at the current token without consuming it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Peek at the kind of the current token.
    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    /// Peek at the kind of the token `n` positions ahead (0 = current).
    fn peek_nth_kind(&self, n: usize) -> Option<&TokenKind> {
        self.tokens.get(self.pos + n).map(|t| &t.kind)
    }

    /// Consume and return the current token, advancing the cursor.
    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    /// Expect the next token to be of `kind`; return it or error.
    fn expect(&mut self, kind: &TokenKind) -> ParseResult<Token> {
        if self.peek_kind() == Some(kind) {
            Ok(self.advance().unwrap())
        } else {
            match self.peek() {
                Some(tok) => {
                    let found = format!("{:?}", tok.kind);
                    Err(format!("{}:{}:{}: Expected {:?}, found {}",
                        tok.span.file, tok.span.line, tok.span.col, kind, found))
                }
                None => Err(format!("{}: Expected {:?}, found EOF", self.file, kind)),
            }
        }
    }

    /// Check if the current token matches `kind` (without consuming).
    fn check(&mut self, kind: &TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    /// If the current token matches `kind`, consume it and return `true`.
    fn consume(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn span_from_token(&self, tok: &Token) -> Span {
        tok.span.clone()
    }

    // ── Top-level ─────────────────────────────────────────────

    /// Parse a complete program.
    ///
    /// ```text
    /// program ::= package? import* (fn_decl | stmt)*
    /// ```
    ///
    /// Statements that appear outside any `fn` are gathered as
    /// *dangling statements* and later wrapped in an implicit `main`
    /// function.
    fn parse_program(&mut self, file: &str) -> ParseResult<Program> {
        let mut package = "main".to_string();
        let mut imports = Vec::new();
        let mut decls = Vec::new();
        let mut dangling_stmts = Vec::new();

        // Optional package declaration.
        if self.check(&TokenKind::Package) {
            self.advance();
            let name = self.expect_ident()?;
            self.expect(&TokenKind::Semicolon)?;
            package = name;
        }

        // Import statements.
        while self.check(&TokenKind::Import) {
            imports.push(self.parse_import()?);
        }

        // Declarations or dangling statements.
        while self.peek().is_some() {
            if self.check(&TokenKind::Fn) {
                decls.push(Decl::FnDecl(self.parse_fn_decl()?));
            } else if self.check(&TokenKind::Packed) || self.check(&TokenKind::Align) || self.check(&TokenKind::Struct) {
                decls.push(Decl::StructDecl(self.parse_struct_decl()?));
            } else if self.check(&TokenKind::Impl) {
                decls.push(Decl::ImplBlock(self.parse_impl_block()?));
            } else if self.check(&TokenKind::Enum) {
                let enum_decl = self.parse_enum_decl()?;
                self.enum_names.insert(enum_decl.name.clone());
                decls.push(Decl::EnumDecl(enum_decl));
            } else {
                let stmt = self.parse_stmt()?;
                dangling_stmts.push(stmt);
            }
        }

        // Wrap dangling statements in an implicit `main` function.
        let has_dangling = !dangling_stmts.is_empty();
        if has_dangling {
            decls.push(Decl::FnDecl(FnDecl {
                name: "main".to_string(),
                params: vec![
                    Param {
                        typ: Type::I32,
                        names: vec!["argc".to_string()],
                        span: Span::new(file, 0, 0),
                    },
                    Param {
                        typ: Type::String,
                        names: vec!["argv".to_string()],
                        span: Span::new(file, 0, 0),
                    },
                ],
                returns: vec![Type::I32],
                body: dangling_stmts,
                span: Span::new(file, 0, 0),
            }));
        }

        Ok(Program { package, imports, decls, file: file.to_string(), has_dangling })
    }

    /// Parse an identifier token and return its string value.
    fn expect_ident(&mut self) -> ParseResult<String> {
        match self.peek_kind() {
            Some(TokenKind::Ident(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(_kind) => {
                let tok = self.peek().unwrap();
                Err(format!("{}:{}:{}: Expected identifier, found {:?}",
                    tok.span.file, tok.span.line, tok.span.col, tok.kind))
            }
            None => Err(format!("{}: Expected identifier, found EOF", self.file)),
        }
    }

    /// `import ident (as ident)? ;`
    fn parse_import(&mut self) -> ParseResult<Import> {
        let tok = self.expect(&TokenKind::Import)?;
        let span = tok.span.clone();
        let name = self.expect_ident()?;
        let alias = if self.consume(&TokenKind::As) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Import { name, alias, span })
    }

    // ── Functions ─────────────────────────────────────────────

    /// `fn ident param-list ("->" type-list)? "{" stmt* "}"`
    fn parse_fn_decl(&mut self) -> ParseResult<FnDecl> {
        let fn_tok = self.expect(&TokenKind::Fn)?;
        let name = self.expect_ident()?;
        let span = fn_tok.span.clone();

        // Parameters: either `(...)` or just `{` (zero-param shorthand).
        let params = if self.check(&TokenKind::LParen) {
            self.parse_params()?
        } else {
            Vec::new()
        };

        // Return types.
        let returns = if self.consume(&TokenKind::Arrow) {
            if self.check(&TokenKind::LParen) {
                // Multiple return types: `-> (type, type, ...)`
                self.advance();
                let mut types = Vec::new();
                loop {
                    types.push(self.parse_type()?);
                    if !self.consume(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                types
            } else {
                vec![self.parse_type()?]
            }
        } else {
            vec![Type::Void]
        };

        // Body.
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_stmts_until(&TokenKind::RBrace)?;

        Ok(FnDecl { name, params, returns, body, span })
    }

    /// `"(" (type ident ("," ident)* ("," param)*)* ")"`
    fn parse_params(&mut self) -> ParseResult<Vec<Param>> {
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            self.advance();
            return Ok(params);
        }

        loop {
            let typ_tok = self.peek().cloned();
            let typ = self.parse_type()?;
            let span = typ_tok.map(|t| t.span).unwrap_or_else(|| Span::new(&self.file, 0, 0));
            let mut names = vec![self.expect_ident()?];
            let mut transition = false;
            // Consume comma-separated names sharing the same type.
            while self.consume(&TokenKind::Comma) {
                // If the next token is a type name, this comma starts a new parameter.
                if self.peek_kind().map(|k| is_type_kind(k, &self.enum_names)).unwrap_or(false) {
                    transition = true;
                    break;
                }
                names.push(self.expect_ident()?);
            }
            params.push(Param { typ, names, span });

            if transition {
                continue;
            }
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.consume(&TokenKind::Comma);
        }

        self.expect(&TokenKind::RParen)?;
        Ok(params)
    }

    // ── Structs & impls ──────────────────────────────────────

    /// `("packed"? "align" "(" int ")"?)? "struct" ident "{" (ident ":" type ";")* "}"`
    fn parse_struct_decl(&mut self) -> ParseResult<StructDecl> {
        let mut packed = false;
        let mut align = None;

        loop {
            if self.consume(&TokenKind::Packed) {
                packed = true;
            } else if self.consume(&TokenKind::Align) {
                self.expect(&TokenKind::LParen)?;
                let n_tok = self.advance().ok_or_else(|| "Expected integer literal in align".to_string())?;
                let n = match &n_tok.kind {
                    TokenKind::IntLit(s) => s.parse::<u64>().map_err(|_| "Invalid align value".to_string())?,
                    _ => return Err(format!("Expected integer literal in align, found {:?}", n_tok.kind)),
                };
                align = Some(n);
                self.expect(&TokenKind::RParen)?;
            } else {
                break;
            }
        }

        let tok = self.expect(&TokenKind::Struct)?;
        let span = tok.span.clone();
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.peek().is_some() {
            let field_name = self.expect_ident()?;
            let field_span = self.peek().map(|t| t.span.clone()).unwrap_or_else(|| span.clone());
            self.expect(&TokenKind::Colon)?;
            let field_type = self.parse_type()?;
            self.expect(&TokenKind::Semicolon)?;
            fields.push(StructField { name: field_name, typ: field_type, span: field_span });
        }
        self.expect(&TokenKind::RBrace)?;
        // Optional trailing semicolon (as shown in the spec).
        self.consume(&TokenKind::Semicolon);
        Ok(StructDecl { name, fields, packed, align, span })
    }

    /// `"impl" ident "{" fn_decl* "}"`
    fn parse_impl_block(&mut self) -> ParseResult<ImplBlock> {
        let tok = self.advance().ok_or_else(|| "Expected 'impl'".to_string())?;
        let span = tok.span.clone();
        let struct_name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.peek().is_some() {
            methods.push(self.parse_fn_decl()?);
        }
        self.expect(&TokenKind::RBrace)?;
        // Optional trailing semicolon (as shown in the spec).
        self.consume(&TokenKind::Semicolon);
        Ok(ImplBlock { struct_name, methods, span })
    }

    /// `enum ident (":" type)? "{" ident ("=" expr)? ("," ident ("=" expr)?)* ","? "}" ";"`
    fn parse_enum_decl(&mut self) -> ParseResult<EnumDecl> {
        let tok = self.expect(&TokenKind::Enum)?;
        let span = tok.span.clone();
        let name = self.expect_ident()?;

        // Optional explicit type.
        let underlying = if self.consume(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&TokenKind::LBrace)?;
        let mut variants = Vec::new();
        loop {
            // Allow trailing comma before closing brace.
            if self.check(&TokenKind::RBrace) {
                break;
            }
            let vname = self.expect_ident()?;
            let vspan = self.peek().map(|t| t.span.clone()).unwrap_or_else(|| span.clone());
            let value = if self.consume(&TokenKind::Eq) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            variants.push(EnumVariant { name: vname, value, span: vspan });
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace)?;
        // Optional trailing semicolon (as shown in the spec).
        self.consume(&TokenKind::Semicolon);
        Ok(EnumDecl { name, typ: underlying, variants, span })
    }

    // ── Types ─────────────────────────────────────────────────

    fn parse_type(&mut self) -> ParseResult<Type> {
        let span = self.peek().map(|t| t.span.clone());
        match self.peek_kind() {
            // `[size type]` array syntax (alternate, no semicolon)
            // or `[type; size]` array syntax (Rust-style, with semicolon)
            Some(TokenKind::LBracket) => {
                self.advance(); // consume [
                // Peek ahead: if next token is IntLit → `[size type]` syntax
                if matches!(self.peek_kind(), Some(TokenKind::IntLit(_))) {
                    let size_expr = self.parse_expr()?;
                    let size_val = match &size_expr {
                        Expr::IntLit(v, _) => *v,
                        _ => {
                            let sp2 = size_expr.span();
                            return Err(format!("{}:{}:{}: Array size must be an integer literal",
                                sp2.file, sp2.line, sp2.col));
                        }
                    };
                    let elem_type = self.parse_type()?;
                    self.expect(&TokenKind::RBracket)?;
                    Ok(Type::TyArray(Box::new(elem_type), Some(size_val as u64)))
                } else {
                    // `[type; size]` syntax
                    let elem_type = self.parse_type()?;
                    self.expect(&TokenKind::Semicolon)?;
                    let size_expr = self.parse_expr()?;
                    let size_val = match &size_expr {
                        Expr::IntLit(v, _) => *v,
                        _ => {
                            let sp2 = size_expr.span();
                            return Err(format!("{}:{}:{}: Array size must be an integer literal",
                                sp2.file, sp2.line, sp2.col));
                        }
                    };
                    self.expect(&TokenKind::RBracket)?;
                    Ok(Type::TyArray(Box::new(elem_type), Some(size_val as u64)))
                }
            }
            Some(TokenKind::Type(s)) | Some(TokenKind::Ident(s)) => {
                let s = s.clone();
                self.advance();
                let sp = span.unwrap_or_else(|| Span::new(&self.file, 0, 0));
                let base = if let Some(t) = Type::from_str(&s) {
                    t
                } else if self.enum_names.contains(&s) {
                    Type::Enum(s)
                } else {
                    return Err(format!("{}:{}:{}: Unknown type '{}'",
                        sp.file, sp.line, sp.col, s));
                };
                // Array type: `Type [ Expr? ]`
                if self.consume(&TokenKind::LBracket) {
                    if !self.check(&TokenKind::RBracket) {
                        let size_expr = self.parse_expr()?;
                        let size_val = match &size_expr {
                            Expr::IntLit(v, _) => *v,
                            _ => {
                                let sp2 = size_expr.span();
                                return Err(format!("{}:{}:{}: Array size must be an integer literal",
                                    sp2.file, sp2.line, sp2.col));
                            }
                        };
                        self.expect(&TokenKind::RBracket)?;
                        Ok(Type::TyArray(Box::new(base), Some(size_val as u64)))
                    } else {
                        self.advance(); // consume ]
                        Ok(Type::TyArray(Box::new(base), None))
                    }
                } else {
                    Ok(base)
                }
            }
            Some(_kind) => {
                let sp = match &self.peek() {
                    Some(t) => t.span.clone(),
                    None => Span::new(&self.file, 0, 0),
                };
                Err(format!("{}:{}:{}: Expected type, found {:?}",
                    sp.file, sp.line, sp.col, self.peek().unwrap().kind))
            }
            None => Err(format!("{}: Expected type, found EOF", self.file)),
        }
    }

    // ── Statements ────────────────────────────────────────────

    /// Parse statements until `end` is encountered (consuming it).
    fn parse_stmts_until(&mut self, end: &TokenKind) -> ParseResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        while !self.check(end) && self.peek().is_some() {
            stmts.push(self.parse_stmt()?);
        }
        if self.check(end) {
            self.advance();
        }
        Ok(stmts)
    }

    /// Parse a single statement.
    ///
    /// Dispatches based on the leading token.  Requires up to 3 tokens
    /// of look-ahead to distinguish:
    /// - `type name ...` (typed declaration)
    /// - `name := expr` (declaration-assignment)
    /// - `name = expr` (assignment)
    /// - `name(...)` (expression / call)
    fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        if self.check(&TokenKind::Return) {
            return self.parse_return_stmt();
        }
        if self.check(&TokenKind::If) {
            return self.parse_if_stmt();
        }
        if self.check(&TokenKind::While) {
            return self.parse_while_stmt();
        }
        if self.check(&TokenKind::For) {
            return self.parse_for_stmt();
        }
        if self.check(&TokenKind::Break) {
            let tok = self.advance().unwrap();
            let span = tok.span.clone();
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Stmt::Break(span));
        }
        if self.check(&TokenKind::Continue) {
            let tok = self.advance().unwrap();
            let span = tok.span.clone();
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Stmt::Continue(span));
        }
        if self.check(&TokenKind::LBrace) {
            self.advance();
            let stmts = self.parse_stmts_until(&TokenKind::RBrace)?;
            return Ok(Stmt::Block(stmts));
        }

        // `fixed` keyword → array variable declaration
        if self.check(&TokenKind::Fixed) {
            return self.parse_var_decl_stmt();
        }

        // Look-ahead to distinguish declarations from expression statements.
        if matches!(self.peek_kind(), Some(TokenKind::Type(_)))
            || matches!(self.peek_kind(), Some(TokenKind::Ident(_)))
        {
            let saved = self.pos;
            let _tok1 = self.peek().unwrap().kind.clone();
            self.advance();
            let tok2 = self.peek().unwrap().kind.clone();

            if matches!(&tok2, TokenKind::Ident(_)) {
                // `type name ...` or `ident name ...` — could be a declaration.
                let _name = match &tok2 {
                    TokenKind::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                self.advance();
                let tok3 = self.peek_kind().cloned();
                self.pos = saved;

                match tok3 {
                    Some(TokenKind::ColonEq) => return self.parse_decl_assign_stmt(),
                    Some(TokenKind::Eq) => return self.parse_var_decl_stmt(),
                    Some(TokenKind::Semicolon) => return self.parse_var_decl_stmt(),
                    Some(TokenKind::Comma) => return self.parse_var_decl_stmt(),
                    _ => {
                        // Fall through to expression statement.
                        let expr = self.parse_expr()?;
                        self.expect(&TokenKind::Semicolon)?;
                        return Ok(Stmt::Expr(expr));
                    }
                }
            } else {
                self.pos = saved;
                // Check for `ident := expr` (no type annotation).
                if matches!(self.peek_kind(), Some(TokenKind::Ident(_))) {
                    let saved2 = self.pos;
                    let _first = self.expect_ident()?;
                    if self.check(&TokenKind::ColonEq) {
                        self.pos = saved2;
                        return self.parse_decl_assign_stmt();
                    }
                    if self.check(&TokenKind::Comma) {
                        // Could be `a, b := expr`
                        let _saved3 = self.pos;
                        self.advance();
                        if matches!(self.peek_kind(), Some(TokenKind::Ident(_))) {
                            self.advance();
                            if self.check(&TokenKind::ColonEq) {
                                self.pos = saved2;
                                return self.parse_decl_assign_stmt();
                            }
                        }
                        self.pos = saved2;
                    } else {
                        self.pos = saved2;
                    }
                }

                let expr = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                return Ok(Stmt::Expr(expr));
            }
        }

        // Fallback: expression statement or decl-assign.
        if matches!(self.peek_kind(), Some(TokenKind::Ident(_))) {
            let saved = self.pos;
            let _ident = self.expect_ident()?;
            if self.check(&TokenKind::ColonEq) {
                self.pos = saved;
                return self.parse_decl_assign_stmt();
            }
            self.pos = saved;
        }

        let expr = self.parse_expr()?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Expr(expr))
    }

    /// `fixed? type name (, name)* ("[" expr "]")? (= expr)? ;`
    fn parse_var_decl_stmt(&mut self) -> ParseResult<Stmt> {
        let is_fixed = self.consume(&TokenKind::Fixed);
        let type_tok = self.peek().cloned();
        let base_typ = self.parse_type()?;
        let span = type_tok.as_ref().map(|t| t.span.clone()).unwrap_or_else(|| Span::new(&self.file, 0, 0));
        let mut names = vec![self.expect_ident()?];
        // Parse optional array size: `name [ expr ]`
        let mut array_size = None;
        if names.len() == 1 && self.consume(&TokenKind::LBracket) {
            let size_expr = self.parse_expr()?;
            match &size_expr {
                Expr::IntLit(v, _) => array_size = Some(*v as u64),
                _ => {
                    let sp = size_expr.span();
                    return Err(format!("{}:{}:{}: Array size must be an integer literal",
                        sp.file, sp.line, sp.col));
                }
            }
            self.expect(&TokenKind::RBracket)?;
        }
        while self.consume(&TokenKind::Comma) {
            names.push(self.expect_ident()?);
        }
        let init = if self.consume(&TokenKind::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        // Wrap element type when this is an array declaration:
        // `fixed` + explicit size → fixed-size C array; explicit size
        // without `fixed` → dynamically-sized (heap) array.
        let typ = if let Some(sz) = array_size {
            if is_fixed {
                Type::TyFixedArray(Box::new(base_typ), sz)
            } else {
                Type::TyArray(Box::new(base_typ), Some(sz))
            }
        } else if is_fixed {
            Type::TyArray(Box::new(base_typ), None)
        } else {
            base_typ
        };
        Ok(Stmt::VarDecl(VarDecl { typ: Some(typ), names, init, span, array_size, is_fixed }))
    }

    /// `fixed? type name (, name)* ":=" expr ";"`
    ///
    /// Declaration-assignment with (optionally) explicit type.
    fn parse_decl_assign_stmt(&mut self) -> ParseResult<Stmt> {
        let is_fixed = self.consume(&TokenKind::Fixed);
        let mut names = Vec::new();

        // If the first token is a type name, parse explicit type.
        if is_fixed || self.peek_kind().map(|k| is_type_kind(k, &self.enum_names)).unwrap_or(false) {
            let type_tok = self.peek().cloned();
            let typ = self.parse_type()?;
            let span = type_tok.map(|t| t.span).unwrap_or_else(|| Span::new(&self.file, 0, 0));
            names.push(self.expect_ident()?);
            while self.consume(&TokenKind::Comma) {
                names.push(self.expect_ident()?);
            }
            self.expect(&TokenKind::ColonEq)?;
            let expr = self.parse_expr()?;
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Stmt::VarDecl(VarDecl { typ: Some(typ), names, init: Some(expr), span, array_size: None, is_fixed }));
        }

        // No explicit type — infer from initializer.
        let first_ident = self.peek().cloned();
        names.push(self.expect_ident()?);
        let span = first_ident.map(|t| t.span).unwrap_or_else(|| Span::new(&self.file, 0, 0));
        while self.consume(&TokenKind::Comma) {
            names.push(self.expect_ident()?);
        }
        self.expect(&TokenKind::ColonEq)?;
        // After `:=`, if we see a type-like start, parse as type annotation
        // (e.g. `a := int[35]` or `a := fixed int[5]`).
        // Also handle alternate array syntax: `a := [35 int]` or `a := fixed [35 int]`.
        let is_bracket_array = matches!(self.peek_kind(), Some(TokenKind::LBracket))
            && matches!(self.peek_nth_kind(1), Some(TokenKind::IntLit(_)))
            && self.peek_nth_kind(2).map(|k| is_type_kind(k, &self.enum_names)).unwrap_or(false);
        let is_fixed_annot = self.consume(&TokenKind::Fixed);
        if is_fixed_annot || self.peek_kind().map(|k| is_type_kind(k, &self.enum_names)).unwrap_or(false) || is_bracket_array {
            let typ = self.parse_type()?;
            // Fold the `fixed` annotation into sized array types:
            // `fixed int[5]` → C array; plain `int[5]` stays dynamic.
            let typ = match (is_fixed_annot, typ) {
                (true, Type::TyArray(elem, Some(sz))) => Type::TyFixedArray(elem, sz),
                (_, t) => t,
            };
            // Optional initializer (e.g. `a := [3 int]{1, 2, 3}`)
            let init = if self.check(&TokenKind::LBrace) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Stmt::VarDecl(VarDecl { typ: Some(typ), names, init, span, array_size: None, is_fixed: is_fixed_annot }));
        }
        let expr = self.parse_expr()?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::VarDecl(VarDecl { typ: None, names, init: Some(expr), span, array_size: None, is_fixed: false }))
    }

    /// `"return" expr-list? ";"`
    fn parse_return_stmt(&mut self) -> ParseResult<Stmt> {
        let ret_tok = self.advance().unwrap(); // return
        let span = ret_tok.span.clone();
        if self.check(&TokenKind::Semicolon) {
            self.advance();
            return Ok(Stmt::Return(Vec::new(), span));
        }
        let mut values = vec![self.parse_expr()?];
        while self.consume(&TokenKind::Comma) {
            values.push(self.parse_expr()?);
        }
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Return(values, span))
    }

    /// `"if" expr "{" stmt* "}" ("else" (if | "{" stmt* "}"))?`
    fn parse_if_stmt(&mut self) -> ParseResult<Stmt> {
        let if_tok = self.advance().unwrap(); // if
        let span = if_tok.span.clone();
        let cond = Box::new(self.parse_expr()?);
        self.expect(&TokenKind::LBrace)?;
        let then_block = self.parse_stmts_until(&TokenKind::RBrace)?;
        let else_block = if self.consume(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                // `else if` — recursively parse as a nested If statement.
                let elif = self.parse_if_stmt()?;
                match elif {
                    Stmt::If { cond, then_block, else_block, .. } => {
                        Some(vec![Stmt::If { cond, then_block, else_block, span: span.clone() }])
                    }
                    _ => unreachable!(),
                }
            } else {
                self.expect(&TokenKind::LBrace)?;
                Some(self.parse_stmts_until(&TokenKind::RBrace)?)
            }
        } else {
            None
        };
        Ok(Stmt::If { cond, then_block, else_block, span })
    }

    /// `"while" expr "{" stmt* "}"`
    fn parse_while_stmt(&mut self) -> ParseResult<Stmt> {
        let while_tok = self.advance().unwrap(); // while
        let span = while_tok.span.clone();
        let cond = Box::new(self.parse_expr()?);
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_stmts_until(&TokenKind::RBrace)?;
        Ok(Stmt::While { cond, body, span })
    }

    // ── For loops ────────────────────────────────────────────

    /// `"for" "(" for-body ")" "{" stmt* "}"`
    ///
    /// Three forms:
    ///   C-style:  `for (init? ; cond? ; inc?) { body }`
    ///   Range:    `for (item : iter) { body }`
    ///   RangeIdx: `for (idx, item : iter) { body }`
    ///
    /// Disambiguated by scanning forward for `;` (C-style) vs `:` (range).
    fn parse_for_stmt(&mut self) -> ParseResult<Stmt> {
        let for_tok = self.advance().unwrap();
        let span = for_tok.span.clone();
        self.expect(&TokenKind::LParen)?;

        // Scan forward (no consumption) to find `;` or `:` at depth 0.
        let is_c_style = {
            let mut depth: i32 = 0;
            let mut found = false;
            for i in self.pos..self.tokens.len() {
                match &self.tokens[i].kind {
                    TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => depth += 1,
                    TokenKind::RParen if depth == 0 => break,
                    TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => depth -= 1,
                    TokenKind::Semicolon if depth == 0 => { found = true; break; }
                    TokenKind::Colon if depth == 0 => break,
                    _ => {}
                }
            }
            found
        };

        if is_c_style {
            self.parse_for_cstyle(span)
        } else {
            self.parse_for_range(span)
        }
    }

    /// Parse C-style for: `(init? ; cond? ; inc?)`
    fn parse_for_cstyle(&mut self, span: Span) -> ParseResult<Stmt> {
        // ── init ────────────────────────────────────────────────
        let init = if self.check(&TokenKind::Semicolon) {
            self.advance();
            None
        } else if matches!(self.peek_kind(), Some(TokenKind::Type(_))) {
            // `Type Ident ... ;` → variable declaration init
            let type_span = self.peek().unwrap().span.clone();
            let typ = self.parse_type()?;
            let mut names = vec![self.expect_ident()?];
            while self.consume(&TokenKind::Comma) {
                names.push(self.expect_ident()?);
            }
            let init_expr = if self.consume(&TokenKind::Eq) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon)?;
            Some(Box::new(Stmt::VarDecl(VarDecl {
                typ: Some(typ), names, init: init_expr, span: type_span,
                array_size: None, is_fixed: false,
            })))
        } else if matches!(self.peek_kind(), Some(TokenKind::Ident(_))) {
            let saved = self.pos;
            let _first = self.expect_ident()?;
            if self.check(&TokenKind::ColonEq) {
                // `ident := expr ;` → decl-assign init
                self.pos = saved;
                let mut names = vec![self.expect_ident()?];
                while self.consume(&TokenKind::Comma) {
                    names.push(self.expect_ident()?);
                }
                self.expect(&TokenKind::ColonEq)?;
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Some(Box::new(Stmt::VarDecl(VarDecl {
                    typ: None, names, init: Some(expr),
                    span: span.clone(),
                    array_size: None, is_fixed: false,
                })))
            } else {
                // `expr ;` → expression init
                self.pos = saved;
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Some(Box::new(Stmt::Expr(expr)))
            }
        } else {
            None
        };

        // ── cond ────────────────────────────────────────────────
        let cond = if self.check(&TokenKind::Semicolon) || self.check(&TokenKind::RParen) {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };
        self.expect(&TokenKind::Semicolon)?;

        // ── inc ─────────────────────────────────────────────────
        let inc = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };
        self.expect(&TokenKind::RParen)?;

        // ── body ────────────────────────────────────────────────
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_stmts_until(&TokenKind::RBrace)?;

        Ok(Stmt::For { init, cond, inc, body, span })
    }

    /// Parse range for: `(ident : expr)` or `(ident , ident : expr)`
    fn parse_for_range(&mut self, span: Span) -> ParseResult<Stmt> {
        let first = self.expect_ident()?;

        if self.consume(&TokenKind::Comma) {
            // `ident , ident : expr` → range with index
            let second = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let iter = self.parse_expr()?;
            self.expect(&TokenKind::RParen)?;
            self.expect(&TokenKind::LBrace)?;
            let body = self.parse_stmts_until(&TokenKind::RBrace)?;
            Ok(Stmt::ForInIndex {
                idx_var: first, item_var: second,
                iter: Box::new(iter), body, span,
            })
        } else {
            // `ident : expr` → range without index
            self.expect(&TokenKind::Colon)?;
            let iter = self.parse_expr()?;
            self.expect(&TokenKind::RParen)?;
            self.expect(&TokenKind::LBrace)?;
            let body = self.parse_stmts_until(&TokenKind::RBrace)?;
            Ok(Stmt::ForIn {
                var: first, iter: Box::new(iter), body, span,
            })
        }
    }

    // ── Expressions (precedence climbing) ─────────────────────

    /// Top-level expression — starts at assignment level.
    fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_assignment()
    }

    /// `assignment ::= or-expr ("=" assignment)?`
    fn parse_assignment(&mut self) -> ParseResult<Expr> {
        let left = self.parse_or()?;
        if self.consume(&TokenKind::Eq) {
            let right = self.parse_expr()?;
            let span = left.span();
            return Ok(Expr::Assign {
                left: Box::new(left),
                right: Box::new(right),
                span,
            });
        }
        Ok(left)
    }

    /// `||` (lowest-precedence binary operator).
    fn parse_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and()?;
        while self.consume(&TokenKind::OrOr) {
            let right = self.parse_and()?;
            let span = left.span();
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::Or,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// `&&`
    fn parse_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_bit_or()?;
        while self.consume(&TokenKind::AndAnd) {
            let right = self.parse_bit_or()?;
            let span = left.span();
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::And,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// `|` (bitwise or)
    fn parse_bit_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_bit_xor()?;
        while self.consume(&TokenKind::Pipe) {
            let right = self.parse_bit_xor()?;
            let span = left.span();
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::BitOr,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// `^` (bitwise xor)
    fn parse_bit_xor(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_bit_and()?;
        while self.consume(&TokenKind::Caret) {
            let right = self.parse_bit_and()?;
            let span = left.span();
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::BitXor,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// `&` (bitwise and)
    fn parse_bit_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality()?;
        while self.consume(&TokenKind::Ampersand) {
            let right = self.parse_equality()?;
            let span = left.span();
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::BitAnd,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    /// `==` `!=`
    fn parse_equality(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            if self.consume(&TokenKind::EqEq) {
                let right = self.parse_comparison()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Eq,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Ne) {
                let right = self.parse_comparison()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Ne,
                    right: Box::new(right),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// `<` `>` `<=` `>=`
    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_shift()?;
        loop {
            if self.consume(&TokenKind::Lt) {
                let right = self.parse_shift()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Lt,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Gt) {
                let right = self.parse_shift()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Gt,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Le) {
                let right = self.parse_shift()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Le,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Ge) {
                let right = self.parse_shift()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Ge,
                    right: Box::new(right),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// `<<` `>>`
    fn parse_shift(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_term()?;
        loop {
            if self.consume(&TokenKind::Shl) {
                let right = self.parse_term()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Shl,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Shr) {
                let right = self.parse_term()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Shr,
                    right: Box::new(right),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// `+` `-`
    fn parse_term(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            if self.consume(&TokenKind::Plus) {
                let right = self.parse_factor()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Add,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Minus) {
                let right = self.parse_factor()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Sub,
                    right: Box::new(right),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// `*` `/` `%`
    fn parse_factor(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            if self.consume(&TokenKind::Star) {
                let right = self.parse_unary()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Mul,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Slash) {
                let right = self.parse_unary()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Div,
                    right: Box::new(right),
                    span,
                };
            } else if self.consume(&TokenKind::Percent) {
                let right = self.parse_unary()?;
                let span = left.span();
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinOp::Mod,
                    right: Box::new(right),
                    span,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// Unary operators: `-` (negate), `!` (logical not), `~` (bitwise not).
    ///
    /// Note: the current implementation desugars unary operators into
    /// binary operations using synthetic literals (e.g. `-x` → `0 - x`).
    /// This works but loses the distinction in the AST if a later pass
    /// needs it.
    fn parse_unary(&mut self) -> ParseResult<Expr> {
        if self.consume(&TokenKind::Minus) {
            let expr = self.parse_unary()?;
            let span = expr.span();
            Ok(Expr::BinaryOp {
                left: Box::new(Expr::IntLit(0, span.clone())),
                op: BinOp::Sub,
                right: Box::new(expr),
                span,
            })
        } else if self.consume(&TokenKind::Bang) {
            let expr = self.parse_unary()?;
            let span = expr.span();
            Ok(Expr::BinaryOp {
                left: Box::new(Expr::BoolLit(false, span.clone())),
                op: BinOp::Ne,
                right: Box::new(Expr::BoolLit(true, span.clone())),
                span,
            })
            // FIXME: proper unary not
        } else if self.consume(&TokenKind::Tilde) {
            let expr = self.parse_unary()?;
            let span = expr.span();
            Ok(Expr::BinaryOp {
                left: Box::new(Expr::IntLit(-1, span.clone())),
                op: BinOp::BitXor,
                right: Box::new(expr),
                span,
            })
        } else {
            self.parse_primary()
        }
    }

    /// Primary expressions: literals, identifiers, function calls,
    /// array literals, parenthesised expressions, and postfix
    /// index/function-call/++/-- operators.
    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let tok = self.advance().ok_or_else(|| "Unexpected EOF".to_string())?;
        let span = tok.span.clone();

        let mut expr = match tok.kind {
            TokenKind::IntLit(s) => {
                let val = if s.starts_with("0x") || s.starts_with("0X") {
                    i128::from_str_radix(&s[2..], 16)
                        .map_err(|e| format!("Invalid hex literal '{}': {}", s, e))?
                } else {
                    s.parse::<i128>()
                        .map_err(|e| format!("Invalid int literal '{}': {}", s, e))?
                };
                Expr::IntLit(val, span)
            }
            TokenKind::FloatLit(s) => {
                let val = s.parse::<f64>()
                    .map_err(|e| format!("Invalid float literal '{}': {}", s, e))?;
                Expr::FloatLit(val, span)
            }
            TokenKind::StringLit(s) => Expr::StringLit(s, span),
            TokenKind::CharLit(s) => {
                let c = s.chars().next().unwrap_or('\0') as u8;
                Expr::CharLit(c, span)
            }
            TokenKind::True => Expr::BoolLit(true, span),
            TokenKind::False => Expr::BoolLit(false, span),
            TokenKind::Self_ => {
                Expr::Ident("self".to_string(), span)
            }
            TokenKind::Type(name) => {
                if self.check(&TokenKind::LParen) {
                    // Type conversion / constructor call: `i64(expr)`
                    self.advance(); // (
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.consume(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    Expr::Call { name, args, span }
                } else {
                    // Type name used as standalone expression
                    Expr::Ident(name, span)
                }
            }
            TokenKind::Ident(name) => {
                if self.check(&TokenKind::LBrace) {
                    // Struct literal: `Name{ field: expr, ... }`
                    self.advance(); // {
                    let mut fields = Vec::new();
                    if !self.check(&TokenKind::RBrace) {
                        loop {
                            let field_name = self.expect_ident()?;
                            self.expect(&TokenKind::Colon)?;
                            let field_val = self.parse_expr()?;
                            fields.push((field_name, field_val));
                            if !self.consume(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    Expr::StructLit { struct_name: name, fields, span }
                } else if self.check(&TokenKind::LParen) {
                    // Function call: `ident(args...)`
                    self.advance(); // (
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.consume(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    Expr::Call { name, args, span }
                } else if self.consume(&TokenKind::PlusPlus) {
                    Expr::PostIncrement {
                        target: Box::new(Expr::Ident(name, span.clone())),
                        span,
                    }
                } else if self.consume(&TokenKind::MinusMinus) {
                    Expr::PostDecrement {
                        target: Box::new(Expr::Ident(name, span.clone())),
                        span,
                    }
                } else {
                    Expr::Ident(name, span)
                }
            }
            TokenKind::LParen => {
                let inner = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                inner
            }
            TokenKind::LBrace => {
                // Array literal: `{ expr, expr, ... }`
                let mut elems = Vec::new();
                if !self.check(&TokenKind::RBrace) {
                    loop {
                        elems.push(self.parse_expr()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBrace)?;
                Expr::ArrayLit(elems, span)
            }
            kind => {
                return Err(format!(
                    "{}:{}:{}: Unexpected token {:?}",
                    span.file, span.line, span.col, kind
                ));
            }
        };

        // Postfix operators: `.field` / `.method()`, `[index]`
        loop {
            if self.consume(&TokenKind::Dot) {
                let dot_span = expr.span();
                let field = self.expect_ident()?;
                if self.check(&TokenKind::LParen) {
                    // Method call: `obj.method(args)`
                    self.advance(); // (
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.consume(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    expr = Expr::MethodCall {
                        target: Box::new(expr),
                        method: field,
                        args,
                        span: dot_span,
                    };
                } else {
                    // Field access: `obj.field`
                    expr = Expr::FieldAccess {
                        target: Box::new(expr),
                        field,
                        span: dot_span,
                    };
                }
            } else if self.consume(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                let span = expr.span();
                self.expect(&TokenKind::RBracket)?;
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }
}

/// Check whether a token kind represents a type name.
///
/// Types can be written with the `Type` token kind (emitted by the lexer)
/// or as a generic `Ident` that happens to name a type (e.g. `int` could
/// appear as either depending on context).
fn is_type_kind(kind: &TokenKind, enum_names: &HashSet<String>) -> bool {
    matches!(kind, TokenKind::Type(_))
        || matches!(kind, TokenKind::Ident(s) if Type::from_str(s).is_some() || enum_names.contains(s))
}

impl Expr {
    /// Return the source-location span of this expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit(_, s) | Expr::FloatLit(_, s) | Expr::StringLit(_, s)
            | Expr::CharLit(_, s) | Expr::BoolLit(_, s) | Expr::Ident(_, s)
            | Expr::Call { span: s, .. } | Expr::BinaryOp { span: s, .. }
            | Expr::Assign { span: s, .. } | Expr::DeclAssign { span: s, .. }
            | Expr::PostIncrement { span: s, .. } | Expr::PostDecrement { span: s, .. }
            | Expr::Index { span: s, .. }
            | Expr::FieldAccess { span: s, .. } | Expr::MethodCall { span: s, .. }
            | Expr::StructLit { span: s, .. } => s.clone(),
            Expr::ArrayLit(_, s) => s.clone(),
        }
    }
}
