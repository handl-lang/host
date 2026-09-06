# ADR-0082: Inductive Types as Desugared Predicates

## Status

Accepted.

Depends on ADR-0032 (types are values, shared `Value` space, universe levels),
ADR-0068 (predicates as types, partial-evaluator typechecker, proofs are
functions), and ADR-0054 (generic typecheck engine). Supersedes the inductive
types bullet in `0032-types-as-values-remaining.md` (item 1) by giving it a
dedicated design.

## Context

### What an inductive type is

An inductive type is a user-defined algebraic datatype defined by enumerating
its constructors. Each declaration generates:

- **Constructors** — the ways to build values of the type.
- **An eliminator (recursor)** — the canonical way to consume a value, with one
  branch per constructor.

```text
data Bool = True | False
data Nat = Zero | Succ(Nat)
data Vec(A, n: Nat) = VNil | VCons(A, Vec(A, n))
```

Four properties distinguish inductive types from plain enums in a
dependently-typed setting:

1. **Inductive families** — types indexed by _values_, not just types.
   `Vec(A, 3)` and `Vec(A, 0)` are distinct types; the length lives in the
   type.
2. **Termination checking** — recursion over an inductive type must be
   structurally decreasing so evaluation always halts and the typechecker is
   total.
3. **Dependent pattern matching** — matching a constructor refines the indices.
   Match `VCons(x, xs)` and the typechecker knows the length is `n+1`.
4. **Proof erasure** — proofs are inductive types (e.g. `Eq(a, b)` inhabited
   only by `Refl`); the runtime erases proof terms, keeping only computation.

### What handl already has

ADR-0032 established that types are values: a type is a collection of values,
`is` is membership evidence, `:` is a membership assertion, and the
typechecker and runtime share the same `Value` representation. Universe levels
and cumulativity are implemented and archived.

ADR-0068 established that types are _predicates_ — ordinary `bool`-returning
functions — and that the typechecker is a partial evaluator with a termination
guard. Proofs are ordinary functions. Induction principles (`array.rec`) are
library functions the checker recognizes, not syntax. There is no `total`
sublanguage; totality is a property the checker verifies on demand for the
expressions it normalizes.

### The gap

Users cannot introduce their own structured types at the type level. They can
write predicates by hand, but defining a sum type with constructors and a
recursor today requires manually constructing the predicate, the constructor
tagging, and the eliminator — verbose, error-prone, and without any guarantee
that the recursor is well-founded. The kernel bakes in `array` and a few
primitives; everything else is out of reach.

`0032-types-as-values-remaining.md` lists inductive types as item 1 of seven
remaining features, but gives no design. This ADR provides that design.

## Decision

### 1. `data` is desugaring, not a new semantic layer

A `data` declaration is syntactic sugar that expands to ordinary handl
definitions the kernel already understands. No new type-level entity, no new
evaluation rule, no new checker mode. The desugaring produces:

- **A predicate** (the type) that recognizes values built by the constructors.
- **Constructors** as ordinary values (tagged records or functions).
- **A recursor** as an ordinary library function the checker recognizes for
  termination (consistent with `array.rec` in ADR-0068).

This is the central decision: inductive types do not add a semantic layer to
the kernel. They are an ergonomic surface over the existing types-as-predicates
model.

```text
data Nat = Zero | Succ(Nat)
```

desugars approximately to:

```handl
Nat = (v) -> v is { tag: "Zero" } or (v is { tag: "Succ", pred: Nat(v.pred) });

Zero = { tag: "Zero" };
Succ = (pred) -> { tag: "Succ", pred: pred };

Nat.rec = (base, step) -> (n) ->
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
value arguments, which ADR-0032 already supports.

### 3. Pattern matching is sugar over the recursor

Surface-level `match` on an inductive type desugars to a call to the generated
recursor. This keeps the eliminators as the single foundational mechanism
(consistent with ADR-0068's `array.rec` approach) while giving users ergonomic
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

### 4. Termination checking is on-demand (ADR-0068)

The recursor is recognized by the checker as structurally recursive. User
functions that recurse on an inductive type must also pass termination analysis
to be usable in proofs (to be `Total`, per ADR-0068). Functions that fail
termination analysis are not rejected — they fall back to runtime checking, per
ADR-0068's graduated-guarantee model.

### 5. Proof erasure is deferred

Inductive types used as proofs (e.g. `Eq(a, b)`) carry runtime cost until
erasure lands. This is consistent with ADR-0068's stated negative: "No erasure.
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

## Consequences

### Positive

- **Users can define their own structured types** — algebraic datatypes,
  inductive families, proofs — without the kernel baking each one in.
- **No new semantic layer.** `data` is sugar; the kernel, typechecker, and
  runtime see only predicates, values, and functions they already handle. This
  is consistent with ADR-0068's "one language, one engine" principle.
- **Inductive families are free.** Parameterized predicates already exist;
  value-indexed predicates fall out without new machinery.
- **Pattern matching is ergonomic** without introducing a second elimination
  mechanism — it desugars to the recursor.
- **Termination is graduated.** Proven recursion enables static proofs; unproven
  recursion falls back to runtime, per ADR-0068.

### Negative

- **Desugaring is verbose at the IR level.** A `data` declaration expands to
  several definitions; the IR is larger than the surface. Debugging may require
  understanding the expansion.
- **No erasure yet.** Proof terms built from inductive types carry runtime cost.
  Users who define `Eq` and construct `Refl` pay for it at runtime until erasure
  lands.
- **Strict positivity is a new check**, but it is local to `data` desugaring
  and does not affect the rest of the system.
- **Dependent pattern matching via eliminators is less direct** than native
  `match` with refinement. Some constructions that are one-liners in Agda/Lean
  require explicit motive arguments here. This is the cost of not having a
  separate proof engine.

### Build order

1. `data` desugaring for simple inductive types (no indices): `Bool`, `Nat`,
   `Maybe`, `Either`. Generate predicate, constructors, recursor.
2. Strict positivity check at desugar time.
3. Checker recognition of generated recursors for termination (structural
   recursion on the inductive argument).
4. `match` as sugar over the recursor.
5. Inductive families: value-indexed predicates (`Vec`, `Fin`).
6. Dependent pattern matching with motive argument.
7. Proof erasure (tracked separately in `0032-types-as-values-remaining.md`
   item 6; not part of this ADR).

Steps 1-3 are the foundation. Steps 4-5 are the ergonomic layer. Step 6 is the
full dependent-typing layer.

## Related work

- `project/adrs/archived/0032-types-as-values.md` — types are values, shared
  `Value` space.
- `project/adrs/archived/0032-types-as-values-v1-complete.md` — v1 completion
  (universe levels).
- `project/adrs/0032-types-as-values-remaining.md` — roadmap this ADR
  supersedes (item 1).
- `project/adrs/0068-predicates-as-types.md` — partial-evaluator typechecker,
  proofs are functions, induction principles are library functions.
- `project/adrs/0054-generic-typecheck-engine.md` — typecheck engine
  architecture.
- Crates: `plugin-typecheck-nbe`, `plugin-engine-typecheck-nbe`,
  `plugin-lib-type`.
