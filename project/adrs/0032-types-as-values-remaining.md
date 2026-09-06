# ADR-0032 (Remaining): Types are Values — Advanced Type System Roadmap

## Status

Proposed

## Context

ADR-0032 v1 (Types are values, shared Value space, universe levels) has been implemented and archived. The core design is in place: a type is a collection of values, `is` is membership evidence, `:` is a membership assertion, equality is a propositional type, and the runtime and typechecker share the same `Value` representation. Universe levels and cumulativity are fully working.

The following advanced features remain on the roadmap.

## Decision

Implement the remaining advanced type-system features:

1. **`data` declarations for inductive types** — Define algebraic datatypes with sum-of-products structure, generating constructors and eliminators.
2. **`rec` / termination checking** — Ensure recursive type-level definitions are total and terminating via structural recursion or lexicographic ordering checks.
3. **Inductive families** — Enable types parameterized by values, like `Vec(A, n)` where `n` is a length index.
4. **Dependent pattern matching with a motive** — Pattern match on indexed types, refining the goal type based on constructor arguments.
5. **Type inference and unification** — Infer types in expressions with `_` holes; solve constraints via unification.
6. **Proof erasure at runtime** — ~~Remove proof terms before evaluation to reduce runtime overhead while preserving type safety.~~
   - *Completed.* Built-in equality/membership proofs (`type.refl`, `type.is_evidence`) and the uninhabited `type.empty` are erased in `Runtime::run_checked` after typechecking. User-defined `data` proof-constructor erasure is a follow-up requiring surface syntax support.
7. **Tactics** — Provide tactic-assisted proof construction (`auto`, `induction`, `rewrite`, `simp`) for interactive type-level programming.

## Consequences

- handl users can express algebraic datatypes, proofs, and dependent types in a unified framework.
- The typechecker can verify totality and termination of inductive definitions at the type level.
- Dependent pattern matching enables type-safe manipulation of indexed structures like vectors and finite sets.
- Proof erasure keeps the runtime efficient despite full dependent typing.
- Tactics reduce boilerplate proof code and improve ergonomics for type-level programming.

## Related work

- `project/adrs/archived/0032-types-as-values.md` — original decision and core implementation
- `project/adrs/archived/0032-types-as-values-v1-complete.md` — v1 completion status (universe levels, etc.)
- ADR-0054 (Generic Typecheck Engine)
- Crates: `plugin-typecheck-nbe`, `plugin-engine-typecheck-nbe`, `plugin-lib-type`
