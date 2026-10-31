+++
id = "ADR-0154"
title = "ADR-0154: Typecheck semantics — identity, equality, universes, literals, erasure"
status = "open"
priority = "medium"
+++

---
title: Typecheck semantics — identity, equality, universes, literals, erasure
type: adr
date: 2026-10-22
status: accepted
applies_to: '`kernel` (`handl-value`, `handl-language`), `kit-prelude-lang`,'
---

# ADR-0154: Typecheck semantics — identity, equality, universes, literals, erasure

  `engine-typecheck-nbe`, `lib-type`

## Context

ADR-0106 made `sym()`/`sym(A)` the nominal-identity primitive and
declared the sym parent graph the subtyping relation; ADR-0152/0153
made `data`, `~[n]~>`, and `f<…>` one indexed-former mechanism. Review
of those decisions surfaced five semantics questions that ADR-0106 left
open or conflated. This ADR settles them. None adds machinery — each
pins down how the existing primitives are *read*.

## Decision

### 1. Deterministic `sym` seeding for declared types

`sym()` mints fresh identity per evaluation — correct for locals, but
a declared former minted at module-eval time breaks equality across
import paths and processes: `Vec` loaded twice is two identities.

Exported/declared formers mint **deterministically**: `sym` seeded by
the module-qualified name (`sym("vec.Vec")` — or the content hash once
ADR-0010 addressing lands). The seed is the identity; minting the same
name is idempotent, not fresh. Plain `sym()` stays non-deterministic
for locals. This is what lets nominal types cross the artifact cache
(ADR-0065), separate compilation, and FFI boundaries.

### 2. Membership and subtyping are two links, not one chain

A single parent chain conflates `x : T` (membership) with `T <: U`
(subtyping) — transitively it makes `Zero instanceof Type` true, since
`Type` tops every type's chain. The split:

- values carry a `type` link → their type's sym;
- types carry a `parent` link → their supertype sym;
- `x : T` reads `x.type`; `T <: U` walks `parent`; `instanceof` =
  `type` link then parent-walk.

`x : T` propagates through supertypes (`x : Nat`, `Nat <: Int` ⇒
`x : Int`) as a *judgment* the checker derives, not a single walk.

### 3. Equality law — structural by default, `sym` opts out

`==` is structural equality over immutable data: two objects with the
same content are the same value, everywhere, in every module. `sym` is
the *only* escape — identical shape, distinct identity.

Consequently: record/tuple/`Pi`/`Sigma` types and former applications
(`Vec(2,Int)` = `{former, args}`) compare structurally; declared
`data` formers compare nominally. `Nat ≠ OtherNat` even when both are
`Zero | Succ` — nominal heads, structural bodies.

### 4. Universes are indexed: `Type(i) : Type(i+1)`

`Type : Type` admits Girard's paradox — with proofs as real values,
every `Path(a,b)` becomes inhabitable and equations lose meaning. The
paradox is a *checker* property (the kernel sees only syms), so the fix
is a checker rule:

- `Type` is an indexed former: `Type(i) : Type(i+1)`; bare `Type` =
  `Type(0)`. `Obj` is the kernel world, not a type — `Type(i) : Obj`,
  and `Obj` has no level to check.
- `Pi`'s level is `max(domain, codomain)` — the existing
  `universe_max` judgment op. `Pi(A : Type, A ~> A)` is `Type(1)` by
  construction; self-quantification is structurally unwritable.
- Levels infer like other indices — invisible in ordinary code; only
  type-quantifying declarations surface them.

### 5. Numeric literals are constrained metas

A literal (`1`, `3.14`) synthesizes a metavariable constrained to a
descendant of `type.number`/`type.float` — it unifies with the first
concrete numeric type it meets (`1 : Float` solves `?l := Float`, not
via `Nat <: Float`, which is false — Nat's parent is Int). Unresolved
metas default: integer literals → `Nat`, fraction literals → `Float`.
`-1` is unary negation applied to a `Nat` — the *application* is
`Int`-typed. No literal-specific typing rules; reuse the meta store.

### 6. Erasure is a pipeline property, verified by an artifact lint

No quantity system on binders (QTT-style `0/1/ω` is rejected — the
`~[n]~>` bracket is dimension, per ADR-0153). Instead:

- Elaboration consumes all type-level content — metas solved, indices
  normalized, `match` lowered to `F.rec`, annotations erased per
  ADR-0151. `exec-transform` dead-code passes then drop anything only
  higher cells referenced.
- **Invariant**: the final runnable artifact contains only 1-arrows +
  runtime values — no former applications, no metas, no undecided
  indices. A lint pass asserts this post-pipeline: stuck-but-referenced
  terms are *live* code DCE cannot remove — the lint is what turns
  "stuck" from silent unsafety into a diagnostic (pipeline bug or a
  needed runtime backstop).
- Runtime-minted cells (`eval`/`resolve` constructing a law or type at
  runtime) are **allowed** — laws-as-values is a feature; erasure is
  best-effort for them, not guaranteed.

### 7. `types.yaml` is a dispatch table, not the spec

The honest split: `lang.types[kind]` maps node kinds to named judgment
ops (`synth_*`/`check_*`); the ops — implemented in Rust, becoming
`.hnl` when the ADR-0105 elaborator returns — are the semantic core
and the normative spec. Simple kinds keep combinator schemes; the file
never pretends to be the checker.

## Consequences

- **Seeding and links are phase-1 work** — they shape `handl-value`'s
  `Val` domain; cheap now, structural rewrites later.
- **`Type(i)` inference** reuses the existing `universe_level`/`max`
  ops; no new judgment machinery.
- **Literal metas** need the constraint field on metas ("must solve to
  a descendant of X") — one small addition to the meta store.
- **The lint** rides `exec-transform` passes already in place; it is a
  verification pass, not a typing discipline.
- Together with 0152/0153 this closes the semantics: syms + indexed
  formers + one equality law + stratified `Type` + pipeline erasure.

## References

- ADR-0106 (sym kernel), §14 (subtyping), §18 (runtime/static
  convergence) — refined by §§1–2 here
- ADR-0152 (`data` formers), ADR-0153 (dimension index)
- ADR-0151 (annotation erasure at exec), ADR-0032 (proof erasure)
- Girard's paradox (why `Type : Type` is excluded); Kovács smalltt
  (NbE conventions the `Val` domain follows)
