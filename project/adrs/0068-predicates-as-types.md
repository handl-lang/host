# ADR-0068: Predicates as Types — Unified Dependent Typing Without a Sublanguage

## Status

Proposed.

Depends on the existing "types as predicates" model documented in the
[Language overview](../../docs/explanation/language-overview.md) (refinement
types, type families, `x is T` as `T(x)`). Extends it with a unified
compile-time/runtime checking strategy and a library of predicates _about_
functions, without introducing a separate total/proof sublanguage.

## Context

### What handl already has

handl treats a type as a predicate: an ordinary value (usually a function)
that applied to a value yields a `type.bool`. `x: T = e` is sugar for
`x = e; assert T(x)`. Refinement types and type families fall out for free:

```handl
Even = (n: type.number) -> n % 2 == 0;
DivisibleBy = (k) -> (n) -> n % k == 0;
m: DivisibleBy(3) = 9;
```

Because types are values, dependent-looking annotations are already
expressible: a return annotation can close over a parameter (a Pi type in
shape), and a record predicate can read earlier fields (a Sigma type in
shape). No new surface syntax is required to _write_ them.

### The open question: when do predicates run, and what can be proven?

The expressive power of a type system is measured by what the checker can
**prove**, not what the user can **write**. Three positions exist on a
spectrum:

1. **Runtime validator.** Predicates run per-value at runtime. No static
   guarantees; just deferred checks. Cheap, honest, weak.
2. **Full dependent type system.** Predicates are proven for _all_ inputs
   ahead of time. Requires a theorem prover: symbolic reasoning about
   functions, induction principles, termination checking, universe
   discipline. This is Idris/Agda/Lean territory — years of work.
3. **Hybrid / partial evaluator.** Predicates are normalized at compile
   time when values are known or normalizable; the checker falls back to a
   runtime check when normalization is blocked (effects, non-termination,
   opaque values).

### Why a declared "total fragment" was considered and rejected

An earlier sketch proposed a `total`-marked sublanguage: structural
recursion only, no effects, a separate proof-carrying typechecker with
induction principles. This is the standard stratification (Idris, Lean's
`meta` vs `prop`, etc.).

It works, but it costs a second language and a second engine, and it forces
users to declare which regime they are in. The insight that avoids it:

> **Totality is not a language mode — it is a property the checker verifies
> on demand, for the expressions it actually needs to normalize.**

The checker normalizes type-position expressions to compare them. To check
`Vec(n + m)` against `Vec(5)` it must evaluate `n + m`. Normalization
requires termination. So the checker runs termination analysis _only on the
expressions being normalized_, not on the whole program. Effects block
normalization and trigger a runtime fallback. No `total` keyword, no
fragment, no bridge rules. "Proven" vs "runtime-checked" becomes a
per-annotation outcome, not a per-region declaration.

### The function-typing gap

A second open question: if types are values (functions), what is the type
_of_ a function? The regress is real but resolves cleanly:

- A type is a predicate — a `bool`-returning function.
- The type of a predicate is `Predicate` — "is this a `bool`-returning
  function?"
- The type of `Predicate` is `Predicate`. Fixed point, not regress.
- No universe levels are required because types are not a foundational
  hierarchy here; they are a subset of values that return `bool`.

What is missing is a library of predicates _about_ functions: signatures,
dependent signatures, purity, totality. `typeof`-style keywords were
considered and rejected — they reintroduce a type-level operation as a
special form, breaking the unification, and they cannot infer signatures
from opaque values soundly. `Fn(A, B)` as a library predicate does the same
job without a keyword.

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
isFunction = (f) -> type.function(f);

Fn = (A, B) -> (f) ->
  isFunction(f) and array.every(samples(A), (x) -> B(f(x)));

DepFn = (A, B) -> (f) ->
  isFunction(f) and array.every(samples(A), (x) -> B(x)(f(x)));

Arity = (n) -> (f) -> isFunction(f) and fn.arity(f) == n;

Pure = (f) -> isFunction(f) and /* effect analysis: no effects */;

Total = (f) -> isFunction(f) and /* termination analysis passes */;
```

Predicates compose with `and`:

```handl
StrictNumToBool = Fn(type.number, type.bool) and Pure and Total;
f: StrictNumToBool = (x) -> x > 0;
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

## Consequences

### Positive

- **No new surface syntax.** Dependent types, refinement types, type
  families, Pi/Sigma shapes, and function signatures are all ordinary
  handl expressions. The language does not grow.
- **One engine.** The partial evaluator is the typechecker. Proofs reuse
  the same normalizer. No second language, no bridge rules.
- **Graduated guarantees.** Static proofs where the checker can normalize,
  runtime checks where it cannot. The user does not declare which regime
  they are in; the checker decides per annotation.
- **Honest framing.** The system is a partial evaluator with a runtime
  backstop and an optional proof layer, not a full dependent type system.
  Claims match what the checker can actually prove.
- **Composable predicates.** `and`/`or` compose types the same way they
  compose boolean expressions. `Fn(A, B) and Pure and Total` is just a
  predicate.

### Negative

- **No erasure.** Predicates are values carried at runtime; the runtime
  backstop has a cost. Optimization (dropping checks for proven
  annotations) is a future pass.
- **Normalization in the typechecker is more work than syntactic
  unification.** It is a small evaluation engine with a termination guard,
  not pattern matching on type expressions.
- **Termination analysis is on us.** Structural recursion to start;
  well-founded recursion is a future extension. Non-structural recursion
  silently falls back to runtime rather than being rejected, which may
  mask bugs the user expected to be caught.
- **`Pure` and `Total` require checker support** that does not yet exist.
  Until they land, function predicates are limited to `isFunction`,
  `Arity`, and sampled `Fn`/`DepFn` — runtime contracts, not static
  proofs.
- **Universe discipline is deferred.** The `Predicate` fixed point avoids
  Girard's paradox in practice because types are not claimed to form a
  foundational hierarchy, but a rigorous treatment of impredicativity is
  not part of this ADR. If paradoxes become reachable, revisit.
- **Full extensional function equality is not provided.** Users who need
  it must write a proof; sampled `FnEq` is a contract, not a proof.

### Build order

1. Partial evaluator as typechecker: normalize type-position expressions,
   runtime fallback on block. (Foundation.)
2. `isFunction`, `Arity`, `Fn(A, B)`, `DepFn(A, B)` as runtime-checked
   library predicates.
3. On-demand termination analysis (structural recursion) for expressions
   the checker normalizes.
4. `array.rec` and `by_eval` / `by_norm` directives; prove length lemmas
   for `array.concat`, `array.map`.
5. `Pure` (effect tracking) and `Total` (termination analysis exposed as a
   predicate); wire proven facts as trusted annotations that eliminate
   runtime checks.

Steps 1–2 are the cheap foundation. Steps 3–5 are the proof layer and are
built when static universals are required.

## Status

Proposed.
