# ADR-0107: Handl SDK Generator — Port-Adapter Foreign SDK Generation

## Status

Accepted.

Depends on the runtime-injected `ffi` package (`call`/`bind`/`errno`),
[ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md),
[ADR-0090 — Bootstrap CLI](./0090-bootstrap-cli.md), and the `export` /
`export ... :=` port-adapter pattern used by `handl-packages/time`.

## Context

Handl already calls C code at runtime through the `ffi` package
(`crates/plugin-ffi/src/lib.rs`) and the `export` / `export ... :=` pattern
already separates *ports* from *adapters* in the stdlib
(`handl-packages/time/index.hnl` is the port, `handl-packages/time-c` is the
libc adapter).

The missing piece is an **authoring tool** that turns a foreign API description
(C headers, WebIDL, JVM signatures, etc.) into the same port-adapter Handl
source automatically. The first backend is **C native**; the architecture must
be a true port-adapter plugin system so later backends (JS browser APIs, JVM,
etc.) are just new `SdkgenBackend` implementations.

This ADR intentionally scopes the first concrete backend to **C / libclang**.
Everything else is left as an extension point.

## Decision

### 1. Port-adapter model

The generator produces **two files per foreign module**, mirroring the existing
`time` / `time-c` pattern:

- **Port** (`<lib>/<module>.hnl`) — abstract `export` arrows, types, and
  grades. This is the contract. It is platform- and backend-agnostic.
- **Adapter** (`<lib>-c/<target>/<module>.hnl`) — concrete `export ... :=`
  arrows that implement the port by calling the foreign ABI.

This means no `op`/`handl` declarations, no `ffi_bind` macro, and no `Handle`
abstraction. The runtime interface is the existing `ffi.call` and `ffi.bind`
primitives in `plugin-ffi`.

### 2. New crates

| Crate | Responsibility |
| --- | --- |
| `crates/plugin-sdkgen` | Core generator. Defines the `SdkgenModule` IR, the `SdkgenBackend` trait, the abstract `Emitter` trait, and the public `generate` API. |
| `crates/plugin-sdkgen-backend-c` | C backend. Parses C headers with libclang, computes target ABI, and produces `SdkgenModule` records. Build/authoring tool only. |
| `crates/plugin-sdkgen-emitter-file` | File emitter. Writes the IR to `.hnl` files on disk. |
| `crates/plugin-ext-sdkgen` | CLI extension. Wires `hnl sdkgen` into `host-cli`. |

Package names follow the existing convention:
`handl-plugin-sdkgen`, `handl-plugin-sdkgen-backend-c`, `handl-plugin-sdkgen-emitter-file`, `handl-plugin-ext-sdkgen`.

### 3. Multi-backend architecture

```text
foreign description
       │
       ▼
plugin-sdkgen-backend-*   (libclang for C, WebIDL parser for JS, etc.)
       │
       ▼
  SdkgenModule            (platform-agnostic IR)
       │
       ▼
plugin-sdkgen             (emit + name/grade/type heuristics)
       │
       ▼
plugin-sdkgen-emitter-*   (file, memory, LSP, cache)
       │
       ▼
  handl-packages/generated/<lib>/<module>.hnl                   (port — abstract exports)
  handl-packages/generated/<lib>-<backend>-<target>/<module>.hnl (adapter — concrete exports)
```

The `SdkgenBackend` trait is backend-specific. The `Emitter` trait is
output-channel-specific. The first `Emitter` implementation is
`plugin-sdkgen-emitter-file`; later emitters (memory, LSP, cache) can be added
without touching any backend.

### 3.1 Three dimensions of extensibility

`SdkgenModule` is not just backend-agnostic; it is also target-agnostic and
client-agnostic. This is what lets the same core support Windows, macOS, CUDA,
WebIDL, etc.:

- **Backend** — the foreign API description being parsed (C headers, WebIDL,
  JNI, CUDA headers, etc.). Each backend is a new `plugin-sdkgen-backend-*`
  crate.
- **Target** — the ABI the generated adapter must satisfy
  (`x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`,
  `nvptx64-nvidia-cuda`, `xtensa-esp32-elf`, `riscv32-esp-elf`, etc.). The
  target triple drives libclang or the backend's ABI analysis.
- **Foreign client** — the Handl runtime primitive family that actually
  executes the call. `ffi` is the first client (`ffi.call`, `ffi.errno()`).
  Windows `kernel32` may use a `win32` client (`win32.call`,
  `win32.get_last_error()`); CUDA uses a `cuda` client (`cuda.launch`,
  `cuda.get_error_string()`); JS uses a `js` client; JVM uses a `jvm` client.

The `SdkgenBackend` produces `SdkgenModule` records that include the target
triple and the chosen client. The `Emitter` then emits imports from the right
client package and calls the right runtime primitives. The output package name
encodes both backend and target: `<lib>-<backend>-<target>`.

### 4. C backend uses libclang, not a hand-rolled parser

C headers do not contain enough information to determine memory layout
portably. The C backend therefore relies on **libclang / `clang-sys`** to
obtain the same ABI the C compiler would use:

- exact record sizes and field offsets for the target triple,
- alignment and padding,
- typedef resolution,
- function signatures, calling conventions, and variadic tails,
- `const` qualifiers and pointer depth.

The backend converts this into `SdkgenModule` entries:

```rust
CRecord { name: "Timespec", fields: [...], size, align }
CFunction { name: "clock_gettime", args: [...], ret: ..., variadic: false }
CTypedef { name: "clockid_t", underlying: "i32" }
```

A target triple (`--target`) is required for cross-compilation; the default is
the host triple. The first supported target is **Linux x86_64**; Windows and
macOS support is explicitly deferred.

### 5. File output layout

```text
handl-packages/generated/libc/clock.hnl                 -- port
handl-packages/generated/libc-c-linux-x86_64/clock.hnl  -- C adapter for linux-x86_64
handl-packages/generated/libc-c-darwin-aarch64/clock.hnl  -- (later)
```

Port files are not regenerated per target. Adapter files are generated per
`<backend>-<target>` package and checked in.

### 6. Output form — abstract `export` and concrete `export ... :=`

#### Port (`handl-packages/generated/libc/clock.hnl`)

```handl
--- libC clock operations.
---
--- This is the platform-agnostic port. It declares the abstract arrows that
--- every `libc-*` adapter must satisfy.

from "ffi" import { FFI, Errno }
from "time" import { Clock }
from "result" import { Result }

--- C `struct timespec`.
export Timespec := {
  tv_sec: Int
  tv_nsec: Int
}

--- `clock_gettime` port.
---
--- @param {clock_id} - C `clockid_t` (`0` = CLOCK_REALTIME, `1` = CLOCK_MONOTONIC)
--- @returns {Result<Timespec, Errno>} - monotonic or real-time clock reading
export ClockGettime: (Int) ~[(FFI, Clock<Timespec>)]~> Result<Timespec, Errno>
```

#### Adapter (`handl-packages/generated/libc-c-linux-x86_64/clock.hnl`)

```handl
--- Linux x86_64 libc adapter for `libc.clock`.

from "ffi" import { call, errno }
from "result" import { ok, err }
from "../libc/clock" import { Timespec, ClockGettime }

--- Calls `clock_gettime` in `libc.so.6`.
export clock_gettime: ClockGettime := (clock_id) ~> do {
  r = call(
    "libc.so.6",
    "clock_gettime",
    [clock_id],
    ["i32", "out_struct:{tv_sec:i64,tv_nsec:i64}"],
    "i32",
    false
  )
  if r.ret < 0 {
    err(errno())
  } else {
    ok(r.buf)
  }
}
```

Note: `call` allocates the `out_struct` itself; the C `struct timespec *`
argument is **not** a visible Handl argument. The port signature reflects the
Handl-visible surface only.

### 7. All FFI arrows carry the `FFI` grade

Every adapter `export ... :=` is graded with `FFI` (and, when applicable,
target-specific grades like `Clock`). `FFI` is the subset of `Effect` reserved
for foreign function calls. FFI is never `Pure`. This is enforced by the
emitter for every generated adapter, not inferred.

### 8. Mappings to Handl

| C concept | Port | Adapter descriptor | Notes |
| --- | --- | --- | --- |
| `int` / `int32_t` | `Int` | `i32` |  |
| `long long` / `int64_t` | `Int` | `i64` |  |
| `double` | `Float` | `f64` |  |
| `char *` (NUL string) | `String` | `cstr` | rejects embedded NUL at runtime |
| `const char *` (byte buffer) | `Bytes` | `cbytes` |  |
| `void *` / opaque handle | `Handle` | `ptr` / `handle` | `handle` wrapped in a resource table entry |
| `T *` (out param) | `Obj` / named record | `out_struct:{...}` | allocated by `ffi.call`; appears in return `.buf` |
| `void *` + size (in) | `Bytes` | `in_buf` |  |
| `struct` | `data` or `Obj` | `out_struct:{...}` | use `data` once desugaring is verified |
| `enum` | `Int` + named constants | `i32` |  |
| `typedef` | type alias | follows underlying |  |
| `size_t` / `ssize_t` | `Nat` / `Int` | target-width integer | `libclang` provides target width |
| variadic | fixed-arity wrapper | `...` not directly used | variadic C calls get fixed-arity Handl wrappers |

`String` lives in the `string` package; `Bytes` is not part of `ffi` and may be supplied by a future `bytes` package. Where the header is ambiguous (e.g. a `char *` that is a byte buffer, or a
pointer that is nullable), a per-library `.sdkgen.yaml` file provides
overrides:

```yaml
symbols:
  open:
    args:
      - cstr
      - i32
    ret: Int
    nullable: false
  fopen:
    args:
      - cstr
      - cstr
    ret: Handle
    nullable: true
```

### 9. Doc comments

The full `---` doc comments live on the **port** `export`s. They include:

- the original C declaration as a comment,
- `@param` for each visible input,
- `@returns` for the output,
- `@details` for ABI notes,
- `@example` when available.

The **adapter** `export`s do **not** repeat the port's doc block. The adapter
only adds a short implementation note — typically the shared library name and
target triple — and any C-ABI caveats that are specific to this backend. The
port remains the single source of documentation.

When the C header contains Doxygen/XML comments, the C backend extracts them
and includes them in the port's generated doc comments.

### 10. Type and grade composition

The generated port uses Handl's existing generic and grade machinery:

- `export` for both abstract types and arrows.
- `~[G]~>` for graded arrows.
- `<T>` for generic types where the C API is generic (e.g. `void *` buffers).
- `FFI` for every adapter, imported from the `ffi` package.
- `Result` imported from `result`; `Option` imported from `option`.
- `call` / `bind` / `errno` from the `ffi` package for the runtime-injected primitives.

The `ffi` package is the single source for the `FFI` grade, `Errno`/`Handle`
types, and the injected `call` / `bind` / `errno` primitives. See
[ADR-0108 — Handl FFI Package](./0108-ffi-package-for-sdkgen.md).

### 11. CLI surface

`hnl sdkgen` is added to `host-cli` (ADR-0090):

```bash
# C header → port + linux-x86_64 adapter
hnl sdkgen c /usr/include/time.h --output handl-packages/generated/libc

# use a persistent .sdkgen.yaml config instead of passing every flag
hnl sdkgen c /usr/include/time.h --config libc.sdkgen.yaml

# cross target
hnl sdkgen c curl/curl.h --output handl-packages/generated/curl --target x86_64-unknown-linux-gnu

# filter symbols
hnl sdkgen c /usr/include/time.h --output handl-packages/generated/libc --only "clock_*"
```

Exit codes follow the existing convention:

- `0` — success
- `1` — generation or validation failure
- `2` — usage or parse error

## Dependencies

The SDK generator is a build-time tool. It assumes the language/runtime
building blocks in [ADR-0108 — Handl FFI Package](./0108-ffi-package-for-sdkgen.md)
are in place:

- `FFI` grade exported by `handl-packages/ffi`.
- `Errno` and `Handle` types exported by `handl-packages/ffi`.
- `call`, `bind`, and `errno` injected into `handl-packages/ffi` by `plugin-ffi`.
- `Result` as a type and `ok` / `err` as arrows from `result`.
- `Option` as a type and `some` / `none` as arrows from `option`.
- Object/record type values for C struct/union types.

### Prerequisites before the first C backend runs

1. Resolve [ADR-0108](./0108-ffi-package-for-sdkgen.md).
2. **Confirm target triple → shared object mapping** for Linux (e.g.
   `libc.so.6`); this mapping lives in `.sdkgen.yaml`, not the generator.
3. **Add `hnl sdkgen` dispatch** to `crates/host-cli/src/main.rs`.

## Implementation phases

### Phase 0 — Readiness

- Resolve [ADR-0108](./0108-ffi-package-for-sdkgen.md): the `ffi` package
  (with injected `call`/`bind`/`errno`), and `Result`/`Option` types and
  arrows.
- Verify object/record type values can represent C structs with the correct
  field layout for `ffi.call` `out_struct` / `in_buf`.

### Phase 1 — C generator (Linux x86_64)

- Implement `plugin-sdkgen`, `plugin-sdkgen-backend-c`, `plugin-sdkgen-emitter-file`, `plugin-ext-sdkgen`.
- Support `struct`, `enum`, `typedef`, pointers, `const`, `int`/`long`/`double`,
  `char *`, `void *`, `size_t`, and variadic detection.
- Generate port + adapter for `time.h` as the first fixture.
- Golden tests for the C backend: realistic fixtures (`clock_gettime`, `open`,
  `read`) plus edge cases (variadics, `void *`, `const` pointers, enums,
  typedefs) and per-target ABI snapshots.

### Phase 2 — Port-adapter cleanliness

- Refactor generated adapters to return `Result<_, Errno>` using `call` + `errno`.
- Add `.sdkgen.yaml` overrides and symbol filtering.
- Generate doc comments from libclang comments.

### Phase 3 — Cross-platform

- Windows and macOS target support in `plugin-sdkgen-backend-c`.
- Shared-object name mapping per target.

### Phase 4 — Additional backends

- `plugin-sdkgen-backend-webidl` (WebIDL) and `plugin-sdkgen-backend-jvm` (JNI
  signatures) by
  implementing `SdkgenBackend`.

## Consequences

- **C bindings become data-driven:** `hnl sdkgen c ...` plus a small config file
  replaces hand-written `call` adapters.
- **Minimal runtime addition:** `plugin-ffi` injects `call`, `bind`, and `errno`
  into `handl-packages/ffi` so adapters can return `Result<_, Errno>` without a
  global `ffi` namespace.
- **Port-adapter pattern is enforced:** every generated SDK has a platform-free
  port and one or more target-specific adapters, consistent with the existing
  `time` / `time-c` stdlib split.
- **libclang dependency:** `plugin-sdkgen-backend-c` depends on `libclang` / `clang-sys`.
  It is a build-time host tool; it is not linked into the runtime.
- **Extensible:** future JS, JVM, or Rust backends only need a new
  `SdkgenBackend`; the IR, emitter, and CLI are reused.
- **CLI surface grows by one subcommand:** `hnl sdkgen` is added in `host-cli`
  alongside `hnl fmt`, `hnl lint`, etc.
- **Generated output is committed:** canonical SDKs are generated once and
  checked in, so they can be typechecked, linted, and reviewed like any other
  source.
- **Dependencies are explicit:** the generator does not implement FFI grades,
  types, or `ffi.errno()`; it depends on [ADR-0108](./0108-ffi-package-for-sdkgen.md).

## References

- [ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md)
- [ADR-0090 — Bootstrap CLI](./0090-bootstrap-cli.md)
- [ADR-0082 — Inductive Types as Desugared Predicates](./0082-inductive-types.md)
- [ADR-0106 — The object/arrow kernel](./0106-object-arrow-kernel.md)
- [ADR-0108 — Handl FFI Package](./0108-ffi-package-for-sdkgen.md)
- `crates/plugin-ffi/src/lib.rs` — `ffi.call` / `ffi.bind` implementation
- `handl-packages/time/index.hnl` — port example
- `handl-packages/time-c/clock_gettime.hnl` — C adapter example
