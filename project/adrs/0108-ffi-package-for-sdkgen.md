# ADR-0108: Handl FFI Package — Grade, Types and Error Arrows

## Status

Accepted.

Depends on [ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md), the existing `grade` package, and the existing `result` and `option` packages.

The SDK generator in [ADR-0107](./0107-handl-sdk-generator.md) needs everything described here before it can emit typed `Result`/`Option`-aware adapters.

## Context

The runtime calls C code through primitives that are injected into the `ffi` Handl package (`call`, `bind`, `errno`; see `crates/plugin-ffi/src/lib.rs`). `handl-packages/ffi` is therefore a **partial package**: it contains authored exports (`FFI`, `Errno`, `Handle`) and Rust-injected exports (`call`, `bind`, `errno`). The runtime still provides the actual implementation, but the surface is typed and imported like any other package:

- A dedicated `FFI` grade, conceptually a subset of `Effect`.
- `Errno` and `Handle` types.
- A way to read `errno` as a value so generated adapters can return `Result<_, Errno>`.

`Option` already lives in the `option` package and `Result` already lives in the `result` package. This ADR does not redefine them; generated adapters import them directly.

## Decision

### 1. `handl-packages/ffi` is the FFI surface package

All SDK-generated adapters import from one place:

```handl
--- ffi — grades, types and arrows for foreign function calls.
---
--- `call`, `bind`, and `errno` are injected by the Rust runtime so they are
--- typed, documented, and imported like ordinary Handl arrows.

--- C `errno` value as a Handl integer.
export Errno := Int

--- Opaque foreign handle (file descriptor, socket, pointer, etc.).
export Handle := Obj

--- Foreign function call primitive. Implementation injected by the runtime.
export call

--- Foreign function bind primitive. Implementation injected by the runtime.
export bind

--- Read the current thread's C `errno` value.
export errno: () ~[FFI]~> Errno
```

`Errno` and `Handle` are type names the SDK uses to keep C concepts explicit; they do not need `data`.

### 2. `FFI` is a subset of `Effect`

The existing `grade` package exports the generic `Effect` grade. The `ffi` package adds the more specific `FFI` grade. For the first implementation, the relationship is documentary: a grade tuple such as `~[(FFI, Clock<T>)]~>` is an explicit named set, not a sub-grade lattice. The generator always emits `FFI` for any adapter that calls `call` / `bind` from the `ffi` package.

### 3. `Result` and `Option` stay in their own packages

The `ffi` package does **not** redefine `Result` or `Option`. Generated adapters import them directly:

```handl
from "result" import { Result, ok, err }
from "option" import { Option, some, none }
from "ffi" import { FFI, Errno }
```

The `result` package must export `Result` as a type and `ok` / `err` as arrows.

The `option` package must export `Option` as a type and `some` / `none` as arrows.

### 4. The runtime injects `call`, `bind`, and `errno` into the `ffi` package

The `plugin-ffi` crate continues to implement the actual C call logic, but the primitives are exposed through the `ffi` Handl package rather than a global. `call` performs a dynamic foreign call, `bind` returns a first-class arrow with bound arguments, and `errno` reads the current C `errno`. This lets generated adapters return typed `Result` values without a global `ffi` namespace:

```handl
from "ffi" import { call, errno }
from "result" import { ok, err }

export clock_gettime: ClockGettime := (clock_id) ~> do {
  r = call("libc.so.6", "clock_gettime", [clock_id], ["i32", "out_struct:{...}"], "i32", false)
  if r.ret < 0 {
    err(errno())
  } else {
    ok(r.buf)
  }
}
```

A future `call_result(...)` primitive can combine the call and `errno` read into one step; the first version only needs `call` and `errno`.

### 5. C structs as object/record type values, not `data`

The generator does not need `data` for C structs. The natural mapping is an object/record type value:

```handl
export Timespec := {
  tv_sec: Int
  tv_nsec: Int
}
```

The port can then `export Timespec` and the adapter can use `r.buf.tv_sec` directly. `data` desugaring is therefore not a prerequisite for the SDK generator. `data` would only be needed if we later decide C structs must be nominal (different from any record with the same fields).

## Language readiness

| Feature                   | Status              | Evidence                           | Gap                                                         |
| ------------------------- | ------------------- | ---------------------------------- | ----------------------------------------------------------- |
| `Effect` grade            | Ready               | `handl-packages/grade/index.hnl`   | none                                                        |
| `FFI` grade               | Not in stdlib       | none                               | create `handl-packages/ffi`                                 |
| `Errno` / `Handle` types  | Not in stdlib       | none                               | add to `handl-packages/ffi`                                 |
| `Result` type             | In `result` package | `handl-packages/result/result.hnl` | export `Result` type plus `ok` / `err` constructors         |
| `Option` type             | In `option` package | `handl-packages/utils/option.hnl`  | export `Option` type plus `some` / `none` constructors      |
| `ffi.errno()`             | Not in runtime      | `crates/plugin-ffi/src/lib.rs`     | add `ffi.errno()` op                                        |
| Object/record type values | In grammar          | record/object literals             | **verify** layout matches C ABI for `out_struct` / `in_buf` |

## Consequences

- The SDK generator (ADR-0107) depends on `handl-packages/ffi` and the runtime primitives here.
- `result` and `option` remain separate; `ffi` only adds the FFI-specific surface.
- `plugin-ffi` injects `call`, `bind`, and `errno` into `handl-packages/ffi`.
- `data` desugaring is not required; the SDK uses object/record type values for C structs.
- All generated adapters have a single import set: `ffi`, `result`, `option`.

## References

- [ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md)
- [ADR-0082 — Inductive Types as Desugared Predicates](./0082-inductive-types.md)
- [ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md)
- `handl-packages/grade/index.hnl`
- `handl-packages/result/result.hnl`
- `handl-packages/utils/option.hnl`
- `crates/plugin-ffi/src/lib.rs`
