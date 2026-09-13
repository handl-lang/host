# ADR-0109: Partial Package Injection for the `ffi` Surface

## Status

Proposed.

Depends on [ADR-0108 — Handl FFI Package](./0108-ffi-package-for-sdkgen.md), [ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md), and the existing `grade` and `type` packages.

## Context

ADR-0108 defines the desired `handl-packages/ffi` surface — a `FFI` grade, `Errno`/`Handle` type values, and `call`/`bind`/`errno` arrows — but leaves the runtime wiring as the open problem. The package is a **partial package**: most of its exports are authored Handl values and types, while a small set of exports (`call`, `bind`, `errno`) are supplied by the Rust runtime.

`handl-kit-canonical` already uses this same pattern for the `type` module: the `type-handl.hnl` record provides the authored surface and `with_ops` injects the native `type.of`/`type.to_string`/`type.to_number`/`type.coalesce` atoms. The `ffi` package can follow that precedent end-to-end.

## Decision

### 1. `handl-packages/ffi/ffi.hnl` is the authored surface

```handl
from "../grade" import { Grade }
from "type" import { Number, Obj }

export FFI := Grade
export Errno := Number
export Handle := Obj

export call: (String, String, Array, Array, String, Bool) ~[FFI]~> Value
export bind: (String, String, Array, Array, String, Bool) ~[FFI]~> Value
export errno: () ~[FFI]~> Errno
```

- `FFI`, `Errno`, `Handle` are re-exported values. Because values can be used as types, they serve as both runtime values and type names.
- `call`/`bind`/`errno` are declared without bodies; the runtime injects the actual implementations.
- Handl arrow types use `~>` or `~[g]~>`; the `FFI` grade is attached to all three primitives.

### 2. `Number` and `Obj` are exposed by the `type` module

For `from "type" import { Number, Obj }` to resolve, the `type` host module must advertise `Number` and `Obj` in its `functions()` list. They are type values, not callable functions, so the concrete mechanism is to add them to `TYPE_HANDL_FUNCS` (or a small `with_ops` injection) with a no-op/`Function` that errors if called. The import path only needs the names to be visible to the dependency resolver.

### 3. `grade` is built as a host `HandlModule`

The `grade` package already exists at `handl-packages/grade/index.hnl`. It is built as a `HandlModule` named `"grade"` over its `Grade` and `Effect` exports and made a dependency of `ffi` so `from "../grade" import { Grade }` resolves. The `Grade` value itself is `Obj` (as declared in `grade/index.hnl`); the runtime already makes `Obj` available in package context.

### 4. `call`, `bind`, and `errno` are injected via `with_ops`

`handl-kit-canonical` builds the `ffi` module as a `HandlModule` from `ffi.hnl`, with the native primitives layered on top:

- `build_grade_module()` is a dependency.
- `build_type_module()` is a dependency.
- A local `OpRegistry` registers `FfiCall`, `FfiBind`, and `FfiErrno` under `ffi.call`, `ffi.bind`, and `ffi.errno`.
- `with_ops(&["ffi.call", "ffi.bind", "ffi.errno"], ...)` injects them into the module.

`FfiErrno` is added to `plugin-ffi`; it wraps `error::last_errno()` and returns the current C `errno` as a number.

### 5. `ffi` replaces the host `FfiModule` in canonical wiring

- `build_stdlib_registry` registers `build_ffi_module()` instead of `FfiModule`.
- `StdlibResolver` resolves `"ffi"` to `build_ffi_module()`.

Because `canonical_wire_names()` is derived from `build_stdlib_registry`, `ffi`, `FFI`, `Errno`, `Handle`, `call`, `bind`, and `errno` enter the wire prelude automatically.

## Consequences

- `ffi` is now a real, importable Handl package with a typed surface, matching the SDK generator needs in ADR-0107.
- The partial-package pattern (`HandlModule` + authored `.hnl` + `with_ops` native injection) is now used for both `type` and `ffi`, providing a reusable template for future packages.
- `grade` and `type` become visible as host modules that other packages can depend on.
- The runtime keeps full control over the unsafe C-call primitives; the authored package only declares types and re-exports values.

## References

- `crates/plugin-ffi/src/lib.rs`
- `crates/kit-canonical/src/lib.rs`
- `handl-packages/grade/index.hnl`
- `handl-packages/type-handl/type-handl.hnl`
