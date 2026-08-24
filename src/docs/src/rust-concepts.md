# Rust Concepts (for non-Rustaceans)

## Quick Tour of Rust for reading this codebase

| If you know...             | This Rust concept                        |
|----------------------------|------------------------------------------|
| `int x = 5;`               | `let x: i32 = 5;` (immutable by default) |
| `int x = 5;` (mutable)     | `let mut x: i32 = 5;`                    |
| `const` function in C++    | `fn foo(&self) -> Type`                  |
| `void` function in C++     | `fn foo(&mut self)` (no return)          |
| `std::vector<int>`         | `Vec<i32>`                               |
| `std::map<K, V>`           | `HashMap<K, V>`                          |
| `nullptr`                  | `None` (from `Option`)                   |
| `T*` (nullable)            | `Option<Box<T>>`                         |
| `union` / `enum` + union   | `enum` with variants and data            |
| `string` (C++ std::string) | `String` (owned) or `&str` (borrowed)    |
| Exceptions                 | `Result<T, E>` with `?` operator         |
| `static_cast<int>`         | `as` keyword: `x as i32`                 |
| `#include`                 | `mod` / `use`                            |
| `// comment`               | `// comment` (same)                      |
| `/* */` block comment      | `/* */` or `///` doc comment             |
| `assert(x == 5)`           | `assert_eq!(x, 5)`                       |

### Common patterns you'll see

```rust
// Creating a new instance
let mut lexer = Lexer::new(input, file);

// Calling a method
lexer.tokenize() ?;  // ? propagates errors

// Pattern matching
match value {
Some(x) => do_something(x),
None => handle_missing(),
}

// Destructuring a struct
let Lexer { chars, pos,..} = lexer;

// Iterating
for token in & tokens { /* ... */ }

// Type ascription
let x: Vec<Token> = Vec::new();
```

---


## `enum` (Rust's tagged union / sum type)

Like a C enum on steroids. Each variant can carry data:

```rust
enum TokenKind {
  Ident(String),      // carries a String
  IntLit(String),
  Fn,                 // carries nothing (like a plain C enum)
  Plus,
  Semicolon,
  // ...
}
```

You match on it with `match`:

```rust
match token.kind {
TokenKind::Ident(name) => { /* name is a String here */ }
TokenKind::Fn => { /* this is just a marker */ }
_ => { /* default case */ }
}
```

## `struct`

Like a C struct but with methods defined in an `impl` block:

```rust
struct Lexer {
  chars: Vec<char>,
  pos: usize,
}

impl Lexer {
  fn new(input: &str) -> Self { /* constructor */ }
  fn peek(&self) -> Option<char> { /* method */ }
  fn advance(&mut self) { /* mutating method */ }
}
```

- `&self` = read-only reference to the struct (like a `const` method in C++)
- `&mut self` = mutable reference (can modify the struct)
- `Self` = the type of the struct itself

## `Option<T>`

Rust's way of saying "maybe there's a value, maybe there isn't" — like a nullable pointer but safer:

```rust
fn peek(&self) -> Option<char> {
  self.chars.get(self.pos).copied()
}
```

Returns `Some(c)` if there's a character, or `None` if we're at the end.

## `Result<T, E>`

Either a success value `Ok(T)` or an error `Err(E)`:

```rust
fn tokenize(&mut self) -> Result<Vec<Token>, String>
```

Callers propagate errors with `?`:

```rust
let tokens = lexer.tokenize() ?;  // returns early if Err
```

## `Box<Expr>`

A pointer to heap-allocated data. Used for recursive types (like expressions containing sub-expressions):

```rust
enum Expr {
  BinaryOp {
    left: Box<Expr>,   // points to another Expr on the heap
    right: Box<Expr>,
    op: BinOp,
  },
  IntLit(i128, Span),
  // ...
}
```

If we didn't use `Box`, the type would be infinitely large (it'd contain itself).

## `#[derive(Debug, Clone)]`

Auto-implements debug printing and cloning for a type:

```rust
#[derive(Debug, Clone)]
struct Span {
  file: String,
  line: usize,
  col: usize,
}
```

- `Debug` lets you print it with `{:?}`
- `Clone` gives you a `.clone()` method to make a copy

## `HashMap<K, V>`

A hash table (dictionary / map):

```rust
let mut map: HashMap<String, Type> = HashMap::new();
map.insert("x".to_string(), Type::I32);
```

## `match`

Like C `switch` but way more powerful — can destructure, bind variables, and must be exhaustive:

```rust
match c {
'/' if self.peek_ahead(1) == Some('/') => { /* line comment */ }
'/' if self.peek_ahead(1) == Some('*') => { /* block comment */ }
'+' => { Token::new(TokenKind::Plus, self.span()) }
c if c.is_ascii_digit() => self.read_number(c),
_ => return Err("unexpected character"),
}
```

## `&` and `&mut` (references)

- `&T` = immutable reference (read-only, borrow)
- `&mut T` = mutable reference
- Rust's borrow checker enforces: either one `&mut` OR many `&`, never both at once. This prevents data races at compile
  time.