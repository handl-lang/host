+++
id = "ADR-0196"
title = "ADR-0196: Traits as Arrow Signatures — Graph-Based Interfaces and Contracts"
status = "open"
priority = "med"
+++

---
title: Traits as Arrow Signatures — Graph-Based Interfaces and Contracts
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed. **Partially superseded by [ADR-0197 — Decoupled Trait Layer](./0197-decoupled-trait-layer.md):**
  the bundled `trait`/`impl`/`law` syntax (§2, §6, §7, §8, §13) is
  superseded by three independent composable features (laws, bundles,
  handlers). The research foundation (§3 catalog, §4 storage, §11 CutElim,
  §12 catalog search as fallback) is preserved and remains the substrate
  for ADR-0197. Prototype implemented in the arrow-calculus PoC.
  Awaiting:

  - Lean4 mechanization of trait inference for law patterns (in progress;
    functor laws proven, non-functor laws require semantic axioms — see
    `EquationalLawInference.lean`).
  - CutElim law checking mechanization (part of this ADR, §11;
    congruence + strong normalization for the 6-rewrite fragment proven
    — see `CutElimCongruence.lean`).
  - Handler-based resolution implementation (§13) — design complete,
    implementation pending.
  - Catalog storage interface + SQLite implementation (done — see
    `trait_catalog_sqlite.rs`).
  - Built-in catalog population (done — functor, coproduct functor,
    sorting, mapping, filtering).
  - Prototype implementation in the arrow-calculus PoC (done — see
    `RESULTS_TRAITS.md`).

  Builds on [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md)
  (`arr`/`>>>`/`first`/`choice`/`loop`/`app` as the kernel),
  [ADR-0192 — Predicates as Types](./0192-predicates-as-types.md) (types as
  predicates, dependent typing without a sublanguage),
  [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md) (modules are
  arrows), and [ADR-0195 — Arrow Surface Syntax v0.1](./0195-arrow-syntax-v0_1-surface-redesign.md)
  (human-facing syntax over the kernel). Extends the kernel with a
  **trait/impl** layer grounded in arrow signatures and graph assignments,
  without introducing a separate interface sublanguage.
---


# ADR-0196: Traits as Arrow Signatures — Graph-Based Interfaces and Contracts

> Imported from `kit-data-prelude/project` — originally `ADR-0075` (global registry repo `lang`); renumbered `ADR-0196` in the host merge.

## Decision

1. **Primary mode: trait inference (default).** The user writes a graph. The system derives:
2. **Secondary mode: explicit declarations (optional).** When the user wants to **name** a law pattern — for documentation, explicit intent, or to constrain generic code — they may declare a trait explicitly. This is sugar over the inference:
3. **The open trait catalog.** Trait inference does not magically derive unknown laws from nothing. It indexed catalog. This is the same approach the existing invariant inference uses (`LoopInvariantDiscovery.lean` recognizes side-channel, accumulator, and filter patterns from a hardcoded set) — generalized from hardcoded patterns to an open, user-extensible library.
4. **Catalog storage: abstract interface, SQLite implementation.** The trait catalog is persisted in a storage backend. The storage interface is abstract; SQLite is the default implementation. The abstraction is key: storage is an implementation detail that can change later without affecting the trait inference engine.
5. **Type parameter syntax: angle brackets, not square brackets.** Type application uses **angle brackets** `F<A>`, not square brackets `[A]`.
6. **Impl declaration (graph layer — annotation-free).** An impl is a graph assignment for each operation in the trait's signature. The user writes **only the graphs**. No types, no proofs.
7. **Generic code with trait bounds.** A function may declare trait bounds on its type parameters. The bound is a **capability declaration**: the named trait's operations are unhandled effects within the function body — someone must provide them (§13).
8. **Supertraits as signature inclusion.** A supertrait bound `trait T : U` means `Σ_T` requires `Σ_U` to be satisfied first. This is **signature inclusion**: the impl must provide graphs for both `Σ_U` and `Σ_T`.
9. **Associated types via dependent types.** A trait may declare an associated type using the dependent-type machinery from `DependentProofGraphs.lean` (`dSigma`/`dPi`):
10. **Blanket impls via ArrowApply; Law verification: the `CutElim` connection; Trait resolution: catalog search (fallback); Trait resolution: explicit handlers (primary); Migration policy.** See the design doc for these decisions.

## Why

ADR-0052 fixed the kernel: programs are arrow graphs built from `arr`/`>>>`/`first`/`choice`/`loop`/`app`. ADR-0192 made types into predicates — ordinary values that return `bool` — with dependent typing falling out without a `total` sublanguage. ADR-0069 made modules into arrows. ADR-0195 fixed the surface syntax.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **No new calculus primitive.** Trait inference is a recognition layer
- **Trait inference is annotation-free.** The user writes only the
- **Laws are proven, not tested** _(contingent on law inference
- **Proof reuse by composition.** If `migrate = deserialize >>> serialize`
- **Explicit declarations are optional sugar.** When the user wants to

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (mechanism, tables, proofs, trade-offs): [`design/0196-traits-as-arrow-signatures.md`](../design/0196-traits-as-arrow-signatures.md).
