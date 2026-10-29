---
title: 'ADR-0032 (Archived v1): Types are Values — Universe Levels and Core Type System'
type: adr
date: 2026-09-02
status: accepted
---

# ADR-0032 (Archived v1): Types are Values — Universe Levels and Core Type System

## Status

Accepted

## Context

The handl language converged on a design where the type language and expression language are unified. Types are ordinary handl values. This required:

1. A shared `Value` representation between runtime and typechecker
2. Base type families (collections) like `type.number`, `type.string`, `type.bool`
3. Propositional equality `===` as a type, not a boolean
4. Membership evidence with `is` and membership assertions with `:`
5. Universe levels (`Type_0`, `Type_1`, etc.) with cumulativity

## Decision Implemented

### Core Features (v1 Complete)

1. **Shared Value representation** — `crates/plugin-vm-type` defines unified `Value`/`Term` space with `Collection`, `Membership`, `Is`, `Eq`, `Refl`, `Empty`
2. **Base collections** — `type.number`, `type.string`, `type.bool`, `type.null`, `type.array`, `type.object`, `type.eq` in primitives
3. **Propositional equality** — `x === y` is a type with `refl` when equal, empty when not
4. **Membership evidence** — `x is T` produces proofs; `:` assertion binds typed names
5. **Universe levels** — `Type_n` for each level n, with cumulative hierarchy and universe cumulativity in checking
6. **Dependent types** — Pi (`∀`), Sigma (`∃`), and dependent pattern structures fully working
7. **NBE typechecker** — Normalization-by-evaluation engine in `plugin-typecheck-nbe` and `plugin-engine-typecheck-nbe` with proper universe level handling

### Implementation Evidence

- ✅ `Value::Universe(Level)` with level arithmetic (max, succ)
- ✅ Typechecker universe cumulativity: `Type_n ⊆ Type_m` for `n ≤ m`
- ✅ Universe level inference in dependent function and sigma types
- ✅ Tests: `synth_universe_and_literal`, `synth_dependent_pi`, `check_universe_cumulativity`, `universe_level_returns_zero_for_base_type`
- ✅ Engine integration: `construct_universe`, `universe_level`, `universe_max` judgments
- ✅ Proper kind checking for type-level expressions

## Consequences

- ✅ Even types like `Even = (n: type.number) -> n % 2 === 0` work as expected
- ✅ `Person = { name: type.string, age: type.number }` as record/sigma type
- ✅ Proper universe level inference prevents type-in-type errors
- ✅ Cumulative hierarchy allows code like `f : Type_1 -> Type_2` to accept `Type_0` arguments

## See also

- ADR-0032 (Remaining) for advanced features: inductive types, dependent pattern matching, tactics, proof erasure
- ADR-0054 (Generic Typecheck Engine) for the engine architecture using this value space
- Crates: `plugin-vm-type`, `plugin-typecheck-nbe`, `plugin-engine-typecheck-nbe`, `plugin-lib-type`
