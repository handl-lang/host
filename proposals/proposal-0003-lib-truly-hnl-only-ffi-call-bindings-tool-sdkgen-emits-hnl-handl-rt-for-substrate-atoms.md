+++
id = "P-003"
title = "lib-* truly .hnl-only: ffi.call bindings, tool-sdkgen emits .hnl, atoms stay in Rust registries"
status = "open"
created = "2026-11-22"
priority = "med"
+++

# lib-* truly .hnl-only: ffi.call bindings, tool-sdkgen emits .hnl, atoms stay in Rust registries

Status: **draft** · Created: 2026-11-22

## Goal & Overview

Make every `lib-*` repo a pure `.hnl` package (`pkg.hnl` + `src/*.hnl`, zero
Rust). The language already has the escape hatch: `ffi.call("lib.so",
"sym", args, types, ret, …)` is live `.hnl` syntax — `lib-cli` calls libc
directly with no binding crate. `lib-bytes`' own docs name the planned
end-state: *"Phase 7 rewrites them as `handle … with ffi.call(…)` and
deletes this crate."*

User direction (2026-11-22): keep Rust speed + correctness for now — kernel
and toolchain stay Rust; migrate atoms to handl later. So `lib-*` = `.hnl`
interface + specs; native atoms live in the existing Rust op registries
(`plugin-lib-stdlib` ops, `*_op_registry`, `kit-ffi`), not in `lib-*` repos.
No new `handl-rt` .so needed.

## Requirements

- `lib-*` repos contain no `Cargo.toml`/`src/*.rs` — uniform shape with the
  ~68 already-pure packages.
- `tool-sdkgen-*` generates `.hnl` `ffi.call` wrapper modules + type
  marshalling specs as data, not Rust crates.
- Native atoms stay in existing runtime registries; `kernel/` untouched.

## Technical Design

Per-bucket path (triage: `workspace/audit-2026-11-20.md` + Plan-A triage):

| Bucket | Count | Path |
|---|---|---|
| const-only couriers | ~30 | Plan A (T-116): drop crate, vendor `.hnl` via `handl-pkg-vendor` — in flight |
| binding crates (`curl,libc,libz,ip-c,otlp-c,serialize-*-c`) | ~8 | `.hnl` `ffi.call` specs; sdkgen emits wrappers from C headers |
| native plugin crates (`handl-plugin-lib-*`) | ~15 | atoms → `ffi.call` or pure `.hnl`; crate dies unless it wraps something `ffi.call` can't reach |
| substrate (`bytes,cmp,error,mood,obj`) | 5 | interface → `.hnl`; atoms stay in Rust op registries (runtime code, not lib code) |

Ordering: Plan A (couriers) → bindings via sdkgen → plugins → substrate last.

## ffi.call coverage (investigated 2026-11-22)

Implemented in `kit-ffi` (`crates/kit-ffi` registers `ffi.{call,bind,errno,
load,read,sym}`; `modules/ffi` has the cranelift JIT marshaller). Resolution
is plain `dlopen` — sonames, paths, or `"self"` (process symbol table). A Rust
`cdylib` `.so` is callable today.

Descriptor vocabulary (`modules/ffi/src/marshal.rs`): ints u8–u64/isize/
usize/i32/i64, `f64` only, `cstr`/`strv`, `ptr`/`handle`, `in_buf`/`cbytes`/
`in_ptr`, `out_buf:N`, `out_ptr:N`, `io_buf:N`, `io_scalar:T`, and real-layout
struct descriptors (`in_struct`/`io_struct`/`out_struct` with `rec:`/`arr:N:T`/
`iov`). Varargs work per-call-site with fixed descriptors (SysV). Returns:
scalars/`cstr`/`ptr`/`handle`/`f64`/`void` — **no struct-by-value**.

Census: already used by lib-cli (libc, ~20 sites), lib-fetch-c (libcurl),
lib-intl-c/lib-unicode-c (ICU), lib-ws-c, lib-tls-libssl, lib-compress (zlib),
lib-os-c, lib-rand-c, lib-concurrent-epoll, and sdkgen-c's generated output.

**Gaps:** no callbacks into `.hnl` (C→handl fn pointers — fetch-c redesigned
around this), no structs/unions by value, no `f32`/`wchar_t`, errno is
Linux-only, `with_finalizer` on handles is dead code, no `va_list`.

## Open questions

1. **`ffi.call` gaps** — callbacks, by-value structs, `f32`, finalizers are
   the blockers for full binding-crate elimination; each is engine/kit-ffi
   work, sequenced after Plan A.
2. `ffi.call("self", sym, …)` can reach process-registered symbols — an
   escape hatch if a runtime atom ever needs FFI-style access without a new
   op.
