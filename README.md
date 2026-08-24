# Nitid - a C language evolution for the 21st century

This project aims at simplifying the C programming language, by using concepts that have become common in other
languages, as:

* packages/modules, as opposed to `#include`s;
* meaningful scoping of functions/vars;
* consistent and scoped memory management (as opposed to manual `malloc()/free()`);
* standard base types: 8,16,32,64,128-bit integers, 32 and 64-bit floats, strings;
* safe string management;
* standard UTF support for strings out-of-the-box;
* no implicit typecasts allowed;
* don't allow uninitialized variables;
* protection against improper use of pointers (null/invalid pointers are automatically destroyed);
* protection against buffer overflows (automatic bounds checking and deallocation);
* prevent race conditions by default;
* leveraging multithreading in a simple way;
* multiple function return values.

All this doesn't mean rejecting the C language: _au contraire_, Nitid **transpiles** the sources to valid C source
files. These can in turn be compiled by any C compiler.

## Comprehensive documentation

The full docs, including language specs, can be found [here](https://paolostefan.github.io/nitid/).

### Building documentation

The project includes a documentation site built with **mdBook** + **cargo doc**:

```bash
make        # builds everything (Rust API docs + language book)
make serve  # serves locally at http://localhost:8000
make open   # builds and opens in browser
make clean  # removes generated docs
```

The Makefile orchestrates `cargo doc --no-deps` and `mdbook build docs/`, then merges the Rust API reference under
`docs/api/`.

## Testing

Test infrastructure lives under `tests/`. The compilation pipeline is exposed as a library (`src/lib.rs`) so tests can
call it directly — no shelling out.

### Running

```bash
cargo test                    # all tests
cargo test all_valid_samples  # valid .nt samples
cargo test all_error_samples  # error samples
```

### Runtime C tests

The C runtime library (`runtime/`) has its own test suite covering `nitid_array` and `nitid_string`.

```bash
cmake -S runtime -B runtime/build -DBUILD_TESTS=ON
cmake --build runtime/build
./runtime/build/tests/nitid_runtime_tests
```

### Test categories

| Test file         | What it does                                                                                                                            |
|-------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| `tests/valid.rs`  | Compiles every`.nt` in `samples/` (e.g. `hello.nt`, `functions.nt`). Asserts the full pipeline (lex → parse → sema → codegen) succeeds. |
| `tests/errors.rs` | Compiles every`.nt` in `samples/errors/` and verifies the error message matches the embedded expectation.                               |

### Adding a new valid sample

Drop a `.nt` file in `samples/`. The batch test picks it up automatically.

### Adding a new error sample

Create a `.nt` file in `samples/errors/` with a comment header declaring what error to expect:

```nitid
// expect-contains: Type mismatch in arithmetic
// error raised in src/sema.rs:179
// Both operands of +, -, *, /, % must have the same type

x := 5 + "hello";
```

Two expectation keywords are supported:

- `// expect: <exact message>` — error must match exactly (used when the error message is deterministic and
  path-independent)
- `// expect-contains: <substring>` — error must contain the given substring (used when the message includes a file
  path, line number, or other variable text)

The batch runner parses the first matching line in the file header.

## Examples

### 1. Hello world

The most annoying code example ever follows.

```nitid
println("Hello, world!");
```

This will transpile to the following C code:

```c++
#include <stdio.h>  
   
int main(int argc, char **argv){  
    printf("Hello, world!\n");
    return 0;
}  
```

---

ℹ️ **Note on unwrapped/dangling code**

When the code in a sourcefile is dangling, that is, not wrapped in `fn functionName() {...}`, the transpiler
automatically creates a `main` function and assigns all the dangling code to it.

For safety, a return value of zero is added: this avoids unwanted scenarios, e.g.
`main()` returns an undefined value like `-1234` and a script checks if the executable errored by testing its retval
against 0.

If dangling code is found in _more than one source file_, a **transpiler error** is raised.

---

### 2. Multiple return values

In general, multiple return values are handled by pre-allocating them in the caller function and passing their addresses
to the function, as follows:

```rust
package add; // This will be the executable name  

fn myFunc(int a) -> (int, int) {
  return a;, a * 2;
}

fn main() {
  a, b: = myFunc(14);
  print("a=%d, b=%d", a, b);
}  
```

```c++
#include <stdio.h>  
   
/**  
 * Results' pointers are passed as arguments.  
 */  
void myFunc(int a, int *res0, int *res1) {  
    *res0 = a;  
    *res1 = a*2;  
}  
   
fn main() {  
   int a,b;  
   
   myFunc(14, &a, &b);  
   printf("a=%d, b=%d\n", a, b);  
}  
```

---

## Features implemented

| Feature                  | Notes                                                      |
|--------------------------|------------------------------------------------------------|
| Lexer & Parser           | Recursive-descent parser, precedence climbing              |
| AST definitions          | Full expression/statement AST                              |
| Semantic analysis        | Variable scoping, return-count checks, basic type checking |
| C codegen                | Full C output with CMake support                           |
| Multiple return values   | Desugared to output pointer params in C                    |
| `println` / `printf`     | Mapped to C `printf`                                       |
| String runtime           | Full UTF support                                           |
| `if` / `while`           | Full support                                               |
| Comments (`//`, `/* */`) |                                                            |
| Hex literals             |                                                            |
| Package declarations     | Parsed, used for CMake project name                        |
| `import` syntax          | Parsed but **not resolved** — no module loader             |

## Features missing / incomplete

The following README goals are **not yet implemented** or only partially implemented:

| Feature                                                     | Status     | What's missing                                                                                                                                           |
|-------------------------------------------------------------|------------|----------------------------------------------------------------------------------------------------------------------------------------------------------|
| **Module/package resolution**                               | ❌ Missing | `import` is parsed but does nothing. No module loader, no symbol resolution across files.                                                                |
| **Memory management**                                       | ❌ Missing | No RAII, no garbage collector, no scoped allocation. Falls back to manual `malloc`/`free` via C.                                                         |
| **Uninitialised variable enforcement**                      | ⚠️ Partial | `Sema` issues a warning for `:=` with inferred type but does not reject uninitialised declarations.                                                      |
| **No implicit type casts**                                  | ⚠️ Partial | Type mismatch is checked for binary ops, but numeric types are compared with `==` without range checking. `:=` inference allows silent numeric widening. |
| **Null/invalid pointer protection**                         | ❌ Missing | No concept of safe pointers in the language; raw C pointers are emitted.                                                                                 |
| **Buffer overflow protection**                              | ❌ Missing | No bounds checking on arrays or strings. The `nitid_string` runtime does not perform bounds checks.                                                      |
| **Race-condition prevention**                               | ❌ Missing | No concurrency model, no borrow checker, no data-race detection.                                                                                         |
| **Multithreading**                                          | ❌ Missing | No thread spawning, no channel, no sync primitives in the language.                                                                                      |
| **UTF support**                                             | ❌ Missing | The type system has `String16`/`String32` but no runtime implementation. The string runtime is ASCII-only.                                               |
| **Full type set (I256, U256, F8, F16, String16, String32)** | ❌ Missing | Types exist in the type system but have no C mapping or runtime. Using them produces broken C code.                                                      |
| **Arrays / slices**                                         | ❌ Missing | No slice types in the language.                                                                                                                          |
| **Standard library**                                        | ❌ Missing | Only `println` and `printf` are built in. No math, collection, or I/O APIs.                                                                              |
| **Source spans in error messages**                          | ⚠️ Partial | Many AST nodes use dummy spans (`Span::new("", 0, 0)`). Error locations are often missing or wrong.                                                      |
| **Codegen for multi-return in expression context**          | ⚠️ Partial | Multi-return calls work in `a, b := foo()` but not when used as sub-expressions.                                                                         |
