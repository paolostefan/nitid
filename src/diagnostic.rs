use std::fmt;
use std::fmt::write;
use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
  Lex,
  Parse,
  Sema,
  Import,
}

impl fmt::Display for Phase {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Phase::Lex => write!(f, "lexer"),
      Phase::Parse => write!(f, "parse"),
      Phase::Import => write!(f, "import"),
      Phase::Sema => write!(f, "semantic"),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
  pub phase: Phase,
  pub span: Span,
  pub message: String,
}

impl Diagnostic {
  pub fn new(phase: Phase, span: Span, message: impl Into<String>) -> Self {
    Self {
      phase,
      span,
      message: message.into(),
    }
  }
}

impl fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "In {} phase, ", self.phase);

    if self.span.line > 0 {
      write!(
        f,
        "{}:{}:{}: {}",
        self.span.file, self.span.line, self.span.col, self.message
      )
    } else if self.span.file.is_empty() {
      write!(f, "{}", self.message)
    } else {
      write!(f, "{}: {}", self.span.file, self.message)
    }
  }
}

impl From<Diagnostic> for String {
  fn from(d: Diagnostic) -> Self {
    d.to_string()
  }
}
