+++
id = "ADR-0200"
title = "ADR-0200: Inductive Types as Desugared Predicates"
status = "open"
priority = "med"
+++

---
title: Inductive Types as Desugared Predicates
type: adr
date: 2026-09-04
status: accepted
status_note: |-
  Accepted.

  Depends on ADR-0189 (types are values, shared `Value` space, universe levels),
  ADR-0192 (predicates as types, partial-evaluator typechecker, proofs are
  functions), and ADR-0054 (generic typecheck engine). Supersedes the inductive
  types bullet in `0032-types-as-values-remaining.md` (item 1) by giving it a
  dedicated design.
---


# ADR-0200: Inductive Types as Desugared Predicates

> Imported from `kit-data-prelude/project` — originally `ADR-0082` (global registry repo `lang`); renumbered `ADR-0200` in the host merge.

## Decision


### 1. `data` is desugaring, not a new semantic layer

A `data` declaration is syntactic sugar that expands to ordinary handl
definitions the kernel already understands. No new type-level entity, no new
evaluation rule, no new checker mode. The desugaring produces:

- **A predicate** (the type) that recognizes values built by the constructors.
- **Constructors** as ordinary values (tagged records or functions).
- **A recursor** as an ordinary library function the checker recognizes for
  termination (consistent with `array.rec` in ADR-0192).

This is the central decision: inductive types do not add a semantic layer to
the kernel. They are an ergonomic surface over the existing types-as-predicates
model.

```text
data Nat = Zero | Succ(Nat)
```

desugars approximately to:

```handl
Nat := (v) -> v is { tag: "Zero" } or (v is { tag: "Succ", pred: Nat(v.pred) });

Zero := { tag: "Zero" };
Succ := (pred) -> { tag: "Succ", pred: pred };

Nat.rec := (base, step) -> (n) ->
  if n.tag == "Zero" then base
  else step(n.pred, Nat.rec(base, step)(n.pred));
```

The checker recognizes `Nat.rec` as structurally recursive on its `n` argument,
so normalization of `Nat.rec(...)` terminates and can be used in proofs.

### 2. Inductive families fall out of parameterized predicates

`Vec(A, n)` is a predicate parameterized by a type `A` and a value `n: Nat`.
The constructors carry the index, and the predicate checks it:

```text
data Vec(A, n: Nat) = VNil | VCons(A, Vec(A, n))
```

`VNil` inhabits `Vec(A, Zero)`. `VCons(x, xs)` inhabits `Vec(A, Succ(k))` when
`xs` inhabits `Vec(A, k)`. The desugared predicate closes over the index and
verifies it on construction. No new mechanism — just predicates that take
value arguments, which ADR-0189 already supports.

### 3. Pattern matching is sugar over the recursor

Surface-level `match` on an inductive type desugars to a call to the generated
recursor. This keeps the eliminators as the single foundational mechanism
(consistent with ADR-0192's `array.rec` approach) while giving users ergonomic
pattern syntax:

```text
match n {
  Zero => 0,
  Succ(k) => 1 + f(k),
}
```

desugars to `Nat.rec(0, (k, ih) -> 1 + f(k))(n)`.

Dependent pattern matching — where the goal type refines based on the matched
constructor — is handled by the recursor's motive argument. The motive is a
function from the index to a type, so each branch can return a different
refined type. This is the standard dependently-typed eliminator design, expressed
as ordinary handl functions.

### 4. Termination checking is on-demand (ADR-0192)

The recursor is recognized by the checker as structurally recursive. User
functions that recurse on an inductive type must also pass termination analysis
to be usable in proofs (to be `Total`, per ADR-0192). Functions that fail
termination analysis are not rejected — they fall back to runtime checking, per
ADR-0192's graduated-guarantee model.

### 5. Proof erasure is deferred

Inductive types used as proofs (e.g. `Eq(a, b)`) carry runtime cost until
erasure lands. This is consistent with ADR-0192's stated negative: "No erasure.
Predicates are values carried at runtime; the runtime backstop has a cost.
Optimization (dropping checks for proven annotations) is a future pass." Erasure
is tracked as item 6 in `0032-types-as-values-remaining.md` and is not part of
this ADR.

### 6. Strict positivity is checked at desugar time

A `data` declaration is rejected if a constructor argument contains the type
being defined in a negative position (e.g. `data Bad = MkBad(Bad -> Int)`).
This is the standard strict positivity condition, required for the recursor to
be well-founded. The check runs at desugar time, before any predicate is
generated. Non-strictly-positive declarations are rejected with a clear error.


## Why

An inductive type is a user-defined algebraic datatype defined by enumerating its constructors. Each declaration generates:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Users can define their own structured types** — algebraic datatypes,
- **No new semantic layer.** `data` is sugar; the kernel, typechecker, and
- **Inductive families are free.** Parameterized predicates already exist;
- **Pattern matching is ergonomic** without introducing a second elimination
- **Termination is graduated.** Proven recursion enables static proofs; unproven

## Enforcement

- `plugin-typecheck-nbe` — where this is true.
- `plugin-tool-typecheck-nbe` — where this is true.
- `plugin-lib-type` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0200-inductive-types.md`](../design/0200-inductive-types.md).
