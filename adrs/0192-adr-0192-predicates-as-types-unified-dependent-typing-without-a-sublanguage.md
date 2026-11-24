+++
id = "ADR-0192"
title = "ADR-0192: Predicates as Types — Unified Dependent Typing Without a Sublanguage"
status = "open"
priority = "med"
+++

---
title: Predicates as Types — Unified Dependent Typing Without a Sublanguage
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed.

  Depends on the existing "types as predicates" model documented in the
  [Language overview](../../docs/explanation/language-overview.md) (refinement
  types, type families, `x is T` as `T(x)`). Extends it with a unified
  compile-time/runtime checking strategy and a library of predicates _about_
  functions, without introducing a separate total/proof sublanguage.
---


# ADR-0192: Predicates as Types — Unified Dependent Typing Without a Sublanguage

> Imported from `kit-data-prelude/project` — originally `ADR-0068` (global registry repo `lang`); renumbered `ADR-0192` in the host merge.

## Decision


### 1. One language, one checking engine, graduated guarantees

There is no total fragment and no proof sublanguage. The typechecker is a
**partial evaluator with a termination guard**:

- To verify `x: T = e`, normalize `T(e)`.
- If normalization succeeds and yields `true`, accept statically.
- If normalization yields `false`, reject.
- If normalization is blocked (effect, non-termination, opaque value),
  insert a runtime check `assert T(e)` and accept.

Termination analysis is run **on demand** on the expressions being
normalized, not on the whole program. Structural recursion passes;
non-structural recursion aborts normalization and falls back to runtime.

### 2. Proofs are functions, propositions are types

A proof is an ordinary handl function whose return type is a proposition (a
predicate applied to values). The checker normalizes the proof body to
verify it. Induction principles (`array.rec`, etc.) are library functions
the checker recognizes, not syntax.

```handl
length_append: (a, b)
  : array.length(append(a, b)) == array.length(a) + array.length(b)
{
  array.rec(
    base: by_eval,
    step: (x, rest, ih) -> by_norm + ih
  )
}
```

`by_eval` and `by_norm` are checker directives, not language constructs:
they instruct the normalizer to reduce both sides and check syntactic
equality, optionally applying an induction hypothesis. A proof that
normalizes to `true` is accepted and can be used as a trusted fact by the
general checker, eliminating a runtime check at use sites.

### 3. A library of predicates about functions

Provide these as ordinary handl values, not keywords:

```handl
isFunction := (f) -> type.function(f);

Fn := (A, B) -> (f) ->
  isFunction(f) and array.every(samples(A), (x) -> B(f(x)));

DepFn := (A, B) -> (f) ->
  isFunction(f) and array.every(samples(A), (x) -> B(x)(f(x)));

Arity := (n) -> (f) -> isFunction(f) and fn.arity(f) == n;

Pure := (f) -> isFunction(f) and /* effect analysis: no effects */;

Total := (f) -> isFunction(f) and /* termination analysis passes */;
```

Predicates compose with `and`:

```handl
StrictNumToBool := Fn(type.number, type.bool) and Pure and Total;
f: StrictNumToBool := (x) -> x > 0;
```

`Pure` and `Total` require checker support (effect tracking and
termination analysis respectively). They are the bridge to the proof
layer: `Total` is what permits the checker to normalize `f` during a
proof, and `Pure` is what makes that normalization sound (no side effects
to replay). The first three predicates are cheap runtime checks; the last
two are the real checker work and are built when the proof layer is
needed.

### 4. No `typeof` keyword

Function signatures are written explicitly with `Fn(A, B)` or
`DepFn(A, B)`. A `typeof` special form was rejected because:

- It cannot infer `A` and `B` from an opaque function body soundly.
- It reintroduces a type-level operation as a special form, breaking the
  "types are values" unification.
- It does not resolve the type-of-type regress (it just renames it).

The annotation `f: Fn(A, B) = ...` is the type. `Fn(A, B)` is a value that
can be stored, passed, and applied. No keyword needed.

### 5. Equality of functions

`Eq(a)` is a predicate checking `b == a`. For functions, `==` is
intensional/structural by default (same body, same code) or reference
equality, depending on what the runtime exposes. Extensional equality
("same outputs for all inputs") is undecidable in general and is not
provided as a primitive. `FnEq(A, B, f)` provides a _sampled_ extensional
check for use as a runtime contract; full extensional equality is a
theorem and lives in the proof layer if needed.


## Why

handl treats a type as a predicate: an ordinary value (usually a function) that applied to a value yields a `type.bool`. `x: T = e` is sugar for `x = e; assert T(x)`. Refinement types and type families fall out for free:

## Not doing

- A type is a predicate — a `bool`-returning function.
- The type of a predicate is `Predicate` — "is this a `bool`-returning
- The type of `Predicate` is `Predicate`. Fixed point, not regress.
- No universe levels are required because types are not a foundational

## Consequences

- **No new surface syntax.** Dependent types, refinement types, type
- **One engine.** The partial evaluator is the typechecker. Proofs reuse
- **Graduated guarantees.** Static proofs where the checker can normalize,
- **Honest framing.** The system is a partial evaluator with a runtime
- **Composable predicates.** `and`/`or` compose types the same way they

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0192-predicates-as-types.md`](../design/0192-predicates-as-types.md).
