+++
id = "ADR-0083"
title = "ADR-0083: FFI-Native Stdlib — Handl over Direct C/POSIX Bindings"
status = "open"
priority = "medium"
+++

---
title: FFI-Native Stdlib — Handl over Direct C/POSIX Bindings
type: adr
date: 2026-09-04
status: accepted
status_note: Accepted — native bodies are handl over direct C/POSIX FFI; Rust shrinks to the kernel.
refs: [ADR-0077, ADR-0069, ADR-0052, ADR-0081]
---

# ADR-0083: FFI-Native Stdlib — Handl over Direct C/POSIX Bindings

## Decision

1. **`ffi_bind` is a host-exposed arrow op.** A binding is data — library,
   symbol, signature, error clause — and is capability-gated like any host op.
2. **Marshalling is a closed ABI vocabulary**: scalars, pointers, structs,
   `Bytes` for buffers, optional and variadic arguments, and an explicit
   handle lifecycle for opaque resources.
3. **Errors are declared per binding.** An optional error clause maps `errno`
   (or a return code) to `Err`; no sentinel-value convention.
4. **A Rust FFI dispatcher does only the mechanics**: load the library and
   symbol (cached), marshal, call, unmarshal, check the error clause.
5. **The abstraction layer is handl**: POSIX wrappers, constants and the
   stdlib layout sit over `ffi_bind`; no per-library Rust remains.
6. **Platform variants are abstracted from day one** (Linux first) and chosen
   by data, not `cfg`/`kind ==` branches.
7. **Rust keeps only the kernel**: dispatcher, module loader, engines, host and
   the core value/runtime crates.
8. **What is eliminated**: per-library Rust `Function` bodies and the
   `plugin-ffi-rust` closure layer (never built).
9. **The async runtime is handl-authored from day one**: `epoll_wait` for
   readiness, handl task dispatch; non-blocking fds, `Suspend` on async ops,
   state machines rather than continuations.
10. **Kernel freedom**: the kernel knows nothing of POSIX, libc or any library —
    every binding is data and every policy is handl.

## Why

ADR-0077 moved the stdlib to handl but left native bodies as Rust `Function`
impls, one per library, trusted to honor axioms nobody could check. Those
bodies duplicated C signatures in Rust, blocked the self-hosting story, and put
`cranelift`/`libloading` in every build. Direct FFI makes native access data
and lets Rust shrink to the kernel.

## Not doing

- **Keep `plugin-ffi-rust` closure bindings** — never built; the closure layer
  is the duplication this removes.
- **One Rust wrapper per library** — unverifiable, and it re-grows the crates
  ADR-0077 deleted.
- **Put libc/POSIX knowledge in the kernel** — it would make the kernel
  platform-bound and every target a kernel change.
- **Write the async runtime in Rust** — it would be the last per-platform
  library body; epoll + handl tasks is the same mechanism as data.

## Consequences
- Native access is data; adding a library is a binding table, not a crate.
- `errno`/return-code clauses make failure explicit, but every binding author
  must state one — the compiler cannot infer it.
- Opaque handles need an explicit lifecycle; leaks become the binding's
  documented responsibility.
- The dispatcher is the only `unsafe` surface, and the kernel stays portable.

## Enforcement
- `crates/plugin-ffi/` — the dispatcher (load, marshal, call, unmarshal).
- `handl-packages/*/` — the `ffi_bind` tables and their error clauses.
- `kit-prelude-lang/` — the stdlib layout and POSIX constants.

## Design
+The ABI vocabulary, dispatcher steps, async runtime, platform variants and the
+tiered migration map:
+[`design/0083-ffi-native-stdlib.md`](../design/0083-ffi-native-stdlib.md).
