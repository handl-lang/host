# ADR-0083: FFI-Native Stdlib — Handl over Direct C/POSIX Bindings

## Status

Proposed. Supersedes [ADR-0051 — `ffi-rust` Rust Binding
Façade](./0051-ffi-rust-binding-facade.md) (Proposed, never built — the Rust
closure binding layer is eliminated in favor of direct FFI data bindings).
Refines [ADR-0077 — Stdlib Migration to
Handl](./0077-stdlib-migration-to-handl.md) Phase 4: native libs migrate to
handl over **direct FFI bindings**, not Rust `Function` bodies in `OpRegistry`.
ADR-0077 Phases 1–3, 5–6 remain valid (pure-logic libs, axioms, capabilities,
imports). Builds on [ADR-0044 — Language as
Data](./0044-language-as-data-and-runtime-split.md) and [ADR-0052 — Arrow
Calculus Kernel](./0052-arrow-calculus-kernel.md).

## Context

### Where we are

ADR-0077 established the `op`/`handl` declaration form and the `OpRegistry` +
`HandlLibraryPlugin` mechanism. ~50 of ~60 `plugin-lib-*` crates already wire
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
  identifier; this ADR adds an `ffi(...)` form.
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
  code; each `op` is one native call. The abstraction's cost collapses to
  the dispatch boundary under JIT.
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

### 1. The `ffi(...)` binding declaration

Add a new `handl` binding form that declares an op as a direct FFI call:

```handl
--- libc.open — open a file, return file descriptor (or -1 on error).
op "libc.open" : (Path, Flags) -> Fd
  guarantee: Or(Ge(output, 0), IsError)
handl "libc.open" = ffi("libc", "open", [cstr, i32] -> i32)
```

The `ffi(...)` form specifies:
- **Library name** — resolved by the dynamic loader (`dlopen`/`LoadLibrary`).
  May be a concrete name (`"libc.so.6"`) or an alias resolved by the host
  (`"libc"` → platform-specific path).
- **Symbol name** — the C symbol (`dlsym`/`GetProcAddress`).
- **Argument marshalling types** — one per argument.
- **Return marshalling type** — the C return type.

### 2. Marshalling types

A fixed, small set of marshalling descriptors covers the vast majority of C
ABIs:

| Descriptor | C type | handl value | Marshalling |
|---|---|---|---|
| `i32` | `int32_t` | `Number` | truncate to i32, pass by value |
| `i64` | `int64_t` | `Number` | truncate to i64, pass by value |
| `f64` | `double` | `Number` | pass by value |
| `cstr` | `const char*` | `String` | null-terminate, pass pointer |
| `buf` | `void* + size_t` | `Array<Byte>` | (ptr, len) pair, two C args |
| `ptr` | `void*` | opaque `Handle` | pass through raw pointer |
| `void` | `void` | `Unit` | no return value |
| `handle` | `void*` | opaque `Handle` | wrap in resource table entry |

`buf` is a two-arg descriptor — it consumes **two** C arguments (pointer +
length) from **one** handl argument. This is the common pattern for
read/write-style functions.

`handle` wraps a raw C pointer in an opaque handl value tracked by a
per-runtime resource table. The table enables cleanup (finalizer callbacks)
and prevents handl code from dereferencing arbitrary pointers. POSIX file
descriptors are plain `i32`/`i64` and do not need `handle` — they pass
through directly.

### 3. Error handling

POSIX convention: return value `< 0` (or `== -1`) signals error; `errno`
holds the detail. The `ffi(...)` form supports an optional error clause:

```handl
handl "libc.open" = ffi("libc", "open", [cstr, i32] -> i32)
  error: { on: Lt(ret, 0), errno: true }
```

When the error condition matches, the FFI dispatcher extracts `errno`
(via `__errno_location` on Linux, `errno` global on macOS, `GetLastError`
on Windows) and returns an `Err(errno)` handl value instead of the raw
return. Without an error clause, the raw return value passes through.

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

### 5. The abstraction layer in handl

Native libs become handl arrow graphs over raw FFI ops:

```handl
--- fs.read — read a file as bytes.
--- Contract: Or(IsOk(output), IsError(output))
fs.read = path ~>
  fd = libc.open(path, O_RDONLY)
  if fd < 0 then
    Err(libc.errno())
  else {
    buf = prim.alloc(BUF_SIZE)
    n = libc.read(fd, buf, BUF_SIZE)
    libc.close(fd)
    if n < 0 then Err(libc.errno())
    else Ok(buf)
  }
```

The abstraction layer (error handling, retry logic, buffering, encoding) is
handl-authored. The leaf operations (`open`, `read`, `write`, `close`) are
FFI data bindings. No Rust bodies.

### 6. Platform variants

POSIX differs across platforms (`epoll` vs `kqueue` vs IOCP, `__errno_location`
vs `errno` global, `open` vs `openat`). The interface/impl split from
ADR-0077 §"Multi-target support" applies, but the "impl" is now a per-platform
`.hnl` file, not a Rust crate:

```
libc_linux.hnl   — epoll, __errno_location, openat
libc_darwin.hnl  — kqueue, errno global, open
libc_windows.hnl — IOCP, GetLastError, CreateFileW
```

The host selects the platform `.hnl` based on `cfg!(target_os)`. The
abstraction layer (`fs.hnl`, `net.hnl`, …) imports from the platform-selected
`libc.hnl` and is platform-neutral.

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
| `plugin-lib-lang-*` (6 crates) | Meta functions (eval/lex/parse/compile/typecheck/run_compiled) — the self-hosting bridge | No |

**Total: ~16 crates, ~6,000 lines.** This is the kernel. It does not grow.
Adding a stdlib lib = adding a `.hnl` file. Adding a language = adding YAML
data. Adding a target = adding a per-platform `libc.hnl`.

### 8. What is eliminated

All `plugin-lib-*` crates except `plugin-lib-lang-*` and `plugin-lib-handl`:

- `plugin-lib-prim`, `plugin-lib-cmp`, `plugin-lib-collections`,
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

**~44 crates, ~44,000 lines of Rust deleted.** What remains is the ~6,000-line
kernel + N `.hnl` files.

### 9. The async runtime (known hard problem)

The async runtime (`plugin-lib-concurrent`, 6,622 lines) is the one lib where
"handl over FFI" is a real project, not a port. `epoll`/`kqueue` FFI is easy;
the scheduler on top — task spawning, wakers, cancellation, fairness — is a
real runtime.

Two paths:

- **Short-term:** FFI to a C-ABI async runtime (`libuv`, or a small C-ABI
  wrapper around `epoll`). The scheduler stays C; handl authors async arrows
  over it. No Rust crate in the workspace.
- **Long-term:** handl-authored scheduler over `epoll`/`kqueue` FFI ops. This
  is the pure path — the entire runtime is handl. It is a multi-month project
  with subtle concurrency hazards, but it is the logical conclusion of this
  ADR.

The ADR does not mandate either path. The short-term path is compatible with
the long-term path (both are FFI, not Rust crates). The decision can be made
when the rest of the stdlib is migrated and the async runtime is the last
Rust-free target.

### 10. Kernel freedom

The key decoupling: the stdlib depends only on the **FFI ABI** (the
marshalling types + the `op`/`handl`/`ffi(...)` declaration form), not on
kernel internals. This means:

- The kernel can swap `serde_json::Value` for a tagged-value representation
  (ADR-0078) without touching the stdlib — the FFI marshalling layer
  translates at the boundary.
- The kernel can swap cranelift for a new JIT without touching the stdlib —
  `ArrSource::Foreign` dispatch is engine-agnostic.
- The kernel can change the `Module`/`Function` traits without touching the
  stdlib — the stdlib never names these traits; it declares `op`s and `ffi`s.
- The kernel can add a new engine (e.g. a bytecode VM) without touching the
  stdlib — the stdlib is engine-neutral data.

The FFI ABI is the stable contract. The kernel evolves freely behind it.

## Consequences

### Positive

- **~44 Rust crates deleted.** The stdlib is `.hnl` files + the ~6,000-line
  kernel. Adding a lib = adding a `.hnl` file. No Rust edit.
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
  stdlib: a new native lib is a data change (`.hnl` + `ffi(...)`), never a
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
- **Marshalling edge cases.** Struct returns, variadic functions, platform
  differences, complex return types (e.g. `stat` fills a struct via pointer
  arg). The descriptor system handles 90%; the last 10% needs per-op escape
  hatches or per-platform `.hnl` variants. Bounded but real work.
- **Async runtime is hard.** The short-term FFI-to-libuv path is pragmatic;
  the long-term handl-authored scheduler is a project. This is the one lib
  where "handl over FFI" is genuinely difficult, not just porting.
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
- **ADR-0077 Phase 4 is refined.** Native libs migrate to `ffi(...)`
  bindings, not Rust `Function` bodies. The `OpRegistry` + `OpFunction`
  dispatch path is unchanged; `FfiOpFunction` is a new `Function` impl.
- **ADR-0051 is superseded.** The Rust closure binding façade is not built.
  The `plugin-ffi` dispatcher replaces it — bindings are data (`ffi(...)`
  declarations), not Rust closures.
- **Already-migrated crates (prim, cmp, stats, hello, collections, …) are
  unaffected.** Their `op`/`handl` declarations stay; only the `handl "x" =
  rust_x` bindings that point at Rust bodies change to `ffi(...)` or pure
  handl arrow graphs.
- **The `handl "x" = rust_x` form is not removed.** It remains for the
  `plugin-lib-lang-*` meta functions (eval, lex, parse, compile, typecheck)
  which are the self-hosting bridge — these genuinely need Rust bodies
  because they call into the kernel's own engines. `ffi(...)` is a new
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
stdlib: a new native lib is a `.hnl` file with `ffi(...)` declarations + an
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
- [ADR-0078 — Value Types as Data](./0078-value-types-as-data.md) — the
  value representation can change behind the FFI marshalling layer without
  breaking the stdlib.

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
| `type` | Pure handl (type introspection — may need kernel meta ops) |
| `utils` | Mixed — pure-handl parts done, finish the rest |
| `tag` | Pure handl (tagging system) |
| `serialize` | Pure handl (over `json`/`yaml`/`toml`) |
| `kv` / `kv-hashmap` | Pure handl (in-memory store) |
| `parse` | Pure handl (parser combinators) |
| `arrow-iter` | Pure handl (arrow iteration) |
| `test` | Pure handl (test framework + kernel meta hooks) |

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

### Tier 3 — Atoms (FFI or kernel primitives)

| Lib | Path |
|---|---|
| `prim` | `prim.add` / `prim.compare` / `prim.alloc` — these are the calculus atoms. Either keep as `rust_x` (kernel primitive) or express as `ffi` to `libc` math ops. Decision: keep as kernel primitives (they are the calculus vocabulary, not stdlib). |

### Tier 4 — Async runtime (hard — deliberate decision required)

| Lib | Path |
|---|---|
| `concurrent` | Short-term: FFI to `libuv` or small C-ABI executor. Long-term: handl-authored scheduler over `epoll`/`kqueue` FFI. |

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
| `plugin-lib-lang-*` (6 crates) | Meta functions (self-hosting bridge) |
