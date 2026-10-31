+++
id = "ADR-0088"
title = "ADR-0088: Bootstrap Typechecker (`plugin-engine-typecheck`)"
status = "open"
priority = "medium"
+++

---
title: Bootstrap Typechecker (`plugin-engine-typecheck`)
type: adr
date: 2026-10-15
status: implemented
status_note: |-
  Implemented (merged to main). Part of the bootstrap tooling strategy
  ([ADR-0085](./0085-bootstrap-tooling-strategy.md)). Builds on
  [ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md),
  [ADR-0052](./0052-arrow-calculus-kernel.md), and
  [ADR-0074](./0074-arrow-syntax-v0_1-surface-redesign.md). Temporary: will be
  replaced by the self-hosted typechecker described in
  [RFC-0004](../rfcs/0004-typechecker-in-handl.md).
---

# ADR-0088: Bootstrap Typechecker (`plugin-engine-typecheck`)

## Context

Arrow kind checking and the structural type inference sketched in ADR-0074
(ID.7, row variables, `rec` region typing per
`research/arrow-calculus-kernel-poc/ARROW_REC_TYPING_V0_1.md`) do not exist
as a usable, per-file checking pass. The stdlib cannot be written with any
confidence about types until something checks it. The existing
`plugin-engine-typecheck*` crates target the eval-engine value model, not
the v0.1 surface AST.

## Decision

The surface-level checking work lands in the existing typecheck crates —
`plugin-engine-typecheck` (with `plugin-typecheck-core` /
`plugin-typecheck-nbe` where the machinery fits) — extended with a
**bootstrap check pass over the v0.1 surface AST**. It is a bootstrap
checker, not the final inference engine: it implements the decidable core
of ADR-0074's typing rules and returns actionable diagnostics (shared types
from `plugin-ext-diag`, ADR-0091); row-variable inference and full `rec`
fixpoint typing stay behind explicit `todo!` boundaries with diagnostics
that say "not yet checked" rather than silently passing.

- **Scope (v0.1):**
  1. Arrow kind checking of composed graphs (`arr`/`>>>`/`first`/`choice`
     /`loop`/`app` arities and wire types per ADR-0052).
  2. Structural type inference for literals, records, positional
     structures, and projections (per ADR-0074 §types).
  3. `rec` region well-formedness: feedback wires are consumed exactly
     where produced, no phantom feedback (per the rec-typing reference).
  4. Module-level checks: every import projection used, every exported
     name bound, no duplicate bindings.
- **Not in scope (deferred to RFC-0004):** trait/impl checking (ADR-0075),
  refinement inference (ADR-0070), state-machine invariants (ADR-0073),
  polymorphic instantiation over row variables.
- **API surface:**

  ```rust
  pub fn check_module(src: &str) -> Result<TypecheckReport, ParseError>;
  pub struct TypecheckReport { pub diagnostics: Vec<Diagnostic> }
  ```

  `Diagnostic` comes from `plugin-ext-diag` (ADR-0091) — no local variant
  types in the typecheck crates.

- **Architecture:** single pass composition — collect bindings → infer
  expression types bottom-up by AST child roles (ADR-0081) → unify
  structurally with spans retained → report. Error recovery: report all
  diagnostics per module, not first failure.
- **Testing:** 100% coverage. Table-driven `tests/` per typing rule from
  ADR-0074 and the grammar spec; golden typecheck reports; CI runs it over
  all `handl-packages/` and `examples/` `.handl` files (must be clean).

## Consequences

- The stdlib gets real feedback while it is being written in handl.
- Deliberately incomplete inference means some valid v0.1 programs are
  rejected with "not yet checked" — acceptable and documented; the RFC-0004
  engine replaces these boundaries.
- The checker lives inside the generic `plugin-engine-typecheck` machinery
  (parsing/unification infrastructure) rather than forking it; what is new
  here is the surface-AST-level pass and its diagnostics. Keeping the
  engine generic preserves replaceability per ADR-0085.
