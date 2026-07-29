/// Lexer (tokenizer) for the Nitid language.
///
/// Converts a raw source string into a sequence of [`Token`] values
/// that the parser can consume.  This is phase 1 of the transpilation
/// pipeline.
///
/// # Features
/// - Single-line (`//`) and block (`/* */`) comments.
/// - String and character literals with C-style escapes.
/// - Integer (decimal / hex) and floating-point literals.
/// - Keyword and type-name recognition.
///
/// # Error handling
/// All errors are returned as `Err(String)` with source location.
use crate::ast::Span;

/// Every kind of token the lexer can produce.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Keywords ──────────────────────────────────────────────
    Package,
    Import,
    As,
    Fn,
    Return,
    If,
    Else,
    While,
    For,
    Let,
    Var,
    True,
    False,
    Break,
    Continue,
    Fixed,
    Struct,
    Impl,
    Self_,
    Packed,
    Align,
    Enum,

    // ── Identifiers & literals ───────────────────────────────
    /// User-defined identifier, e.g. a variable or function name.
    Ident(String),
    /// Integer literal as a raw string (e.g. `"42"`, `"0xFF"`).
    IntLit(String),
    /// Float literal as a raw string (e.g. `"3.14"`, `"1e-5"`).
    FloatLit(String),
    /// String literal content (without surrounding quotes).
    StringLit(String),
    /// Char literal content (without surrounding quotes).
    CharLit(String),

    /// A type keyword (e.g. `int`, `float`, `string`).
    Type(String),

    // ── Punctuation ──────────────────────────────────────────
    Semicolon,
    Colon,
    Comma,
    Dot,
    Arrow,      // ->
    ColonEq,    // :=
    Eq,         // =
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,

    // ── Operators ────────────────────────────────────────────
    Plus, Minus, Star, Slash, Percent,
    PlusPlus, MinusMinus,
    Ampersand, Pipe, Caret, Tilde,
    Lt, Gt, Le, Ge, EqEq, Ne,
    AndAnd, OrOr,
    Shl, Shr,
    Bang,

    // ── Special ──────────────────────────────────────────────
    Hash,
    Underscore,
}

/// A single token with its source location.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// Create a new token with the given kind and source location.
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The lexer state machine.
///
/// Walks through the input character by character, tracking position
/// for error reporting.
pub struct Lexer {
    /// All characters of the input, collected up front for simplicity.
    chars: Vec<char>,
    /// Current index into `chars`.
    pos: usize,
    /// Current line number (1-based).
    line: usize,
    /// Current column number (1-based).
    col: usize,
    /// Source file name (for error messages).
    file: String,
}

impl Lexer {
    /// Create a new lexer for the given input and source file name.
    pub fn new(input: &str, file: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            file: file.to_string(),
        }
    }

    // ── Character-level helpers ───────────────────────────────

    /// Return the current character without consuming it.
    fn span(&self) -> Span {
        Span::new(&self.file, self.line, self.col)
    }

    /// Return the current character without consuming it.
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Look ahead `n` characters without consuming anything.
    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    /// Consume and return the current character, updating position.
    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        c
    }

    /// Skip over any whitespace characters (spaces, tabs, newlines, CRs).
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    // ── Keyword / type classification ─────────────────────────

    /// Check if `s` is a Nitid keyword and return the corresponding token kind.
    fn is_keyword(s: &str) -> Option<TokenKind> {
        match s {
            "package" => Some(TokenKind::Package),
            "import" => Some(TokenKind::Import),
            "as" => Some(TokenKind::As),
            "fn" => Some(TokenKind::Fn),
            "return" => Some(TokenKind::Return),
            "if" => Some(TokenKind::If),
            "else" => Some(TokenKind::Else),
            "while" => Some(TokenKind::While),
            "for" => Some(TokenKind::For),
            "let" => Some(TokenKind::Let),
            "var" => Some(TokenKind::Var),
            "true" => Some(TokenKind::True),
            "false" => Some(TokenKind::False),
            "break" => Some(TokenKind::Break),
            "continue" => Some(TokenKind::Continue),
            "fixed" => Some(TokenKind::Fixed),
            "struct" => Some(TokenKind::Struct),
            "impl" => Some(TokenKind::Impl),
            "self" => Some(TokenKind::Self_),
            "packed" => Some(TokenKind::Packed),
            "align" => Some(TokenKind::Align),
            "enum" => Some(TokenKind::Enum),
            _ => None,
        }
    }

    /// Check if `s` names a Nitid type.
    fn is_type(s: &str) -> bool {
        matches!(
            s,
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "f8"
                | "f16"
                | "f32"
                | "f64"
                | "int"
                | "float"
                | "double"
                | "string"
                | "string16"
                | "string32"
                | "bool"
                | "void"
        )
    }

    // ── Literal readers ───────────────────────────────────────

    /// Read a `"..."` string literal, handling C-style escape sequences.
    ///
    /// The opening `"` must already have been consumed by the caller.
    /// The closing `"` is consumed by this method.
    fn read_string(&mut self) -> Result<Token, String> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // consume opening "
        let mut s = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(format!(
                        "{}:{}:{}: Unterminated string literal",
                        self.file, start_line, start_col
                    ));
                }
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('\'') => s.push('\''),
                    Some('0') => s.push('\0'),
                    Some('u') => {
                        let code = self.read_hex_digits(4)?;
                        let c = char::from_u32(code).ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Invalid Unicode escape \\u{:04X}",
                                self.file, start_line, start_col, code
                            )
                        })?;
                        s.push(c);
                    }
                    Some('U') => {
                        let code = self.read_hex_digits(8)?;
                        let c = char::from_u32(code).ok_or_else(|| {
                            format!(
                                "{}:{}:{}: Invalid Unicode escape \\U{:08X}",
                                self.file, start_line, start_col, code
                            )
                        })?;
                        s.push(c);
                    }
                    // Unknown escape — pass through verbatim.
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => {
                        return Err(format!(
                            "{}:{}:{}: Unterminated escape in string",
                            self.file, start_line, start_col
                        ));
                    }
                },
                Some(c) => s.push(c),
            }
        }
        Ok(Token::new(
            TokenKind::StringLit(s),
            Span::new(&self.file, start_line, start_col),
        ))
    }

    /// Read a `'x'` character literal.
    ///
    /// The opening `'` must already have been consumed by the caller.
    /// Supports the same escape sequences as strings.
    fn read_char(&mut self) -> Result<Token, String> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // skip opening '
        let c = match self.advance() {
            None => {
                return Err(format!(
                    "{}:{}:{}: Unterminated char literal",
                    self.file, start_line, start_col
                ));
            }
            Some('\\') => match self.advance() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('\\') => '\\',
                Some('\'') => '\'',
                Some('0') => '\0',
                Some('u') => {
                    let code = self.read_hex_digits(4)?;
                    if code > 0xFF {
                        return Err(format!(
                            "{}:{}:{}: Unicode escape \\u{:04X} out of range for char",
                            self.file, start_line, start_col, code
                        ));
                    }
                    char::from_u32(code).ok_or_else(|| {
                        format!(
                            "{}:{}:{}: Invalid Unicode escape \\u{:04X}",
                            self.file, start_line, start_col, code
                        )
                    })?
                }
                Some('U') => {
                    let code = self.read_hex_digits(8)?;
                    if code > 0xFF {
                        return Err(format!(
                            "{}:{}:{}: Unicode escape \\U{:08X} out of range for char",
                            self.file, start_line, start_col, code
                        ));
                    }
                    char::from_u32(code).ok_or_else(|| {
                        format!(
                            "{}:{}:{}: Invalid Unicode escape \\U{:08X}",
                            self.file, start_line, start_col, code
                        )
                    })?
                }
                Some(_c) => {
                    return Err(format!(
                        "{}:{}:{}: Invalid escape sequence",
                        self.file, start_line, start_col
                    ));
                }
                None => {
                    return Err(format!(
                        "{}:{}:{}: Unterminated escape in char",
                        self.file, start_line, start_col
                    ));
                }
            },
            Some(c) => c,
        };
        // Expect closing quote.
        match self.advance() {
            Some('\'') => {}
            _ => {
                return Err(format!(
                    "{}:{}:{}: Expected closing ' in char literal",
                    self.file, start_line, start_col
                ));
            }
        }
        Ok(Token::new(
            TokenKind::CharLit(c.to_string()),
            Span::new(&self.file, start_line, start_col),
        ))
    }

    /// Read exactly `n` hex digits after a `\u` / `\U` escape.
    fn read_hex_digits(&mut self, n: usize) -> Result<u32, String> {
        let start = (self.line, self.col);
        let mut val: u32 = 0;
        for _ in 0..n {
            match self.advance() {
                Some(c) if c.is_ascii_hexdigit() => {
                    val = val * 16 + c.to_digit(16).unwrap();
                }
                Some(c) => {
                    return Err(format!(
                        "{}:{}:{}: Expected {} hex digits in Unicode escape, got '{}'",
                        self.file, start.0, start.1, n, c
                    ));
                }
                None => {
                    return Err(format!(
                        "{}:{}:{}: Unterminated Unicode escape, expected {} hex digits",
                        self.file, start.0, start.1, n
                    ));
                }
            }
        }
        Ok(val)
    }

    /// Read a numeric literal (integer or float).
    ///
    /// The first digit `first` must already have been peeked (not consumed).
    /// Handles:
    /// - Decimal integers: `42`
    /// - Hex integers: `0xFF`
    /// - Floats: `3.14`, `1e10`, `1.5e-3`
    fn read_number(&mut self, first: char) -> Token {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // consume first char
        let mut s = String::new();
        s.push(first);
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c.is_whitespace() && !is_float {
                // Space between digits in large numbers (e.g. `1 000 000`)?
                self.advance();
            } else if c == '.' && !is_float {
                let next = self.peek_ahead(1);
                if next.map_or(false, |n| n.is_ascii_digit()) {
                    is_float = true;
                    s.push(c);
                    self.advance();
                } else {
                    break;
                }
            } else if c == 'e' || c == 'E' {
                is_float = true;
                s.push(c);
                self.advance();
                if self.peek() == Some('+') || self.peek() == Some('-') {
                    s.push(self.advance().unwrap());
                }
            } else if c == 'x' || c == 'X' {
                s.push(c);
                self.advance();
                while let Some(h) = self.peek() {
                    if h.is_ascii_hexdigit() {
                        s.push(h);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let kind = if is_float {
                    TokenKind::FloatLit(s)
                } else {
                    TokenKind::IntLit(s)
                };
                return Token::new(kind, Span::new(&self.file, start_line, start_col));
            } else {
                break;
            }
        }
        let kind = if is_float {
            TokenKind::FloatLit(s)
        } else {
            TokenKind::IntLit(s)
        };
        Token::new(kind, Span::new(&self.file, start_line, start_col))
    }

    /// Read an identifier or keyword.
    ///
    /// The first character `first` must already have been peeked.
    /// Checks keyword and type tables before returning an `Ident`.
    fn read_ident(&mut self, first: char) -> Token {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // consume first char
        let mut s = String::new();
        s.push(first);
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        // Keywords take precedence over identifiers.
        if let Some(kw) = Self::is_keyword(&s) {
            return Token::new(kw, Span::new(&self.file, start_line, start_col));
        }
        // Type names are emitted as a special token kind.
        if Self::is_type(&s) {
            return Token::new(
                TokenKind::Type(s),
                Span::new(&self.file, start_line, start_col),
            );
        }
        Token::new(
            TokenKind::Ident(s),
            Span::new(&self.file, start_line, start_col),
        )
    }

    // ── Comment readers ───────────────────────────────────────

    /// Skip a single-line comment (`//`) until the next newline.
    fn read_comment_line(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    /// Skip a block comment (`/* ... */`), which may span multiple lines.
    fn read_comment_block(&mut self) -> Result<(), String> {
        self.advance(); // skip the `*` (the `/` was already consumed)
        while let Some(c) = self.peek() {
            if c == '*' && self.peek_ahead(1) == Some('/') {
                self.advance();
                self.advance();
                return Ok(());
            }
            self.advance();
        }
        Err(format!(
            "{}:{}:{}: Unterminated block comment",
            self.file, self.line, self.col
        ))
    }

    // ── Entry point ───────────────────────────────────────────

    /// Tokenise the entire input.
    ///
    /// Returns a `Vec<Token>` on success, or an error string
    /// describing the first invalid character or unterminated literal.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            let c = match self.peek() {
                None => break,
                Some(c) => c,
            };

            let tok = match c {
                // Comments
                '/' if self.peek_ahead(1) == Some('/') => {
                    self.read_comment_line();
                    continue;
                }
                '/' if self.peek_ahead(1) == Some('*') => {
                    self.read_comment_block()?;
                    continue;
                }

                // Punctuation
                ';' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Semicolon, span)
                }
                ':' if self.peek_ahead(1) == Some('=') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::ColonEq, span)
                }
                ':' if self.peek_ahead(1) == Some(':') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Colon, span)
                }
                ':' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Colon, span)
                }
                '-' if self.peek_ahead(1) == Some('>') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Arrow, span)
                }
                ',' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Comma, span)
                }
                '.' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Dot, span)
                }
                '(' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::LParen, span)
                }
                ')' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::RParen, span)
                }
                '{' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::LBrace, span)
                }
                '}' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::RBrace, span)
                }
                '[' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::LBracket, span)
                }
                ']' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::RBracket, span)
                }

                // Operators (multi-char checked before single-char)
                '=' if self.peek_ahead(1) == Some('=') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::EqEq, span)
                }
                '=' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Eq, span)
                }
                '!' if self.peek_ahead(1) == Some('=') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Ne, span)
                }
                '!' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Bang, span)
                }
                '+' if self.peek_ahead(1) == Some('+') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::PlusPlus, span)
                }
                '+' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Plus, span)
                }
                '-' if self.peek_ahead(1) == Some('-') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::MinusMinus, span)
                }
                '-' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Minus, span)
                }
                '*' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Star, span)
                }
                '/' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Slash, span)
                }
                '%' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Percent, span)
                }
                '&' if self.peek_ahead(1) == Some('&') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::AndAnd, span)
                }
                '&' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Ampersand, span)
                }
                '|' if self.peek_ahead(1) == Some('|') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::OrOr, span)
                }
                '|' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Pipe, span)
                }
                '^' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Caret, span)
                }
                '~' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Tilde, span)
                }
                '<' if self.peek_ahead(1) == Some('=') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Le, span)
                }
                '<' if self.peek_ahead(1) == Some('<') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Shl, span)
                }
                '<' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Lt, span)
                }
                '>' if self.peek_ahead(1) == Some('=') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Ge, span)
                }
                '>' if self.peek_ahead(1) == Some('>') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Shr, span)
                }
                '>' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Gt, span)
                }

                // Special
                '#' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Hash, span)
                }
                '_' => {
                    let span = self.span();
                    self.advance();
                    Token::new(TokenKind::Underscore, span)
                }

                // String / char / number / ident literals
                '"' => self.read_string()?,
                '\'' => self.read_char()?,
                c if c.is_ascii_digit() => self.read_number(c),
                c if c.is_ascii_alphabetic() || c == '_' => self.read_ident(c),

                // Anything else is an error.
                _ => {
                    return Err(format!(
                        "{}:{}:{}: Unexpected character '{}'",
                        self.file, self.line, self.col, c
                    ));
                }
            };
            tokens.push(tok);
        }
        Ok(tokens)
    }
}
