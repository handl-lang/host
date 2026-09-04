# ADR-0083: FFI-Native Stdlib — Handl over Direct C/POSIX Bindings

## Status

Accepted. Supersedes [ADR-0051 — `ffi-rust` Rust Binding
Façade](./0051-ffi-rust-binding-facade.md) (Proposed, never built — the Rust
closure binding layer is eliminated in favor of direct FFI data bindings).
Refines [ADR-0077 — Stdlib Migration to
Handl](./0077-stdlib-migration-to-handl.md) Phase 4: native libs migrate to
handl over **direct FFI bindings**, not Rust `Function` bodies in `OpRegistry`.
ADR-0077 Phases 1–3, 5–6 remain valid (pure-logic libs, axioms, capabilities,
imports). Depends on [ADR-0078 — Value Types as
Data](./0078-value-types-as-data.md) Phase 2 (`TaggedValue` + `Bytes` type)
for buffer FFI ops. Builds on [ADR-0044 — Language as
Data](./0044-language-as-data-and-runtime-split.md) and [ADR-0052 — Arrow
Calculus Kernel](./0052-arrow-calculus-kernel.md).

## Context

### Where we are

ADR-0077 established the `op`/`handl` declaration form and the `OpRegistry` +
`HandlLibraryPlugin` mechanism. ~50 of 56 `plugin-lib-*` crates already wire
`with_ops`/`OpRegistry`. The migration is ~80% done by crate count.

But the remaining step — ADR-0077 Phase 4 (native libs) — still proposes
keeping **Rust `Function` bodies** as the `handl` implementations:

```handl
op "fs.read" : Path -> Bytes
handl "fs.read" = rust_fs_read   -- Rust body wrapping std::fs::read
```

This means every native lib keeps a Rust crate that wraps a Rust ecosystem
crate (`std::fs`, `rustls`, `hyper`, `tokio`, `sha2`, …). The `Module`/
`Plugin` boilerplate is eliminated, but the Rust *implementation* stays —
one crate per native lib, forever.

### Why this is the wrong stopping point

1. **The Rust wrappers add nothing over the C they wrap.** `std::fs::read`
   calls `open(2)` + `read(2)` + `close(2)`. `sha2` calls OpenSSL or a
   Rust-native implementation. `hyper` wraps `epoll`/`kqueue` + HTTP framing.
   The battle-tested layer is the kernel (POSIX) and the C library (OpenSSL,
   zlib, libcrypto) — not the Rust wrapper. Going direct FFI cuts out the
   middleman without losing the battle-testing.

2. **Rust crate dependencies churn; C ABIs don't.** POSIX hasn't changed
   meaningfully in decades. OpenSSL's C ABI is older than Rust. `tokio`/
   `hyper`/`reqwest` churn their APIs every release. A POSIX FFI binding is
   *more* stable than a Rust crate dependency.

3. **The Rust impl crate layer is debt you'd pay to remove later.** If the
   long-term goal is "stdlib is handl, Rust stops growing," building Rust
   impl crates first and deleting them later is wasted work. Build the end
   state directly.

4. **The Rust stdlib crates couple the stdlib to kernel internals.** Today
   every `plugin-lib-*` crate depends on `handl-platform` (`Module`/
   `Function`/`EvalContext` traits). If the kernel changes these traits,
   every stdlib crate breaks. FFI bindings depend only on a stable C ABI —
   the kernel can evolve freely behind it.

### What already exists

- **`op`/`handl` declaration form** (ADR-0077 Phase 2) — `op "x.y" : T -> T`
  + `handl "x.y" = impl` in `.hnl` source. The `impl` currently names a Rust
  identifier; this ADR adds a `ffi_bind` op.
- **`OpRegistry` + `OpFunction`** (`plugin-lib-handl/src/op_registry.rs`) —
  op name → `Function` impl dispatch. `OpFunction` is the eval-time bridge.
  The `Function` trait is the dispatch contract; an `FfiOpFunction` impls it
  by doing `lib.get(symbol)` + marshal + call.
- **`plugin-loader-native`** (`crates/plugin-loader-native/src/lib.rs`) —
  already does `libloading::Library::new` + `extern "C" fn` symbol lookup.
  Today it loads one symbol per plugin (`handl_plugin_run`); this ADR
  generalizes it to N symbols per module with per-symbol marshalling.
- **Cranelift JIT** (`plugin-engine-eval-cranelift`) — compiles
  `ArrSource::Foreign(name)` to a native call into the module registry. The
  handl abstraction layer (arrow graph over FFI ops) JIT-compiles to native
  code; each `op` is one native call plus per-arg marshalling. For I/O-bound
  ops the marshalling cost is negligible relative to the syscall; for
  compute-bound hot loops (e.g. per-block crypto) it dominates and should
  be avoided by calling a single C function for the whole operation.
- **Pure-logic libs already migrated** (`stats`, `hello`, `collections`,
  `fn`, `cmp`, `prim`, …) — these are handl arrow graphs, not Rust bodies.
  They don't need FFI; they're already done (ADR-0077 Phase 1).

### The gap

The `handl "x" = rust_x` form names a Rust identifier resolved at compile
time. There is no way to bind an op to an external C symbol from handl
source. The FFI dispatcher (`plugin-loader-native`) exists but operates at
plugin granularity (one symbol), not op granularity (N symbols with
marshalling).

## Decision

### 1. The `ffi_bind` op — FFI bindings via host-exposed arrow

FFI bindings use a **host-exposed op** (`ffi_bind`), not new surface
syntax. This is consistent with ADR-0074 (the surface syntax spec) which
has no function-call form `f(x)` or type-descriptor syntax. The
`ffi_bind` op is called using approved surface syntax (juxtaposition,
positional arrays, records) and returns a `Function` value that the
`handl` declaration binds to an op:

```handl
--- libc.open — open a file, return file descriptor (or -1 on error).
op "libc.open" : (Path, Flags, Mode?) -> Fd
  guarantee: Or(Ge(output, 0), IsError)
handl "libc.open" = ffi_bind "libc" "open" ["cstr", "i32", "i32?"] "i32" { on: "Lt(ret, 0)", errno: true }
```

The `ffi_bind` op takes:
- **Library name** (`String`) — resolved by the dynamic loader
  (`dlopen`/`LoadLibrary`). May be a concrete name (`"libc.so.6"`) or an
  alias resolved by the host (`"libc"` → platform-specific path).
- **Symbol name** (`String`) — the C symbol (`dlsym`/`GetProcAddress`).
- **Argument marshalling descriptors** (`[String]`) — one per argument,
  as string values (e.g. `"cstr"`, `"i32"`, `"i32?"`).
- **Return marshalling descriptor** (`String`) — the C return type.
- **Error clause** (optional `Record`) — `{ on: <condition>, errno: true }`.

It returns a `Function` value (an `FfiOpFunction`) that the `handl`
declaration registers in `OpRegistry`. The expression is evaluated at
module load time; the resulting `Function` is the op's implementation.

**This extends ADR-0077's `handl` form**: the RHS becomes a surface
expression that evaluates to a `Function` value, not just a Rust
identifier. `handl "x" = rust_x` (ADR-0077) and `handl "x" = ffi_bind
...` (this ADR) are both valid — the RHS is evaluated at load time and
the result is registered. No new surface syntax is invented; only the
`ffi_bind` op is new (a `Function` impl in `plugin-ffi`, registered
by the host).

### 1.1. Capability gating of `ffi_bind`

`ffi_bind` is a capability-gated op. A `.hnl` file calling
`ffi_bind` must hold the `ffi.<library>` capability corresponding to
the library being bound. Without it, the call is rejected at load time.

```handl
--- stdlib/libc/linux.hnl — requires ffi.libc capability
cap ffi.libc
handl "libc.open" = ffi_bind "libc" "open" ["cstr", "i32", "i32?"] "i32" { on: "Lt(ret, 0)", errno: true }
```

**Capability granularity: per library.** Each C library gets its own
capability (`ffi.libc`, `ffi.openssl`, `ffi.zlib`, …). A `.hnl` file
binding `ffi_bind "openssl" ...` must hold `cap ffi.openssl`. This
prevents an untrusted `.hnl` file from binding `ffi_bind "libc"
"system" ["cstr"] "i32"` — it lacks `cap ffi.libc`.

**Library name resolution is host-controlled.** The library name passed
to `ffi_bind` is an alias resolved by the host, not a filesystem
path. The host maps `"libc"` to the platform-appropriate shared object
(`libc.so.6` on Linux, `libSystem.dylib` on macOS). Untrusted `.hnl`
files cannot specify arbitrary `.so`/`.dylib`/`.dll` paths — the host
rejects path-like library names and only resolves registered aliases.
The set of registered aliases is a host configuration decision, not a
`.hnl`-level declaration.

**Guest runtimes (§9) receive no `ffi.*` capabilities by default.** The
host scheduler grants specific `ffi.*` capabilities to a guest runtime
based on its privilege level. A sandboxed guest gets none; a trusted
guest gets `ffi.libc` for POSIX I/O. This is the security boundary
between host and guest.

### 2. Marshalling types

A fixed, small set of marshalling descriptors covers the vast majority of C
ABIs:

| Descriptor | C type | handl value | Marshalling |
|---|---|---|---|
| `i32` | `int32_t` | `Number` | check fit, pass by value (error on overflow) |
| `i64` | `int64_t` | `Number` | check fit, pass by value (error on overflow) |
| `f64` | `double` | `Number` | pass by value |
| `cstr` | `const char*` | `String` | null-terminate UTF-8, error on embedded NUL, pass pointer |
| `cbytes` | `const void* + size_t` | `Bytes` | raw bytes, no UTF-8 validation, (ptr, len) pair |
| `in_buf` | `const void* + size_t` | `Bytes` | C reads from buffer, handl keeps ownership, (ptr, len) pair |
| `out_buf(n)` | `void* + size_t` | `Bytes` (output) | allocate `n` bytes, pass `(ptr, n)` as two C args, return `{ ret, buf }` where `buf` is the first `ret` bytes (or all `n` if `ret` is not a count) |
| `ptr` | `void*` | opaque `Handle` | pass through raw pointer |
| `void` | `void` | `Unit` | no return value |
| `handle` | `void*` | opaque `Handle` | wrap in resource table entry |
| `out_struct(fields)` | `struct T*` (out-param) | handl record | allocate struct with `#[repr(C)]` layout, pass ptr, extract fields by computed offset after call |
| `i32?` / `i64?` / … | optional C arg | `Number?` | pass only if the caller provides a value; omit from the C call otherwise (for conditional args like `open`'s `mode`) |
| `...` | variadic tail | remaining args | C variadic call (e.g. `printf`); each trailing arg marshalled by its handl type |

**Integer marshalling checks for overflow, does not truncate silently.**
A `Number` marshalled to `i32` that doesn't fit in `[-2^31, 2^31)` raises
a marshal error at call time, not a silent truncation. Same for `i64`.
This is consistent with handl's correctness-first philosophy — silent
truncation would violate declared axioms without warning.

**`cstr` rejects embedded NUL bytes.** A `String` containing `\0`
cannot be marshalled to a C `const char*` (NUL terminates the string in
C). The dispatcher raises a marshal error rather than silently truncating
at the first NUL. Use `cbytes` for byte strings that may contain NUL.

**Buffer direction matters.** `in_buf` (C reads from handl bytes) and
`out_buf` (C writes into allocated buffer, handl receives the result) are
distinct descriptors. `out_buf(n)` allocates `n` bytes and passes
`(ptr, n)` as **two consecutive C arguments** — the pointer and the
buffer size. The descriptor expands to two C args, not one. The return
value is a record `{ ret: <return type>, buf: Bytes }` where `buf`
contains the first `ret` bytes of the allocated buffer (for functions
like `read` where the return is a byte count), or all `n` bytes (when
the return is a status code, not a count — the caller discards excess).
The caller accesses the result via `result.buf`.

Example for `read(fd, void *buf, size_t count)`:
```handl
handl "libc.read" = ffi_bind "libc" "read" ["i32", "out_buf:8192"] "i64" { on: "Lt(ret, 0)", errno: true }
```
The descriptor list `["i32", "out_buf:8192"]` expands to three C args:
`fd, ptr, 8192`. The return is `{ ret: i64, buf: Bytes }`.

`cbytes` is for raw byte strings (e.g. Linux file paths that may not be
UTF-8). Unlike `cstr`, it does not null-terminate or validate encoding.
Requires the `Bytes` type (see §2.1).

`out_struct` declares the struct layout inline. The dispatcher allocates
a struct with **`#[repr(C)]` layout** — field offsets are computed using
C alignment and padding rules for the target ABI, not sequential
packing. After the C call, each field is extracted at its computed
offset. The declared field types must match the platform's actual C
struct; platform variants (§6) handle per-platform struct differences
via separate `.hnl` files. Example:

```handl
handl "libc.stat" = ffi_bind "libc" "stat" ["cstr", "out_struct:{st_dev:i64,st_ino:i64,st_mode:i32,st_size:i64,st_mtime:i64}"] "i32" { on: "Lt(ret, 0)", errno: true }
```

The dispatcher computes offsets as `#[repr(C)]` would: `st_dev` at
offset 0, `st_ino` at offset 8 (after 8-byte `i64` + padding),
`st_mode` at offset 20 (after another `i64`), etc. If the platform's
`struct stat` has a different field order or types, a separate
`stdlib/libc/{platform}.hnl` declares the correct layout.

`handle` wraps a raw C pointer in an opaque handl value tracked by a
per-runtime resource table. POSIX file descriptors are plain `i32`/`i64`
and do not need `handle` — they pass through directly. See §2.2 for
handle lifecycle.

### 2.0. Optional and variadic arguments

C functions with conditional or variadic trailing arguments need
descriptors that match the C ABI:

**Optional arguments (`?` suffix).** POSIX `open(path, flags, mode)`
requires `mode` only when `O_CREAT` is set. The `?` suffix marks an
argument as optional — the dispatcher passes it only if the handl caller
provides a value, and omits it (calling the C function with fewer args)
otherwise:

```handl
handl "libc.open" = ffi_bind "libc" "open" ["cstr", "i32", "i32?"] "i32" { on: "Lt(ret, 0)", errno: true }
```

When called as `libc.open(path, O_RDONLY)`, the dispatcher calls
`open(path, O_RDONLY)` (2 C args). When called as
`libc.open(path, prim.or(O_CREAT, O_WRONLY), 0o644)`, it calls
`open(path, O_CREAT | O_WRONLY, 0o644)` (3 C args). This matches C's
variadic calling convention for `open`.

**Variadic tail (`"..."` descriptor).** True C variadics like `printf`
take a trailing sequence of args whose types are not declared in the
function signature. The `"..."` descriptor marks the variadic tail; each
trailing handl arg is marshalled by its handl type (`Number` → `i64` via
default promotion, `String` → `cstr`, `Bytes` → `cbytes`):

```handl
handl "libc.printf" = ffi_bind "libc" "printf" ["cstr", "..."] "i32"
```

Variadic FFI is inherently unsafe (no type checking on trailing args)
and should be avoided where possible — prefer wrapped handl functions
that call typed FFI ops. `printf` is the main use case; most POSIX
functions are not variadic.

### 2.1. The `Bytes` type — prerequisite for buffer FFI

The `in_buf`, `out_buf`, and `cbytes` descriptors require a `Bytes` type —
a contiguous byte buffer represented as `Rc<Vec<u8>>`, not
`serde_json::Value::Array` of `Value::Number`. Without it, a 1MB file read
becomes 1 million `Value::Number` allocations — unusable.

**ADR-0078 (Value Types as Data) Phase 2 introduces `TaggedValue`** as the
native representation, replacing `serde_json::Value`. ADR-0078 does not
yet declare a `Bytes` type. This ADR requires adding `Bytes` to
ADR-0078's `value_types.yaml`:

```yaml
- id: Bytes
  description: contiguous byte buffer
  kind: scalar
  implements: [Iterable, Lengthable, Serializable, Indexable]
```

With native representation:
```yaml
- target: native
  type: Bytes
  layout: "Rc<Vec<u8>>"
```

**ADR-0078 Phase 2 (TaggedValue + Bytes) is a prerequisite for ADR-0083
Phase 1** (POSIX layer with buffer ops). Phase 0 (FFI dispatcher with
simple types: `i32`, `cstr`) can proceed without it to prove the
mechanism.

### 2.2. Handle lifecycle

Handl values today (`serde_json::Value`) and near-future (`TaggedValue`,
ADR-0078) have no finalizer/Drop hook. When a handl value holding a C
resource (file descriptor, opaque pointer) goes out of scope, the resource
leaks. There is no GC, no RAII.

**Short-term (Phase 0–2): explicit close in handl.** POSIX file
descriptors are plain `i32` — they pass through as numbers. Handl code
calls `libc.close(fd)` explicitly. Leaks due to early return or panic are
programmer error, same as C. For opaque C pointers (`handle` descriptor),
the FFI dispatcher maintains a resource table mapping `handle_id →
(raw_ptr, finalizer_fn)`. Finalizers run on runtime shutdown as a safety
net — not ideal for long-running processes, but prevents permanent leaks.

**Long-term (Phase 4 prerequisite for async runtime): finalizable
values.** Extend `TaggedValue` with a `Resource` variant carrying
`Rc<RefCell<Option<RawHandle>>>` + a finalizer closure. When the last
`Rc` drops, the finalizer runs. This is RAII — the correct long-term
answer. The FFI dispatcher is designed to plug into finalizable values
when they exist, not reinvent them.

**Finalizable values are a Phase 4 prerequisite.** The async runtime
(§9) is a long-lived server process. Shutdown-only finalizers are
insufficient — fd leaks from early returns or task cancellation
accumulate over a server's lifetime and exhaust the fd table. The async
scheduler (Phase 4) requires finalizable values so that cancelled tasks
release their fds automatically. Phase 4 cannot ship without this.

### 3. Error handling

POSIX convention: return value `< 0` (or `== -1`) signals error; `errno`
holds the detail. `ffi_bind` accepts an optional error clause record
as its last argument:

```handl
handl "libc.open" = ffi_bind "libc" "open" ["cstr", "i32", "i32?"] "i32" { on: "Lt(ret, 0)", errno: true }
```

When the error condition matches, the FFI dispatcher extracts `errno`
(via `__errno_location` on Linux, `errno` global on macOS, `GetLastError`
on Windows) and returns an `Err(errno)` handl value instead of the raw
return. Without an error clause, the raw return value passes through.

**There is no callable `libc.errno()` op.** errno is read synchronously
by the dispatcher immediately after the C call returns, inside
`Function::call`, before any handl code runs. This is safe because
`errno` is thread-local and the dispatcher operates on the same thread
as the C call. A separate `errno()` op would be racy — handl code (or
another FFI call) between the failing call and the errno read would
clobber `errno`. The error clause is the *only* mechanism for obtaining
errno. The async scheduler (§9) must not yield between a C call and its
errno read — the dispatcher guarantees this by reading errno inside
`Function::call` before returning to the engine.

### 4. The FFI dispatcher

One Rust crate (`plugin-ffi`, or extend `plugin-loader-native`) provides
the generic dispatcher. It is the **only** new Rust.

```
plugin-ffi  (leaf)
  ├─ handl-platform      (Function, EvalContext, EvalError)
  ├─ plugin-lib-handl    (OpFunction, OpRegistry)
  ├─ libloading          (dlopen/dlsym)
  └─ serde_json
```

The dispatcher:
1. **Loads libraries** (cached by name — `dlopen` once, reuse).
2. **Looks up symbols** (cached by library + symbol — `dlsym` once).
3. **Marshals args** per the descriptor (handl `Value` → C types).
4. **Calls the symbol** (`unsafe extern "C"`).
5. **Unmarshals the return** (C type → handl `Value`).
6. **Checks error clause** (if present, extract errno, return `Err`).

The dispatcher implements `Function` (the same trait `OpFunction` dispatches
to), so it plugs into the existing `OpRegistry` with no engine changes. An
`FfiOpFunction` is just a `Function` whose `call` does the marshal/call/
unmarshal dance.

**Compile-time marshalability checking.** The typechecker verifies that
each `op` parameter type is marshalable to its corresponding `ffi_bind`
descriptor type. The `op` signature declares handl types (`Path`, `Flags`);
the `ffi_bind` call declares C marshalling types (`"cstr"`, `"i32"`). The
typechecker enforces: `String`/`Path` → `cstr`/`cbytes`, `Number` →
`i32`/`i64`/`f64`, `Bytes` → `in_buf`/`out_buf`/`cbytes`, `Handle` →
`ptr`/`handle`. Non-marshalable combinations (e.g. `Bytes` → `cstr`) are
compile errors, not runtime marshal failures. This catches type/descriptor
mismatches before any C call is made.

**The marshalling type set is the FFI calculus vocabulary.** The
descriptor set (`i32`, `cstr`, `out_buf`, `out_struct`, …) is a fixed
Rust enum — adding a new marshalling pattern is a kernel edit. This is
consistent with the engine-genericity rule: the marshalling types are
primitive *forms* (like `LexerPattern` variants), not language
constructs. The escape hatch for unsupported marshalling is a per-op
handl wrapper that decomposes the call into supported descriptors (e.g.
a complex struct return becomes an `out_struct` + field extraction in
handl, not a new descriptor). If a genuinely new marshalling pattern is
needed for a wide class of C functions, it is added to the kernel as a
new descriptor variant — a bounded, infrequent change, not per-lib
Rust.

### 5. The abstraction layer in handl

Native libs become handl arrow graphs over raw FFI ops:

```handl
--- fs.read — read a file as bytes.
--- Contract: Or(IsOk(output), IsError(output))
fs.read = path ~>
  match libc.open(path, O_RDONLY) {
    Err(errno) => Err(errno)
    Ok(fd) => {
      result = libc.read(fd, out_buf(BUF_SIZE))
      libc.close(fd)
      match result {
        Err(errno) => Err(errno)
        Ok(r)      => Ok(r.buf)
      }
    }
  }
```

The abstraction layer (error handling, retry logic, buffering, encoding) is
handl-authored. The leaf operations (`open`, `read`, `write`, `close`) are
FFI data bindings. No Rust bodies.

### 5.1. POSIX constants via regular bindings

POSIX constants (`O_RDONLY`, `SIGTERM`, `AF_INET`, `EPOLLIN`) are C
`#define` macros — they are not symbols and cannot be resolved via
`dlsym`. They must be declared as named values in handl.

**No new `const` declaration is needed.** ADR-0074 was right to remove
`const` from the surface — regular bindings of literals are already
constants. The JIT inlines literal bindings; there is no runtime cost.
Use existing surface syntax (`name = literal`):

```handl
--- POSIX open flags (Linux)
O_RDONLY   = 0
O_WRONLY   = 1
O_RDWR     = 2
O_CREAT    = 64
O_TRUNC    = 512
O_APPEND   = 1024
O_CLOEXEC  = 524288

--- POSIX signals (Linux)
SIGHUP     = 1
SIGINT     = 2
SIGTERM    = 15

--- epoll event flags (Linux)
EPOLLIN    = 1
EPOLLOUT   = 4
```

**Semantics:**
- **Type:** bindings of integer literals are `Number` (inferred). No
  explicit type annotation needed.
- **FFI coercion:** when a binding is passed to an FFI op, the
  typechecker coerces it to the descriptor's C type (`i32`, `i64`, …).
  Overflow is checked at compile time against the descriptor type.
- **References:** a binding may reference other bindings:
  `O_RW = prim.or(O_RDONLY, O_RDWR)`. Dependencies are resolved in order;
  circular references are a compile error.
- **Scope and export:** bindings are scoped to the `.hnl` file. They are
  exportable via `export O_RDONLY` (same as `op`/`cap`), making them
  available to importing modules.
- **Passability:** bindings are regular `Number` values — they can be
  passed to any handl function expecting a `Number`, not just FFI ops.

**Flag composition:** POSIX flags are combined with bitwise OR. Handl's
`prim` atoms today are `add`, `compare`, `alloc` — no bitwise OR. Two
options:

1. **Add `prim.or` as a kernel atom** (simplest — a small addition to
   `plugin-lib-prim`: the atom + type checking + registration). Flag
   composition: `prim.or(O_RDONLY, O_CLOEXEC)`.
2. **A macro system** (future ADR) that computes flag combinations at
   compile time: `macro OPEN_READ = O_RDONLY | O_CLOEXEC`. More powerful
   but a larger design — parameterized macros, code generation, etc.

**Decision: add `prim.or` now.** A full macro system is defined in
[ADR-0084 — Arrow Macros](./0084-arrow-macros.md). `prim.or` is a
degenerate macro — ADR-0084 subsumes it
when implemented. Until then, `prim.or` is a small addition to
`plugin-lib-prim` (the atom + type checking + registration) and unblocks
flag composition.

### 5.2. Stdlib layout

The stdlib lives at the **repo root** under `stdlib/`, not inside the
Rust workspace (`handl-packages/` is the old location and is migrated):

```
stdlib/
  libc/
    linux.hnl          — ffi_bind bindings for Linux POSIX (Phase 1)
    darwin.hnl         — ffi_bind bindings for macOS POSIX (future)
    windows.hnl        — ffi_bind bindings for Windows Win32 (future)
  fs/
    fs.hnl             — file system abstraction over libc
  os/
    os.hnl             — env, process, signals over libc
  net/
    net.hnl            — sockets over libc
  crypto/
    crypto.hnl         — hashing, encryption over OpenSSL FFI
  compress/
    compress.hnl       — zlib FFI
    gzip.hnl           — gzip over zlib
  concurrent/
    scheduler.hnl      — epoll-based async scheduler (Phase 4)
    task.hnl           — task spawning, wakers, cancellation
  ...
```

Each package is a directory under `stdlib/{pkg-name}/`. A package may
contain multiple `.hnl` files (e.g. `concurrent/scheduler.hnl` +
`concurrent/task.hnl`). Platform variants live under `stdlib/libc/` and
are selected by the host based on `cfg!(target_os)`.

This separates the stdlib (handl data) from the kernel (Rust crates under
`crates/`). The repo root reflects the architecture:

```
stdlib/     — the standard library (handl, data)
crates/     — the kernel (Rust, frozen)
project/    — ADRs, docs
```

### 6. Platform variants — Linux first, abstracted from day one

POSIX differs across platforms (`epoll` vs `kqueue` vs IOCP, `__errno_location`
vs `errno` global, `open` vs `openat`). The interface/impl split from
ADR-0077 §"Multi-target support" applies, but the "impl" is now a per-platform
`.hnl` file, not a Rust crate:

```
stdlib/libc/linux.hnl   — epoll, __errno_location, openat    (Phase 1)
stdlib/libc/darwin.hnl  — kqueue, errno global, open         (future)
stdlib/libc/windows.hnl — IOCP, GetLastError, CreateFileW    (future)
```

**Linux is the initial target, but the abstraction is designed from day
one.** The abstraction layer (`stdlib/fs/fs.hnl`, `stdlib/net/net.hnl`, …)
imports from a platform-selected `libc.hnl` and is platform-neutral — it
only uses the POSIX surface, not Linux-specific extensions. The host
selects the platform `.hnl` based on `cfg!(target_os)`.

Adding a platform = providing the same op interface over a different C ABI
in a new `stdlib/libc/{platform}.hnl` file. The abstraction layer does not
change. This is the same data-driven principle as the rest of the ADR: a
new platform is a new `.hnl` file, not a kernel recompile.

### 7. What stays Rust (the kernel)

The Rust surface area is fixed and minimal. It does not grow when you add
a stdlib lib.

| Crate | Role | Grows? |
|---|---|---|
| `handl-language` | `Language` data model, `Ast`, `Scheme`, `Value` | No (calculus vocabulary) |
| `handl-runtime` | Binds `Language` to engines, staged lex/parse/eval | No |
| `handl-value` | Value type representations | No |
| `platform` | `Plugin`/`Module`/`Function` traits, `ModuleRuntime` | No |
| `plugin-module` | `ModuleRuntime`, `ModuleRule`, `ModuleRegistry` | No |
| `plugin-engine-lex-pattern` | Lexer engine | No |
| `plugin-engine-parse-pratt` | Parser engine | No |
| `plugin-engine-eval-tree-walk` | Tree-walk interpreter | No |
| `plugin-engine-eval-cranelift` | JIT compiler | No |
| `plugin-engine-eval-bytecode` | Bytecode VM | No |
| `plugin-engine-typecheck-nbe` | Typecheck engine | No |
| `plugin-lib-handl` | Generic `.hnl` loader (`HandlModule`, `OpRegistry`) | No |
| `plugin-ffi` | FFI dispatcher (this ADR) | No |
| `plugin-loader-native` | Dynamic library loading | No |
| `plugin-lib-prim` | Calculus atoms (`add`, `compare`, `alloc`, `or`) — kernel vocabulary, not stdlib | No |
| `plugin-lib-lang-*` (6 crates) | Meta functions (eval/lex/parse/compile/typecheck/run_compiled) — the self-hosting bridge | No |

**Total: ~17 crates, ~6,000 lines.** This is the kernel. It does not grow.
Adding a stdlib lib = adding a `.hnl` file. Adding a language = adding YAML
data. Adding a target = adding a per-platform `libc.hnl`.

#### Why `plugin-lib-handl` and `plugin-lib-lang-*` cannot be handl

These crates are the **bootstrap** — the circular dependency at the bottom
of the self-hosting stack. They cannot be handl because they *are* the
mechanism that runs handl.

**`plugin-lib-handl`** is the generic `.hnl` loader. It reads a `.hnl`
source, parses `op`/`handl` declarations, builds `HandlModule`s
and `OpFunction`s, and evaluates `(source).field(args)` via a cached
per-thread `Runtime`. You cannot load handl with handl — something must
read the `.hnl` file and feed it to the engine. This is the loader. It is
the kernel's way of turning `.hnl` data into callable modules.

**`plugin-lib-lang-*`** (6 crates: `lang-eval`, `lang-lex`, `lang-parse`,
`lang-compile`, `lang-typecheck`, `lang-run-compiled`) are the meta
functions that let handl code call back into the kernel's own engines:
`eval.eval(lang, ast)`, `lex.lex(lang, text)`, `parse.parse(lang, tokens)`,
`compile.compile(lang, ast, target)`, `typecheck.typecheck(lang, ast)`,
`run_compiled.run_compiled(handle)`. These are the self-hosting bridge —
they let a handl program lex/parse/eval/compile handl. You cannot eval
handl with handl without an evaluator; you cannot lex handl with handl
without a lexer. These crates wrap the Rust engines and expose them as
callable `Function`s.

Both are thin: `plugin-lib-handl` is ~1,000 lines of loader + dispatch
logic. `plugin-lib-lang-*` are ~500 lines each of "extract `Language` from
arg, build sub-runtime, call engine, return result." They contain no
domain logic — they are plumbing between handl values and Rust engine
calls. They are the irreducible bootstrap, not stdlib.

Could they be FFI instead? In principle — expose the engines via a C ABI
and call them via `ffi_bind`. But the engines deal in complex Rust types
(`Language`, `Ast`, `Scheme`, `Tokens`) that don't have stable C
representations. Marshalling these through C would add complexity and
lose type safety for no benefit — these crates are already thin plumbing,
not domain logic. The bootstrap stays Rust.

### 8. What is eliminated

All `plugin-lib-*` crates except `plugin-lib-lang-*`, `plugin-lib-handl`,
and `plugin-lib-prim`:

- `plugin-lib-cmp`, `plugin-lib-collections`,
  `plugin-lib-utils`, `plugin-lib-stats`, `plugin-lib-hello`,
  `plugin-lib-dedent`, `plugin-lib-type`, `plugin-lib-fn`, … (pure-logic —
  already handl, delete Rust wrappers)
- `plugin-lib-os`, `plugin-lib-os-std`, `plugin-lib-fs`, `plugin-lib-net`,
  `plugin-lib-crypto`, `plugin-lib-base64`, `plugin-lib-hex`,
  `plugin-lib-compress`, `plugin-lib-compress-gzip`, `plugin-lib-tls`,
  `plugin-lib-tls-native`, `plugin-lib-http-client`,
  `plugin-lib-http-client-std`, `plugin-lib-http-server`,
  `plugin-lib-http-server-std`, `plugin-lib-ip`, `plugin-lib-url-rfc`,
  `plugin-lib-url-whatwg`, `plugin-lib-archive`, `plugin-lib-archive-tar`,
  `plugin-lib-datetime`, `plugin-lib-rand`, `plugin-lib-unicode`,
  `plugin-lib-intl`, `plugin-lib-otlp`, `plugin-lib-otlp-std`,
  `plugin-lib-ws-*`, `plugin-lib-concurrent`, `plugin-lib-tag`,
  `plugin-lib-serialize`, `plugin-lib-kv`, `plugin-lib-kv-hashmap`,
  `plugin-lib-parse`, `plugin-lib-arrow-iter`, `plugin-lib-test`, …
  (native — replaced by handl over FFI)

Also eliminated:
- `kit-stdlib` Rust wiring (replaced by a data manifest listing `.hnl` files)
- ADR-0051's `plugin-ffi-rust` (never built — superseded by `plugin-ffi`)

**~48 crates, ~44,000 lines of Rust deleted** (of 56 `plugin-lib-*` crates
total; 8 stay — `plugin-lib-handl`, `plugin-lib-prim`, 6 × `plugin-lib-lang-*`).
What remains is the ~6,000-line kernel (~17 crates including `plugin-lib-prim`)
+ N `.hnl` files.

### 9. The async runtime — handl-authored from day one

The async runtime (`plugin-lib-concurrent`, ~6,700 lines) is the one lib where
"handl over FFI" is a real project, not a port. `epoll`/`kqueue` FFI is easy;
the scheduler on top — task spawning, wakers, cancellation, fairness — is a
real runtime.

**Decision: handl-authored scheduler over `epoll` FFI, from day one.**

No `libuv` bridge. No C-ABI async runtime. The scheduler is handl, over
`epoll_create1`/`epoll_ctl`/`epoll_wait` FFI ops. This is the pure path and
the logical conclusion of this ADR — the entire runtime is handl.

Why not the `libuv` short-term path:

- **It's an intermediate layer you'd delete later.** Same argument as the
  Rust impl crates. If the end state is a handl scheduler, building a libuv
  bridge first is debt.
- **libuv's API shapes the abstraction.** If handl async is built over
  libuv's event loop model, the abstraction layer absorbs libuv's design
  (handles, request pools, callbacks). Migrating to a handl-authored
  scheduler later means rethinking the abstraction, not just swapping the
  impl.
- **epoll FFI is simple.** Three syscalls (`epoll_create1`,
  `epoll_ctl`, `epoll_wait`) + `read`/`write` readiness. The complexity is
  in the scheduler logic, which is handl's domain — that's where handl
  shines (arrow graphs, state machines, compositional contracts).

The scheduler is hard but bounded. The core loop is:
1. `epoll_wait` for ready events (FFI)
2. Dispatch each event to the corresponding task's waker (handl)
3. Run ready tasks until they block or complete (handl)
4. Repeat

Task spawning, cancellation, and fairness are handl arrow graphs over this
loop. The contracts (termination, fairness, cancellation safety) are
expressible in handl's axiom system — something Rust cannot offer. This is
where "proofs where Rust can't offer them" (ADR-0077) pays off concretely.

#### Two-level architecture: host scheduler, guest runtimes

The scheduler does not run inside the guest runtime. It runs in a
**host-level runtime** — the language-as-first-class-citizen model
(ADR-0044). The host spawns a `Language` as super-admin, that host runs
the scheduler, and the scheduler evals guest runtimes as tasks:

```
Host runtime (super-admin)
  ├─ scheduler.hnl (handl, runs in host runtime)
  │    ├─ epoll_wait loop (FFI)
  │    ├─ task table (handl state)
  │    └─ evals guest runtimes as tasks
  └─ Guest runtime (user program)
       └─ has access to scheduler ops (spawn, await, yield)
```

This resolves the tree-walk single-threadedness concern: the **host**
controls when guest tasks run, not the guest. Each guest task gets its
own runtime instance. The host scheduler multiplexes them via `epoll`.
The guest sees `spawn`/`await`/`yield` as ops dispatched by the host
scheduler — no cooperative yield needed inside the guest's tree-walk
engine. The host pauses/resumes entire guest runtime instances.

This is cleaner than cooperative scheduling inside a single engine:
- The guest engine stays simple (single-threaded, no yield points)
- The scheduler is at a higher level than the guest (it *owns* the
  guest runtimes)
- Scheduling decisions are made by handl code (the scheduler), not by
  Rust engine internals

#### Suspension mechanism

A synchronous tree-walk engine cannot be paused mid-`Function::call`
without saving its continuation. The two-level architecture avoids this
by making suspension an **op-level protocol**, not an engine feature:

1. **Non-blocking I/O only.** All fds used by async ops are set to
   non-blocking mode (`O_NONBLOCK`). A blocking `read` becomes a
   non-blocking `read` that may return `EAGAIN`.

2. **Async ops return `Suspend`.** When a guest task calls an async op
   (e.g. `net.read_async(fd)`) and the underlying `read` returns
   `EAGAIN`, the op's `Function::call` returns a special `Suspend(waker)`
   value instead of a normal result. The tree-walk engine propagates
   `Suspend` up the call stack as a non-local return — the guest's
   `await` combinator catches it and returns it to the host scheduler.

3. **Host registers the waker with epoll.** The host scheduler receives
   `Suspend(waker)`, calls `epoll_ctl` to register the fd for
   `EPOLLIN`/`EPOLLOUT` readiness with the waker as the event data, and
   moves on to the next ready task.

4. **Host resumes on readiness.** When `epoll_wait` reports the fd is
   ready, the host scheduler calls the waker, which re-enters the guest
   runtime at the task's entry point. The task's state machine (a handl
   arrow graph) resumes from where it left off — the async op is retried,
   succeeds this time, and the task continues.

5. **State machines, not continuations.** The guest task is structured
   as a handl state machine (arrow graph with explicit states). Each
   `Suspend` is a state transition; each resume re-enters at the current
   state. The tree-walk engine runs each state to completion (a normal
   synchronous eval); suspension happens only at async op boundaries,
   between states. No engine-level continuation capture is needed.

This is cooperative scheduling at the **op boundary**, not inside the
engine. The guest engine stays simple — it runs synchronous code. The
async protocol (suspend, register, resume, retry) is handl-authored in
the scheduler and the async op wrappers. The `Suspend` value is a new
`Function::call` return variant (or a sentinel `Value`), not an engine
feature — it flows through the existing dispatch path.

This is a real project — likely the hardest single lib in the migration.
But it is the right investment: it's the one place where handl-authored
logic over FFI is *better* than a Rust implementation, not just equivalent,
because the scheduler's correctness properties are verifiable in handl.

### 10. Kernel freedom

The key decoupling: the stdlib depends only on the **FFI ABI** (the
marshalling types + the `ffi_bind` op), not on
kernel internals. This means:

- The kernel can swap `serde_json::Value` for `TaggedValue` + `Bytes`
  (ADR-0078) without touching the stdlib — the FFI marshalling layer
  translates at the boundary. (ADR-0078 Phase 2 is a prerequisite for
  buffer ops, but once landed, the kernel can further evolve the value
  representation behind the FFI ABI.)
- The kernel can swap cranelift for a new JIT without touching the stdlib —
  `ArrSource::Foreign` dispatch is engine-agnostic.
- The kernel can change the `Module`/`Function` traits without touching the
  stdlib — the stdlib never names these traits; it declares `op`s and `ffi`s.
- The kernel can add a new engine (e.g. a bytecode VM) without touching the
  stdlib — the stdlib is engine-neutral data.

The FFI ABI is the stable contract. The kernel evolves freely behind it.

## Consequences

### Positive

- **~48 Rust crates deleted.** The stdlib is `.hnl` files + the ~6,000-line
  kernel (~17 crates). Adding a lib = adding a `.hnl` file. No Rust edit.
- **Kernel is free to evolve.** The stdlib depends on the FFI ABI, not kernel
  internals. Trait changes, engine swaps, value representation changes —
  none break the stdlib.
- **No intermediate Rust layer to delete later.** Building the end state
  directly avoids the ADR-0077 Phase 4 → ADR-0083 double migration.
- **C ABIs are more stable than Rust crates.** POSIX, OpenSSL, zlib have
  decades-stable ABIs. Rust ecosystem crates churn every release.
- **Compositional proofs end-to-end.** The handl abstraction layer carries
  axioms (ADR-0077 Phase 3); FFI ops carry trusted axioms enforced at the
  boundary. The correctness story is entirely in handl.
- **ADR-0044 taken to its conclusion.** "Language as data" extends to the
  stdlib: a new native lib is a data change (`.hnl` + `ffi_bind`), never a
  kernel recompile.

### Negative / trade-offs

- **FFI marshalling has overhead.** Each `op` call marshals handl values to
  C types and back. Under cranelift JIT, this is one native call + marshal;
  under tree-walk, it's slower. The abstraction layer adds N FFI calls per
  high-level operation (e.g. `fs.read` = `open` + `read` + `close` = 3 FFI
  calls). Acceptable for I/O-bound ops; would need care for compute-bound
  ops (crypto hashing in a hot loop — prefer FFI to a single `SHA256` call,
  not composing over per-block FFI).
- **The FFI dispatcher is new Rust to build.** ~1 crate, ~500–1000 lines.
  This is the enabling piece; everything else is handl.
- **Marshalling edge cases.** Struct returns (`out_struct` with
  `#[repr(C)]` layout), variadic functions (`...` descriptor), conditional
  args (`?` suffix), platform differences, complex return types. The
  descriptor system handles the common cases; the last 10% needs per-op
  handl wrappers that decompose calls into supported descriptors, or
  per-platform `.hnl` variants. Bounded but real work.
- **Async runtime is the hardest lib.** A handl-authored scheduler over
  `epoll` FFI is a real project — likely the single hardest piece of the
  migration. No `libuv` bridge is built (it would be intermediate debt).
  The payoff is a scheduler with verifiable correctness properties (fairness,
  termination, cancellation safety) — something no Rust scheduler offers.
- **`op` axioms for FFI ops are trusted.** The C body is assumed to honor
  the declared `guarantee`/`requires`. A bug in libc/OpenSSL/zlib that
  violates the axiom is not caught by the proof system — only by runtime
  checking (ADR-0077 Phase 3) or testing. This is the same trust boundary
  as ADR-0077 Phase 4 Rust bodies; FFI doesn't make it worse.
- **Debugging across the FFI boundary is harder.** A crash in a C function
  called via FFI is a native crash, not a handl eval error. The dispatcher
  should catch segfaults where feasible (signal handler on Unix, SEH on
  Windows) and convert them to `EvalError`. Not always possible; documented
  as a known limitation.

### Compatibility

- **ADR-0077 Phases 1–3, 5–6 remain valid.** Pure-logic libs via
  `HandlModule`, `op`/`handl` declarations, axiom enforcement, capability
  declarations, importable modules — all unchanged.
- **ADR-0077 Phase 4 is refined.** Native libs migrate to `ffi_bind`
  bindings, not Rust `Function` bodies. The `OpRegistry` + `OpFunction`
  dispatch path is unchanged; `FfiOpFunction` is a new `Function` impl.
- **ADR-0051 is superseded.** The Rust closure binding façade is not built.
  The `plugin-ffi` dispatcher replaces it — bindings are data (`ffi_bind`
  calls), not Rust closures.
- **Already-migrated crates (prim, cmp, stats, hello, collections, …) are
  unaffected.** Their `op`/`handl` declarations stay; only the `handl "x" =
  rust_x` bindings that point at Rust bodies change to `ffi_bind` or pure
  handl arrow graphs.
- **The `handl "x" = rust_x` form is not removed.** It remains for the
  `plugin-lib-lang-*` meta functions (eval, lex, parse, compile, typecheck)
  which are the self-hosting bridge — these genuinely need Rust bodies
  because they call into the kernel's own engines. `ffi_bind` is a new
  sibling form, not a replacement for all `rust_x` bindings.

## Rationale

ADR-0077 Phase 4 proposed keeping Rust `Function` bodies for native libs.
This is an intermediate layer: the Rust wraps C (POSIX/OpenSSL/zlib), and
the handl wraps the Rust. The Rust adds boilerplate, churn risk, and kernel
coupling without adding battle-testing (the C underneath is already
battle-tested) or correctness (Rust offers no proof mechanism; the axioms
live in handl either way).

Going direct FFI eliminates the intermediate layer. The stdlib is handl
over C. The kernel is Rust (engines + FFI dispatcher). The boundary is the
FFI ABI — stable, minimal, and decoupled from kernel internals.

This is ADR-0044 ("language as data") taken to its conclusion for the
stdlib: a new native lib is a `.hnl` file with `ffi_bind` calls + an
arrow graph, never a Rust crate. The kernel stops growing.

## Related ADRs

- [ADR-0077 — Stdlib Migration to Handl](./0077-stdlib-migration-to-handl.md)
  — this ADR refines Phase 4 (FFI instead of Rust bodies) and keeps
  Phases 1–3, 5–6.
- [ADR-0051 — `ffi-rust` Rust Binding Façade](./0051-ffi-rust-binding-facade.md)
  — superseded. The Rust closure binding layer is replaced by direct FFI
  data bindings.
- [ADR-0044 — Language as Data](./0044-language-as-data-and-runtime-split.md)
  — this ADR extends "language as data" to the stdlib.
- [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) — the
  calculus vocabulary that stays Rust.
- [ADR-0078 — Value Types as Data](./0078-value-types-as-data.md) —
  **prerequisite.** ADR-0078 Phase 2 (`TaggedValue` + `Bytes` type) is
  required for ADR-0083 Phase 1 (buffer FFI ops). The value
  representation can change behind the FFI marshalling layer without
  breaking the stdlib.
- [ADR-0084 — Arrow Macros](./0084-arrow-macros.md) —
  `prim.or` (§5.1) is a degenerate macro. ADR-0084 defines the full
  type-safe macro system that subsumes it.

## Appendix: Migration map by tier

### Tier 1 — Pure handl (already done or trivial)

No FFI needed. Arrow graphs over `prim.*` or other handl libs.

| Lib | Status |
|---|---|
| `prim` | Migrated (atoms — see Tier 3 for FFI version) |
| `fn` | Pure handl (already `.hnl`) |
| `cmp` | Migrated (`op`/`handl`, Rust bodies → pure handl) |
| `collections` | Migrated (partially — finish pure-handl port) |
| `stats` | Migrated (`HandlModule`) |
| `hello` | Migrated (`HandlModule`) |
| `dedent` | Pure handl (string manipulation) |
| `type` | Pure handl logic, calls kernel meta ops (`type_of`) via `plugin-lib-lang-*` |
| `utils` | Mixed — pure-handl parts done, finish the rest |
| `tag` | Pure handl (tagging system) |
| `serialize` | Pure handl (over `json`/`yaml`/`toml`) |
| `kv` / `kv-hashmap` | Pure handl (in-memory store) |
| `parse` | Pure handl (parser combinators) |
| `arrow-iter` | Pure handl (arrow iteration) |
| `test` | Pure handl logic, calls kernel meta ops (`compile`, `run_compiled`) via `plugin-lib-lang-*` for compiled-code execution |

### Tier 2 — Handl over FFI (direct C/POSIX bindings)

FFI data bindings + handl arrow graph abstraction layer.

| Lib | FFI target | Difficulty |
|---|---|---|
| `fs` | POSIX (`open`/`read`/`write`/`close`/`stat`) | Easy |
| `os` | POSIX (`getenv`/`setenv`/`exec`/`fork`/`wait`/`time`) | Easy |
| `net` | POSIX (`socket`/`bind`/`listen`/`accept`/`connect`/`recv`/`send`) | Easy |
| `base64` | Pure handl or FFI to `libbase64` | Trivial |
| `hex` | Pure handl | Trivial |
| `encoding` | Over `base64`/`hex` | Trivial |
| `crypto` | OpenSSL / libsodium C ABI | Easy–Medium |
| `compress` | zlib / libdeflate C ABI | Easy |
| `compress-gzip` | zlib gzip C ABI | Easy |
| `tls` | OpenSSL C ABI (not rustls) | Medium |
| `ip` | Pure handl (parsing/logic) or `libc` inet ops | Easy |
| `url` | Pure handl (RFC parsing) | Easy |
| `archive` / `archive-tar` | libarchive C ABI or pure handl | Medium |
| `datetime` | `libc` time FFI + handl calendar logic | Medium |
| `rand` | `getrandom(2)` FFI + handl distributions | Easy |
| `unicode` | ICU C ABI or pure handl tables | Medium |
| `intl` | ICU C ABI | Medium |
| `json` | Pure handl parser (or FFI to `libjq` for perf) | Easy |
| `yaml` | libyaml C ABI | Medium |
| `toml` | Pure handl parser | Easy |
| `http` | HTTP/1.1 over `net` (handl framing); HTTP/2 harder | Medium |
| `otlp` | Over `http` + `proto` (handl) | Medium |
| `ws` | WebSocket framing over `net` (handl) + `tls` | Medium |

### Tier 3 — Atoms (kernel primitives, not stdlib)

| Lib | Path |
|---|---|
| `prim` | `prim.add` / `prim.compare` / `prim.alloc` / `prim.or` — calculus atoms (ADR-0052). Integer addition and bitwise OR are CPU instructions, not C functions — cannot be FFI. Stays Rust in `plugin-lib-prim` as kernel vocabulary. |

### Tier 4 — Async runtime (handl-authored, hardest lib)

| Lib | Path |
|---|---|
| `concurrent` | Handl-authored scheduler over `epoll` FFI ops. No `libuv` bridge. Core loop: `epoll_wait` → dispatch to wakers → run ready tasks → repeat. Contracts (fairness, termination, cancellation) verifiable in handl's axiom system. |

### Tier 5 — Kernel (stays Rust, never migrates)

| Crate | Role |
|---|---|
| `handl-language` | `Language`/`Ast`/`Scheme`/`Value` data model |
| `handl-runtime` | Engine binding, staged execution |
| `handl-value` | Value representations |
| `platform` | `Plugin`/`Module`/`Function` traits |
| `plugin-module` | `ModuleRuntime`/`ModuleRule`/`ModuleRegistry` |
| `plugin-engine-*` (7 crates) | Lexer/parser/eval/typecheck engines |
| `plugin-lib-handl` | Generic `.hnl` loader |
| `plugin-ffi` | FFI dispatcher (this ADR) |
| `plugin-loader-native` | Dynamic library loading |
| `plugin-lib-prim` | Calculus atoms (`add`, `compare`, `alloc`, `or`) |
| `plugin-lib-lang-*` (6 crates) | Meta functions (self-hosting bridge) |

## Appendix: Execution plan

Six phases, each independently shippable. Linux is the initial target;
other platforms are future work (§6). The platform layer is abstracted
from day one — the abstraction layer (`fs.hnl`, `net.hnl`, …) imports
from a platform-selected `libc.hnl` and is platform-neutral.

### Phase 0 — FFI dispatcher (the enabling piece, ~1 Rust crate)

The only new Rust. Everything else is handl.

1. Build `plugin-ffi` crate — `FfiOpFunction` implementing `Function`:
   library cache (`dlopen` once), symbol cache (`dlsym` once),
   marshalling (`"i32"`/`"i64"`/`"f64"`/`"cstr"`/`"in_buf"`/`"out_buf"`/
   `"cbytes"`/`"ptr"`/`"void"`/`"handle"`/`"out_struct"`),
   error clause (errno extraction), resource table for `handle`-typed
   opaque pointers.
2. Register `ffi_bind` as a host op (a `Function` impl in
   `plugin-ffi`) that takes library, symbol, descriptor list, return
   descriptor, and optional error clause, and returns an `FfiOpFunction`.
   No lexer/parser changes — uses existing surface syntax
   (juxtaposition, arrays, records).
3. Extend `handl` RHS to accept surface expressions (not just Rust
   identifiers) — evaluate at load time, register the resulting
   `Function` in `OpRegistry`.
4. Add `prim.or` to `plugin-lib-prim` — bitwise OR for flag composition.
5. Wire `FfiOpFunction` into `OpRegistry` — no engine changes, same
   `OpFunction` dispatch path.
6. Prove it — `libc.getpid` via `ffi_bind`. No Rust body, no
   `plugin-lib-*` crate.
7. **Testing strategy.** FFI ops are tested via:
   - **Real syscalls on Linux CI** — `libc.getpid`, `libc.open` +
     `libc.read` + `libc.close` on a temp file, `libc.stat` on a known
     path. Assertions check return values and errno on failure paths
     (e.g. open a nonexistent file → `Err(ENOENT)`).
   - **Marshal error tests** — overflow (`i32` with `2^31`), embedded
     NUL (`cstr` with `"\0"`), type mismatch (passing `Bytes` to `cstr`
     descriptor — should be a compile error, not runtime).
   - **Platform-specific expectations** — errno values, struct layouts,
     constant values are platform-dependent. Tests are gated by
     `cfg!(target_os)` and only run on the matching platform. CI runs
     Linux tests; macOS/Windows tests are future work.
   - **No mock C libs.** Mocking C libraries adds complexity without
     testing the real FFI path. Real syscalls are the test surface.

**Done when:** a handl program can `ffi_bind "libc" "getpid" [] "i32"` and
get a PID back. Simple types only (`i32`, `cstr`) — buffer ops need
`Bytes` (Phase 0.5).

### Phase 0.5 — `Bytes` type (prerequisite for Phase 1)

ADR-0078 Phase 2 introduces `TaggedValue` as the native representation.
This phase adds the `Bytes` type to ADR-0078:

1. Declare `Bytes` in `value_types.yaml` (scalar, `Rc<Vec<u8>>` layout).
2. Add `TaggedValue::Bytes` variant to `handl-value`.
3. Wire `Bytes` into the FFI marshalling layer (`in_buf`/`out_buf`/`cbytes`).

**Done when:** a handl program can `ffi_bind "libc" "read" ["i32", "out_buf:1024"] "i64"` and get bytes back as a `Bytes` value (in the `buf` field of the result record), not a `Value::Array`.

### Phase 1 — POSIX layer (`stdlib/libc/linux.hnl`)

Raw Linux syscall/POSIX surface as `ffi_bind` calls + regular
bindings for constants:

`open`/`openat`/`read`/`write`/`close`/`stat`/`fstat`/`lseek`/`mmap`/
`munmap`/`getpid`/`getenv`/`setenv`/`fork`/`exec`/`wait`/`kill`/`signal`/
`socket`/`bind`/`listen`/`accept`/`connect`/`recv`/`send`/
`epoll_create1`/`epoll_ctl`/`epoll_wait`/`__errno_location`.

Constants: `O_RDONLY`/`O_WRONLY`/`O_RDWR`/`O_CREAT`/`O_TRUNC`/`O_CLOEXEC`/
`SIGHUP`/`SIGINT`/`SIGTERM`/`AF_INET`/`AF_UNIX`/`EPOLLIN`/`EPOLLOUT`/etc.

Other platforms (`stdlib/libc/darwin.hnl`, `stdlib/libc/windows.hnl`) are
future work. The abstraction layer imports from the platform-selected
`libc.hnl` and is platform-neutral — adding a platform is a new `.hnl`
file, not a kernel recompile.

**Done when:** `stdlib/libc/linux.hnl` covers the POSIX surface needed by
Tier 2 libs. All ops are `ffi_bind` calls. All constants are
regular bindings. No Rust.

### Phase 2 — Stdlib port (in dependency order)

Each lib is a `.hnl` file. Delete the corresponding `plugin-lib-*` Rust
crate after its `.hnl` replacement passes the crate's existing tests.

**Easy wins (prove the pattern):**

1. `stdlib/fs/fs.hnl` → delete `plugin-lib-os-std` (fs) + `plugin-lib-os` (fs)
2. `stdlib/os/os.hnl` → delete rest of `plugin-lib-os` + `plugin-lib-os-std`
3. `stdlib/net/net.hnl` → delete `plugin-lib-net` (if separate)
4. `stdlib/base64/base64.hnl` / `stdlib/hex/hex.hnl` → delete `plugin-lib-base64` + `plugin-lib-hex`
5. `stdlib/crypto/crypto.hnl` (OpenSSL FFI) → delete `plugin-lib-crypto`
6. `stdlib/compress/compress.hnl` / `stdlib/compress/gzip.hnl` (zlib FFI) →
   delete `plugin-lib-compress` + `plugin-lib-compress-gzip`

**Medium:**

7. `stdlib/url/url.hnl` / `stdlib/ip/ip.hnl` (pure handl) → delete `plugin-lib-url-*` + `plugin-lib-ip`
8. `stdlib/datetime/datetime.hnl` (`libc` time FFI + handl logic) → delete `plugin-lib-datetime`
9. `stdlib/rand/rand.hnl` (`getrandom` FFI) → delete `plugin-lib-rand`
10. `stdlib/json/json.hnl` / `stdlib/toml/toml.hnl` (pure handl) /
    `stdlib/yaml/yaml.hnl` (libyaml FFI) → delete `plugin-lib-json` +
    `plugin-lib-toml` + `plugin-lib-yaml`
11. `stdlib/unicode/unicode.hnl` / `stdlib/intl/intl.hnl` (ICU FFI) →
    delete `plugin-lib-unicode` + `plugin-lib-intl`
12. `stdlib/archive/archive.hnl` / `stdlib/archive/tar.hnl` → delete
    `plugin-lib-archive` + `plugin-lib-archive-tar`
13. `stdlib/tls/tls.hnl` (OpenSSL FFI) → delete `plugin-lib-tls` + `plugin-lib-tls-native`

**Harder:**

14. `stdlib/http/http.hnl` (HTTP/1.1 over `net`) → delete `plugin-lib-http-client*` + `plugin-lib-http-server*`
15. `stdlib/ws/ws.hnl` (WebSocket over `net` + `tls`) → delete `plugin-lib-ws-*`
16. `stdlib/otlp/otlp.hnl` (over `http`) → delete `plugin-lib-otlp` + `plugin-lib-otlp-std`

**Pure-logic cleanup (finish ADR-0077 Phase 1):**

17. Finish `stdlib/collections/`, `stdlib/utils/`, `stdlib/cmp/`,
    `stdlib/tag/`, `stdlib/kv/`, `stdlib/parse/`, `stdlib/arrow-iter/`,
    `stdlib/test/`, `stdlib/dedent/`, `stdlib/type/`. Delete remaining
    Rust wrappers. Migrate from `handl-packages/` to `stdlib/`.

**Done when:** all Tier 1 + Tier 2 libs are `.hnl` files. All
`plugin-lib-*` Rust crates deleted except `plugin-lib-lang-*` and
`plugin-lib-handl`.

### Phase 3 — Kit wiring as data

Replace `kit-stdlib`'s hand-listed `register_plugin(...)` calls with a
data manifest (YAML) at `stdlib/stdlib.yaml`. A single generic registrar
reads the manifest, loads each `.hnl` from `stdlib/{pkg-name}/`, wires FFI
libraries, registers modules. Delete `kit-stdlib` Rust crate.

**Done when:** `kit-stdlib` is a YAML file, not a Rust crate.

### Phase 4 — Async runtime (handl-authored scheduler)

Handl-authored scheduler over `epoll` FFI ops (§9), under
`stdlib/concurrent/`. Core loop: `epoll_wait` → dispatch to wakers → run
ready tasks → repeat. Task spawning, cancellation, fairness as handl
arrow graphs. Contracts verifiable in handl's axiom system. Delete
`plugin-lib-concurrent`.

**Prerequisite: finalizable values (§2.2).** The async runtime is a
long-lived server process. Shutdown-only finalizers are insufficient —
fd leaks from task cancellation accumulate. Phase 4 requires
finalizable `TaggedValue::Resource` so that cancelled tasks release
their fds automatically via RAII. This is an extension to ADR-0078
(`handl-value`), not a new crate.

This is the hardest single lib. It may run in parallel with Phase 2 (it
only depends on `stdlib/libc/linux.hnl` from Phase 1 + finalizable
values, not on other Tier 2 libs).

**Done when:** `plugin-lib-concurrent` is deleted. Async is
handl-over-epoll.

### Phase 5 — Kernel stabilization

Document and test the stable contracts the stdlib depends on:

1. **FFI ABI** — marshalling types + `ffi_bind` op. Versioned.
2. **`op`/`handl` declaration form** — stable since ADR-0077.
3. **Calculus vocabulary** — combinator set, primitive forms, value shapes.

Everything else (engine internals, trait shapes, value representation, JIT
implementation) is free to change behind these contracts.

**Done when:** the kernel has a documented stable boundary and a test
suite verifying the stdlib works against it without coupling to internals.

### Summary

| Phase | What | Rust deleted | New Rust |
|---|---|---|---|
| 0 | FFI dispatcher + `ffi_bind` + `prim.or` | 0 | ~1 crate (`plugin-ffi`) + 1 atom (`prim.or`) |
| 0.5 | `Bytes` type (ADR-0078 Phase 2) | 0 | 0 (extends `handl-value`) |
| 1 | `stdlib/libc/linux.hnl` (POSIX FFI ops + constants) | 0 | 0 |
| 2 | Stdlib port to `stdlib/{pkg}/` (Tier 1 + 2) | ~48 crates | 0 |
| 3 | Kit wiring as `stdlib/stdlib.yaml` manifest | 1 crate (`kit-stdlib`) | 0 |
| 4 | Async runtime (`stdlib/concurrent/`, host-level scheduler over epoll) | 1 crate (`concurrent`) | 0 (requires finalizable values in `handl-value`) |
| 5 | Kernel stabilization | 0 | 0 (documentation) |

**End state:** ~17 Rust crates (the kernel, ~6,000 lines, does not grow) +
N `.hnl` files (the entire stdlib). Adding a lib = adding a `.hnl` file.
The kernel is free to evolve behind the FFI ABI.
