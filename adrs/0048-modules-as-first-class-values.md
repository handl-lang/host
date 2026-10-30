---
title: Modules as First-Class Values
type: adr
date: 2026-09-02
status: superseded
status_note: Accepted — modules are tagged values; closures are handles into a thread-local registry.
refs: [ADR-0013, ADR-0044]
---

# ADR-0048: Modules as First-Class Values

## Decision

1. **`Module` is a handl-level tagged value** — `{__module__: {name,
   functions}}` — and `handl-language` gains `module_value` / `as_module` /
   `is_module_value`, mirroring the language value helpers.
2. **A callable is a closure handle or a foreign name**: `{__closure__: id}`
   references the registry; `{__foreign__: "math.add"}` references a Rust
   `HostFn`. Only the handle/name is serializable, never the closure.
3. **`CLOSURE_REGISTRY` is a thread-local map** `id → {body Ast, captured env,
   defining Language}`, mirroring `COMPILED_REGISTRY`; a handle does not cross
   threads or processes.
4. **The Rust `Function` trait stays** as the primitive seam, and a
   `ClosureFunction` adapter makes a handle callable like a foreign function.
5. **`plugin-lib-module` is the leaf crate** owning the module ops
   (`module`, `module_get`, …) and the registry.
6. **The module system can be written in handl** on top of the leaf; bootstrap
   is staged: leaf ops → handl module system → stdlib modules.
7. **Rust access is not exclusive**: handl reaches Rust through registered
   handles and foreign names, not a privileged runtime trait.

## Why

A closure pairs a body (an arena reference) with an environment, so it cannot
be serialized into a value; without a handle, modules could not be values at
all. Self-hosting (ADR-0044) needs modules, imports and their exports to be
inspectable data — otherwise the module system stays Rust-only.

## Not doing

- **Serialize closures themselves** — the body is an arena reference; an
  opaque handle is the only honest value.
- **Use a Rust `Module` trait object as the value** — not inspectable, not
  language-as-data, not serializable.
- **Keep an exclusive Rust access path** — the runtime reaches Rust through
  the same handle/foreign seam handl uses.

## Consequences

- Handles are thread-local, like compiled-program handles: no cross-thread or
  cross-process module values.
- Module calls gain one adapter hop (`ClosureFunction`), shared with foreign
  calls.
- Bootstrap is staged; until the handl module system lands, the leaf ops carry
  the module table.

## Enforcement

- `crates/plugin-lib-module/` — module ops and registry.
- `crates/handl-runtime/src/meta.rs` — the `COMPILED_REGISTRY` precedent.
- Module tests in `crates/handl-runtime/tests/`.

## Design

Value shapes, registry semantics, bootstrap phases and trade-offs:
[`design/0048-modules-as-first-class-values.md`](../design/0048-modules-as-first-class-values.md).
