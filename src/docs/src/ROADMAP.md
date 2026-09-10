# Nitid Implementation Roadmap

**Current state:** ~4200 LOC Rust + ~140 LOC C runtime. Lexer/parser/sema/codegen support scalar types, if/while, multi-return, for loops (C-style + range-based), break/continue, postfix ++/--, source-span errors, no-implicit-cast enforcement, array types (fixed/dynamic), array literals, array indexing (incl. negative), range-based array traversal, structs with methods, enums (typed/untyped with overflow checking), and a `nitid_array` runtime with bounds-checked access.

**North Star (intermediate):** By the end of **v0.2**, write a small demo (plasma / fire / GL triangle + shaders) in roughly 100–150 lines—low-level and explicit, with minimal boilerplate—Amiga-era C energy, modern tooling.

**Note on resources:** Before v0.3, resource lifetime is **manual and explicit** (`SDL_Destroy*`, `free`, etc.). RAII / safe references land in v0.3 and upgrade the graphics layer; they do not block the first demos.

---

## Base features

### Composite types

| Feature | What’s involved |
|--------|------------------|
| Arrays | `Type::TyArray`, `Expr::ArrayLit`, `Expr::Index`. Parser handles `Type[...]`, `fixed`, declaration-level `[size]`. Sema validates element uniformity and indexing. Codegen emits `nitid_array_*`. |
| For-range iteration | `for (item : array)` / `for (idx, item : array)`. |
| Structs | `struct`, field access, struct literals. Codegen: C `typedef struct`. |
| Enums | C-like enums with optional underlying type, overflow checks, auto-increment. |
| Methods / dot-call | `obj.method(args)` with `self` injection; codegen as `StructName_method(&obj, args)`. |

### Strings handling

| Feature | What’s involved |
|--------|------------------|
| UTF-8 validation & Unicode escapes | Well-formed UTF-8 in literals; `\uXXXX` / `\UXXXXXXXX`. |
| String16 / String32 runtime | `nitid_string16` / `nitid_string32` + conversions. |
| Encoding-safe string ops | Code-point indexing; `+` and comparisons via runtime. |

---

## Development roadmap

### v0.1.0 — Module system

| # | Feature | What’s involved |
|---|---------|-----------------|
| v0.1.1 | File-level import resolution | `import Foo` → find package files, parse, merge into package symbol table. |
| v0.1.2 | Qualified access | `Foo.someFunc()` — parse + sema against imported package. |
| v0.1.3 | Import aliasing | `import Foo as f` → `f.someFunc()` (parsed; wire through). |
| v0.1.4 | Multi-file compilation | Dependency graph (DFS). One `.c` per source file; foreign prototypes; CMake lists all files. |
| v0.1.5 | Name conflict detection | Duplicate symbols across imports → error (real package name, not alias). |
| v0.1.6 | Mangled C names for imports | Prefix imported fns in C output (`Math_multiply`) to avoid flat-namespace clashes. |

**Exit criteria:** `import Math; Math.sqrt(16)` works across two `.nt` files. Two packages can export the same function name without linker conflicts.

---

### v0.2.0 — FFI, raw memory & graphics

Goal: open a window, draw via **software framebuffer** and/or **OpenGL shaders**, pump events, link **SDL2**—without drowning in boilerplate. Graphics is a first-class milestone, not a side quest.

#### Language / compiler

| # | Feature | What’s involved |
|---|---------|-----------------|
| v0.2.1 | `extern "C"` / foreign functions | Declare and call C functions; stable ABI to generated C (or direct linker symbols). |
| v0.2.2 | Raw pointers | `*T` / `*mut T` (or equivalent spelling): load/store, cast, pointer arithmetic where needed for GL/SDL buffers. |
| v0.2.3 | `repr(C)` / layout control | Struct layout matching C; opaque types (`SDL_Window`, `GLuint` wrappers, etc.). `sizeof` / `alignof` builtins as needed. |
| v0.2.4 | C-compatible extras | C string literals / `*const u8` helpers; function pointers (callbacks) at least for common SDL/GL patterns; explicit `unsafe` for deref and FFI. |
| v0.2.5 | Link flags / build glue | Pass-through `-l`/`-L` (or `link_lib "SDL2"`, `link_lib "GL"`); optional `pkg-config`; ability to compile/link a small C shim if required. |

#### Graphics stack (SDL2 + SW framebuffer + OpenGL from day one)

| # | Feature | What’s involved |
|---|---------|-----------------|
| v0.2.6 | SDL2 bindings (minimal) | Hand-written thin bindings: init/quit, window, events, timing, GL context creation (`SDL_GL_*`), surface/texture path as needed. Prefer 1:1 with C—no hidden magic. |
| v0.2.7 | Software framebuffer path | Pixel buffer → present (texture update or `SDL_UpdateWindowSurface` style). Helpers: `set_pixel`, `fill`, `blit`, clear. Immediate “chunky Amiga” feedback loop. |
| v0.2.8 | OpenGL path | Load GL entry points (glad/epoxy/static core profile—pick one and document it). Clear, viewport, VBO/VAO minimal path, shader compile/link, draw a triangle. |
| v0.2.9 | Shader workflow | Shaders from string/file; uniform setters; optional crude hot-reload (stat file + recompile). |
| v0.2.10 | `std.ffi` + experimental `std.gfx` | `std.ffi` / `std.c`: C types, null, casts. `std.gfx`: thin ergonomic layer (`Window`, `Event`, `Canvas`/`Framebuffer`, `Shader`) **always** escapable to raw SDL/GL pointers. |
| v0.2.11 | Killer examples | (1) window + clear, (2) plasma/fire/tunnel on SW framebuffer, (3) colored GL triangle + time uniform, (4) keyboard/mouse + vsync. Screenshots/GIFs in README. |

**Design rules for v0.2**

- **Two visible layers:** `sdl.*` / `gl.*` (almost 1:1 C) and `gfx.*` (comfortable; `.raw()` / `.ptr()` always available).
- **No GC; no hidden allocations in the render hot path.** Static buffers or bump/frame arenas are fine even before full RAII.
- **Manual resource management** is documented and expected (`DestroyWindow`, `glDelete*`, etc.).
- Bindings stay **minimal**—only what the examples need; grow driven by demos.

**Exit criteria:**

- A Nitid program links against **SDL2** and opens a window.
- Same program (or a sibling example) renders either a **software-framebuffer** effect *or* an **OpenGL** shader demo (ideally both in-tree).
- Event loop + input works; cleanup is explicit and leak-free under normal exit.
- A dangling FFI mistake may still be UB in unsafe paths; safe Nitid code should not need to touch raw GL/SDL without `unsafe`.

**Explicitly deferred out of v0.2:** full RAII, borrow checker, rich `Result` ergonomics (can use error codes), generics/traits, package manager, auto-bindgen.

---

### v0.3.0 — Memory safety

| # | Feature | What’s involved |
|---|---------|-----------------|
| v0.3.1 | Scoped allocation / RAII | `scope { ... }` or equivalent; drop at block exit; arenas with auto-cleanup. |
| v0.3.2 | Safe reference types | `ref T` (or `&T` / `&mut T` spelling TBD); borrow regions or ref-count as designed; checked deref where applicable. |
| v0.3.3 | Null prevention | `T?` / Option—no deref without check (or `match` when available). |
| v0.3.4 | Buffer overflow checks | All `a[i]` (and string index) through bounds checks; panic on violation. |
| v0.3.5 | Graphics upgrade | `Window`, `Shader`, `Texture`, GL objects with `drop`; frame arenas for per-frame data; keep unsafe escape hatches. |

**Exit criteria:** Dangling pointer or OOB in *safe* code → runtime panic (or compile-time rejection), not silent UB. SDL/GL wrappers can be used mostly without manual destroy in the common path.

---

### v0.4.0 — Concurrency

| # | Feature | Approach |
|---|---------|----------|
| v0.4.1 | Thread spawn | `spawn f(args)` → `pthread_create` (or platform equivalent). |
| v0.4.2 | Channels | `chan<T>`, `send` / `recv`. |
| v0.4.3 | Mutex / sync | `mutex`, scoped locking. |
| v0.4.4 | Race prevention | Borrow-style analysis and/or TSan-oriented codegen. |

**Exit criteria:** Two threads communicate over a channel without data races. (Classic split: logic thread vs render thread becomes expressible.)

---

### v0.5.0 — Standard library & polish

| # | Feature | Notes |
|---|---------|-------|
| v0.5.1 | `match` / switch | Pattern matching on enums and values. |
| v0.5.2 | Stdlib: math | Trig, log, pow. |
| v0.5.3 | Stdlib: collections | Vec, HashMap, string builder. |
| v0.5.4 | Stdlib: io | File read/write, networking basics. |
| v0.5.5 | Stdlib: cli | Argument parsing. |
| v0.5.6 | Match destructuring | Struct fields, enum variants. |
| v0.5.7 | Graphics polish | Use `Result`, richer `gfx` (sprites, camera2D, mesh helpers) without hiding SDL/GL. |

---

### v0.6.0 — Tooling & ecosystem

| # | Feature | What’s involved |
|---|---------|-----------------|
| v0.6.1 | Package manager | `nitid pkg`, versioned deps, registry later. |
| v0.6.2 | Real build system | Replace ad-hoc link flags; multi-package projects. |
| v0.6.3 | C bindgen / `@cImport`-style | Generate bindings from headers; SDL3/Vulkan/etc. become cheap. |
| v0.6.4 | Formatter, docs, LSP | `nitid fmt`, `nitid doc`, editor support. |
| v0.6.5 | Optional backends | Raylib/sokol as alternative “batteries” packages—not core requirements. |

---

### v0.7.0 — Nice to have

| # | Feature | What’s involved |
|---|---------|-----------------|
| v0.7.1 | Struct tags | Optional field metadata (`age: u8 "json:\"age\""`); reflection / CT API. |
| v0.7.2 | Embed file → variable | `go:embed`-style inclusion (shaders, assets). |
| v0.7.3 | I256 / U256 runtime | Soft wide integers in C runtime. |
| v0.7.4 | F8 / F16 runtime | Soft or hardware `_Float16` wrappers. |

---

### v1.0.0 — Stability

- Language + FFI layout (`repr(C)`) spec frozen enough for external bindings.
- Stdlib and `gfx`/`sdl`/`gl` packages versioned with compatibility promises.
- Documented unsafe obligations and panic model.

---

## Key architectural decisions to make

1. **Runtime library design** — grow from `nitid_string` / `nitid_array` toward umbrella `nitid_runtime.h` (`ref`, channels, mutex, …). Graphics stays **out** of the core runtime: link SDL2/GL as external deps.
2. **GL loading strategy** — ship with one documented approach (e.g. glad as a vendored C shim vs runtime loader). Same for SDL2 vs SDL3: **SDL2 first** for ecosystem maturity; SDL3 as a later package.
3. **Unsafe model** — narrow and explicit: pointer deref, FFI calls, and aliasing tricks require `unsafe`; safe wrappers live in `std.gfx`.
4. **Error style pre-match** — SDL/GL examples may use `bool` + last-error or simple status enums until `Result`/`match` land.

---

## Suggested v0.2 implementation order

1. `extern "C"` + link flags + hello `SDL_Init` / `SDL_Quit`
2. Opaque pointers + `repr(C)` structs + event poll loop + window clear
3. Software framebuffer present path + plasma/fire demo
4. `SDL_GL_CreateContext` + GL function loading + triangle
5. Shader from source + uniform time
6. Thin `std.gfx` API on top; keep raw SDL/GL public
7. README gifs + “how to link on Linux/macOS/Windows”

---

## Non-goals (near term)

- Full Vulkan/Metal abstraction in core
- Qt-style UI frameworks
- Package registry before three solid in-tree graphics demos
- Hiding C interop behind a black box
