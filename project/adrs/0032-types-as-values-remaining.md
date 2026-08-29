# ADR-0032 (Remaining): Types are Values — Advanced Type System Roadmap

## Status

Proposed

## Context

ADR-0032 (Types are values, runtime and typechecker share one value space) has been implemented and archived. The core design is in place: a type is a collection of values, `is` is membership evidence, `:` is a membership assertion, equality is a propositional type, and the runtime and typechecker share the same `Value` representation in `handl-plugin-vm-type`.

The following advanced features remain on the roadmap.

## Decision

Implement the remaining type-system features:

1. Universe levels plus `data` declarations for inductive types.
2. `rec` / termination checking for the pure type level.
3. Inductive families like `Vec(A, n)`.
4. Dependent pattern matching with a motive.
5. Type inference and unification (`_` holes).
6. Proof erasure at runtime.
7. Tactics (`auto`, `induction`, `rewrite`).

## Consequences

- handl can express datatypes, proofs, and dependent types.
- The typechecker can check totality and termination of pure type-level programs.
- Users get dependent pattern matching, holes, and tactic-assisted proof.
- Proof terms are erased before runtime, preserving the unified value model.

## Related work

- `project/adrs/archived/0032-types-as-values.md`
