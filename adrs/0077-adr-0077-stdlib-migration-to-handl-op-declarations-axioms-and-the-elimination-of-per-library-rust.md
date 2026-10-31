+++
id = "ADR-0077"
title = "ADR-0077: Stdlib Migration to Handl — `op` Declarations, Axioms, and the Elimination of Per-Library Rust"
status = "open"
priority = "medium"
+++

---
title: Stdlib Migration to Handl — `op` Declarations, Axioms, and the Elimination of Per-Library Rust
type: adr
date: 2026-09-03
updated: 2026-10-29
status: accepted
status_note: |-
  Accepted. Builds on [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md),
  [ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md),
  [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md),
  and [ADR-0051 — FFI Rust Binding Facade](./0051-ffi-rust-binding-facade.md).
---

# ADR-0077: Stdlib Migration to Handl — `op` Declarations, Axioms, and the Elimination of Per-Library Rust

## Decision

1. The stdlib is handl source. Every `plugin-lib-*` library is a `.hnl` file
   under `handl-packages/`; per-library Rust crates are deleted. Rust remains
   only for the kernel, the engines, the loader, the FFI dispatcher, the host
   and the core value/runtime crates.
2. Every public arrow carries a `---` doc comment (the API contract in prose)
   and an `axiom` declaration (input/output types, `guarantee`, optional
   `requires`/`termination`).
3. A pure-handl arrow needs no proof file: the contract cache infers each arrow
   once and importers reuse it (ADR-0069 §4). An `op` arrow's axiom is trusted,
   and its contract is enforced at the eval boundary.
4. `op` declarations are the FFI surface — `op "name" : A -> B guarantee: …`
   binds a native implementation; `cap` declarations state capabilities. Both
   are additive top-level forms.
5. Every migrated file declares its capabilities and follows the
   interface/impl split (`<lib>` port, `<lib>-<target>` adapter) for
   multi-target support.
6. The six-phase port plan, the per-file requirements and the per-library
   inventory are `project/rfcs/0077-stdlib-migration.md`. Each phase is
   independently shippable and reversible; pure-logic libs go first, native
   libs after `op` + axiom enforcement exist.
7. Native bodies are handl over direct C/POSIX FFI (ADR-0083) — no Rust
   closures. A native function is a leaf `Function` with no module scaffolding
   around it.
8. Each library's public API is preserved across its migration; a breaking
   change (the `Module`/`Plugin` → `Function` + `.hnl` switch) is gated behind
   the phase that performs it.

## Why

The stdlib was mostly Rust scaffolding around handl logic because the eval-side
module registry and the inference-side module loader were never connected
(`HandlModule` closed the pure-logic gap). The correctness story — contracts,
proofs, capabilities, documentation — lived in Rust, where it cannot be
verified. Moving it into handl makes the stdlib's semantics checkable and
deletes ~60 per-library crates.

## What stays Rust

The kernel only: `plugin-lib-handl` (loader), `plugin-ffi` (dispatcher), the
`plugin-engine-*` crates, the `plugin-lib-lang-*` self-hosting bridge, the host
crates (`host-cli`, `host-lsp`, `host-tooling`), and the core crates
(`handl-language`, `handl-runtime`, `handl-value`, `platform`, `plugin-module`,
`plugin-loader-native`). Everything else — logic, declarations, contracts,
proofs, capabilities, doc comments and native implementations — is `.hnl`.

## Not doing

- **Keep the per-library Rust crates** — that is the scaffolding this removes;
  their contracts stay unverifiable.
- **One big-bang migration** — phases are independently shippable and
  reversible; the stdlib is mixed during the migration by design.
- **Prove the Rust bodies** — the kernel cannot verify them; `op` axioms are
  trusted and checked at the boundary instead.

## Consequences

- ~60 `plugin-lib-*` crates collapse into one loader + `.hnl` files; each
  library's logic, types, contracts, capabilities and docs live in one file.
- Trusted `op` axioms: a native body that violates its declared `guarantee` is
  caught by boundary checks or tests, not by the proof system.
- Runtime `requires` checking adds overhead at foreign calls; a production mode
  may disable it once inference and testing carry the weight.
- `op`/`cap` are two new top-level forms; the lexer and Pratt parser must
  support them. Native crates that used `Module`/`Plugin` switch to
  `HandlLibraryPlugin`/`HandlModule` — a breaking change gated behind Phase 4.

## Enforcement

- `handl-packages/*/index.hnl` — the doc comment + `axiom` on every public arrow.
- `crates/plugin-lib-stdlib/build.rs`, `crates/plugin-lib-stdlib/src/packages.rs`
  — vendoring and the package table (ADR-0160).
- `crates/plugin-ffi` — the `op` boundary where `requires` is checked.

## Revisit if

- A native library cannot express its contract as an `op` axiom + handl
  interface, or boundary checking costs more than the correctness it buys.

## Related

- [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md) — the module system this migration completes (Phase 7).
- [ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md) — the `ArrSource::Foreign` / `Scheme` IR this builds on.
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md) — the refinement types axioms use.
- [ADR-0051 — FFI Rust Binding Façade](./0051-ffi-rust-binding-facade.md) — the prior Rust-side declaration model this replaces.
- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md) — the inference engine behind the contract cache.
