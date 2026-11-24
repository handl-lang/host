---
title: 'ADR: Generic Arrow Calculus for Any Dimension — design detail'
type: design
date: 2026-10-16
updated: 2026-10-29
refs: [ADR-0207]
---

> Imported from `kit-data-prelude/project/design/0146-generic-arrow-calculus-any-dimension.md` — ADR-0146 renumbered to ADR-0207 in the host merge.

# ADR: Generic Arrow Calculus for Any Dimension — design detail

Moved verbatim from ADR-0207 (ADR-0161 size budget). The ADR carries the rules;
this page carries the full record.

## Context

We have an arrow-calculus kernel (ADR-0052) and a v0.1 surface syntax (ADR-0195,
ARROW_LANGUAGE_SPEC_V0_1.md) that covers the 1-dimensional case: arrows `A ~> B`,
composition `>>>`, product `***`, fan-out `&&&`, choice `|||`, and feedback `loop`/`rec`.
The frontier calculus (ADR-0202) introduced 2-cell machinery and grades. ADR-0203
showed grades (session types) fit over the arrow calculus as data.

This ADR extends the arrow calculus to **any dimension** with two design rules:

1. **`>>>` is the sole composition operator.** `*` never appears as composition
   syntax. Inside `<>`, composition is still `>>>`; there is no separate
   `compose<>` concept.
2. **One typed expression system.** Values, types, proofs, arrows, grades, and
   algebras are all typed expressions. There are no lambdas and no primitive
   functions — functions emerge from arrows and combinators.

## Decision

### 1. Arrows are primitive; functions emerge

There is no lambda calculus, no `λ`, no primitive functions. The only
computational primitive is the **arrow**. Functions are the degenerate pure
arrows (Hughes's `arr`); everything else — combinators, type constructors,
grade algebras — is expressed with arrows over typed expressions.

Everything is a typed expression: values, types, proofs, arrows, grades,
algebras, etc.

### 2. Two fundamental operators: `()` and `<>`

The surface has exactly two application-like operators:

```text
f(x)       -- invoke/apply an arrow
f<x>       -- enter/specialize a dimension
```

`<>` works on **any expression**, not just arrows:

```text
MyObj<T> : T := z

MyObj<Number>       -- value of type Number
```

### 3. Dimensions contain any expression

A dimension is not restricted to types. It can contain any expression:

```text
f<Int>
f<3>
f<a()>
f<List(Number)>
f<proof>
f<SomeAlgebra>
```

Multiple dimensions:

```text
f<T, U, R>
```

Nested computation:

```text
f<a()>
```

**Composition inside a dimension is still `>>>`** — there is no `compose<a >>> b>`:

```text
f<a >>> b>
```

The `<>` operator is **specialization/entry into a dimension**, not a call.
The `()` operator is **invocation** of an arrow. They are orthogonal:
`f<Int>(x)` specializes `f` to `Int`, then invokes it with `x`.

### 4. Types are expressions; type constructors are arrows

Types are ordinary expressions, and type constructors are arrows:

```text
List   : Type ~> Type
Option : Type ~> Type
Vector : Nat ~> Type
```

Therefore:

```text
List(Int)
Option(String)
Vector(3)
```

**Higher-kinded types come naturally from arrows.** `List` is an arrow from
`Type` to `Type`; applying it with `()` is type construction. There is no
separate HKT mechanism — `Type ~> Type` arrows and `()` are all it takes.

### 5. Generic arrows and objects are arrow families

Generic arrows are arrow families — arrows over dimensions:

```text
identity<T> : T ~> T

map<T, U> : (T ~> U) ~> (List(T) ~> List(U))
```

Generic objects work identically:

```text
MyObj<T> : T := z

MyObj<Number>
```

There is no separate "generics" subsystem: a generic arrow/object is an
arrow/object whose type mentions dimension variables, specialized with `<>`.

### 6. Dependent types: dependency is not restricted to types

Expressions can depend on other expressions:

```text
Vector(n)
buffer<n>
```

Dependency is **not restricted to types** — dimensions can depend on values,
arrows, proofs, grades, etc. This subsumes dependent type theory: the
Pi type is a dimension over the domain (`(x : A) ~> B(x)`), and the Sigma
type is a record with a dependent second component
(`{ fst: A, snd: B(fst) }`). Neither Pi nor Sigma is a new primitive;
they are derived from arrows + records (ADR-0195 `{}` record sugar).

Record `{}` syntax remains structural sugar: construction desugars to
`arr` + wiring; patterns desugar to projections and fan-out (`&&&`).

### 7. Proofs are ordinary typed expressions (Curry–Howard)

```text
P : Type
p : P
```

Proofs are ordinary typed expressions and can be dimensions:

```text
f<p>
```

A proof-dimensioned arrow carries its Curry–Howard witness as a dimension
argument. No separate proof language: proofs are values, types, and
dimensions in the same expression system.

### 8. Graded arrows

Grades annotate arrows:

```text
A ~[r]~> B
```

Examples:

```text
readFile : Path ~[IO]~> String
consume  : Token ~[1]~> Result
```

The notation is pure ASCII: `~`, the bracketed grade, `~>`. It extends the
ungraded arrow `~>` (ADR-0195) with the same bracket form the graded surface
of ADR-0203 already uses (`A ~[Session(!T)]~> B`). No new symbols — only
`~`, `[`, `]`, `>` and alphanumerics.

Grades are **algebraic, not inherently usage counts**. They can represent
usage/resources, effects, permissions, cost, staging, security, or
combinations of these.

**Grade composition uses `>>>` inside the annotation.** Given

```text
A ~[r]~> B
B ~[s]~> C
```

the composite is

```text
A ~[r >>> s]~> C
```

Conceptually, `>>>` combines the grades **according to the selected grade
algebra**. The grade syntax reuses the same composition glyph the arrows
use — one composition operator, everywhere.

**Grades are optional.** They infer from wiring the way types infer
(ADR-0054); an explicit `~[r]~>` annotation constrains, documents, or
resolves ambiguity. The un-graded arrow `A ~> B` is the unit-grade case.

### 9. Grade algebras are ordinary structures

Grade systems are ordinary structures rather than hard-coded language
features:

```text
Usage
Effect
Permission
Cost
```

Each algebra defines how `>>>`, parallel composition, etc. combine its
grades. A grade algebra is itself a typed expression (it can be passed as a
dimension: `f<SomeAlgebra>`). The typechecker computes grade composition
during inference by consulting the selected algebra; `n + m` and `n * m`
arithmetic are methods of the algebra, not built-in operators.

### 10. Arrow combinators are the computational vocabulary

The computational vocabulary remains arrow-based (ADR-0052, ADR-0195):

```text
>>>
first
second
***
&&&
choice
loop
...
```

`>>>` is **always** sequential composition — at the arrow level, inside
grade annotations, and inside dimensions. There is exactly one composition
operator in the entire language.

Derivations (unchanged from the earlier draft of this ADR):

- **`match` derives** from `choice` (`|||`) with pattern dispatch; product
  patterns derive from `&&&` + projections.
- **`loop` is primitive** (Paterson feedback); `rec` is its surface region
  form; `fix` is a library combinator derived from `loop`.
- **`Either(A, B)` and `Array(T)` are library types**, not primitives —
  they are the data arrows process, not new combinators.
- **Pi/Sigma derive** from arrows + records (§6).

### 11. Quantification is consistent with dimension binding

Explicit quantification reuses the same binder structure as dimensions and
arrow parameters:

```text
identity<T> : T ~> T          -- T is a dimension variable (implicit forall)
forall X. (X ~> X)            -- explicit form; same scoping as identity<X>
(x : A) ~> B(x)               -- dependent arrow = Pi, binds like a dimension
forall G. A ~[G]~> B          -- grade quantification, same binder
```

By default dimension/grade variables in signatures are implicitly universally
quantified (Hindley–Milner style, ADR-0054). Explicit `forall` is used for
documentation, nesting, rank-N grades, or resolving ambiguity. One binder
syntax, one scoping rule, everywhere.

## The whole idea

```text
             EXPRESSION
              /       \
            ()         <>
           /             \
       invoke       specialize
        arrow        dimension
                         │
          ┌──────────────┼──────────────┐
        type           value          proof
        grade         algebra          arrow
```

**Core principle:** one typed expression system, primitive arrows, `()` for
computation, `<>` for arbitrary dimensions, and `>>>` for composition.

## Notation summary

| Form | Meaning |
|------|---------|
| `f(x)` | invoke/apply arrow `f` |
| `f<x>` | enter/specialize dimension `x` |
| `f >>> g` | sequential composition (sole composition operator) |
| `A ~> B` | arrow type (un-graded) |
| `A ~[r]~> B` | graded arrow type |
| `A ~[r >>> s]~> C` | grade composition via `>>>` |
| `f<a >>> b>` | composition inside a dimension (still `>>>`) |
| `{ ... }` | record construction/pattern (structural sugar) |
| `first`, `second`, `***`, `&&&`, `choice`, `loop` | arrow combinators |
| `forall X. T` | explicit quantification (same binder as dimensions) |

All arrow glyphs are ASCII — `~`, `[`, `]`, `>` plus alphanumerics. The
graded arrow `~[r]~>` is the ungraded `~>` with a bracketed grade: same
tokens (`token.tilde`, array, `token.arrow`), no new lexer rules.

## Consequences

### Positive

- **One composition operator.** `>>>` composes arrows, grades, and
  computations inside dimensions. Nothing else composes.
- **One expression system.** Types, proofs, grades, algebras, values, and
  arrows are typed expressions; `()` and `<>` operate on all of them
  uniformly.
- **HKT, generics, and dependent types are free.** Type constructors are
  `Type ~> Type` arrows; generics are arrow families with `<>`; dependency is
  unrestricted expression dependency.
- **Grade systems are pluggable.** Algebras are ordinary structures
  (expressions), not hard-coded features — new grade disciplines need no
  language change.
- **No lambda calculus.** Functions emerge from arrows; the kernel stays
  six combinators (ADR-0052).
- **ASCII-only notation.** Every arrow form is `~>`, `~[r]~>`, `>>>` —
  tilde/gt/brackets and alphanumerics only. Lexes with the existing
  `token.arrow`/`token.tilde` rules; no new tokens, no unicode arrows.

### Negative

- **`()` vs `<>` overload is subtle.** Both look like application; tooling,
  error messages, and learners must distinguish invoke vs specialize.
  `f<Int>(x)` chains both, which must parse unambiguously.
- **Grade algebra dispatch adds inference complexity.** The typechecker must
  resolve which algebra governs a `~[r >>> s]~>` composition and consult its
  methods; ambiguity between algebras needs a resolution rule.
- **Unrestricted dependency is undecidable in general.** Dimensions depending
  on arbitrary expressions requires totality/termination checking for
  dimension arguments (ties into ADR-0105 totality_model.hnl).
- **Proofs as dimensions blur term/type/proof levels.** Curry–Howard
  uniformity is powerful but erases the syntactic distinction that helps
  error reporting and editor support.

## Open Questions

1. **Parse/precedence of `f<Int>(x)`.** Confirm `<>` binds tighter than `()`
   and that `f<a >>> b>(x)` parses as specialization then invocation. Extend
   the Pratt grammar (ADR-0053) with `<>` postfix and `()` application at
   the right binding powers.
2. **Which grade algebra governs a composition?** When `r` and `s` come from
   different algebras (e.g. `Effect` and `Permission`), how is the combined
   grade `r >>> s` resolved — product algebra, principal algebra, or error?
3. **Dimension termination.** What checks guard `f<a()>` where `a()` may
   diverge? Propose: dimension arguments must be total (checked by
   `totality_model.hnl`), matching the dependent-type precedent.
4. **Interaction with `Term`/`TermData`.** Do `()` and `<>` introduce new
   `Term` tags (e.g. `Invoke`, `Spec`), or desugar to existing `App`/
   `Arr`/`Op` machinery? Lean toward desugaring; confirm against
   `plugin-engine/src/term.rs`.
5. **Resolved — surface glyph.** The ungraded arrow is `~>` and the graded
   arrow is `A ~[r]~> B`: the ADR-0195 `~>` glyph with the ADR-0203
   bracketed grade. Both are ASCII and both parse today — the Pratt
   special case already builds a `graded_arrow` node for `A ~ [r] ~> B`
   (`plugin-tool-parse-pratt`), so no new tokens or lexer rules are
   needed. Remaining work is consuming the form in `op`/`cap` declaration
   contexts and unifying it with the legacy `Arrow(A, B)` call-style
   signatures in the stdlib packages.

## Relation to existing ADRs

- **ADR-0052** — Arrow Calculus Kernel: the six combinators are the
  dimension-1 case; `>>>` remains the sole kernel composition.
- **ADR-0195** — Arrow Surface Syntax v0.1: `>>>`, `***`, `&&&`, `|||`, `rec`,
  `{}` record sugar are reused; `~>` is the ungraded arrow glyph; `<>`/`()`
  generalize the application story.
- **ADR-0202** — Frontier Calculus: 2-cells are the dimension-2 case of the
  dimension mechanism.
- **ADR-0203** — Session Types as Grade Morphisms: the graded arrow surface
  `A ~[Session(...)]~>`; grade composition via `>>>` generalizes its
  sequential-composition grades.
- **ADR-0053/0054** — parser and typecheck engines: host `<>`/`()` parsing
  and grade inference with algebra dispatch.
- **ADR-0105** — Typechecker is Handl: dimension totality checking lands in
  `totality_model.hnl`; Pi/Sigma derivations in the type models.
- **ARROW_DESUGARING_V0_1.md** — derivation rules (match to `choice`, rec to
  `loop`) that §10 generalizes.

## References

- Hughes — _Generalising Monads to Arrows_ (2000).
- Paterson — _A New Notation for Arrows_ (2001).
- Lindley, Wadler, Yallop — _The Arrow Calculus_ (2008/2010, JFP).
- Sanada — _Algebraic Effects and Handlers for Arrows_ (2024).
- Atkey — _Syntax and Semantics of Quantitative Types_ (2018) — semiring
  grades; grade composition.
- McBride — _Quantitative types and linearity_ — grades as algebras over
  usage.
- de Moura et al. — Lean4 — dependent dimensions and total expression
  dependencies.
