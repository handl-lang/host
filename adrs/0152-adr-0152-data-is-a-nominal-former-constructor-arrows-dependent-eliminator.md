+++
id = "ADR-0152"
title = "ADR-0152: `data` is a nominal former + constructor arrows + dependent eliminator"
status = "open"
priority = "medium"
+++

---
title: '`data` is a nominal former + constructor arrows + dependent eliminator'
type: adr
date: 2026-10-21
status: accepted
applies_to: '`kernel` (`handl-value`), `kit-prelude-lang` (grammar, desugar'
---


# ADR-0152: `data` is a nominal former + constructor arrows + dependent eliminator

## Decision


`data F(i₁…iₖ : I₁…Iₖ) :: C₁ : S₁ | … | Cₙ : Sₙ` elaborates into three
declarations in the ordinary object/arrow world. All three are values
and types of the kind ADR-0106 already defines — no new machinery.

### 1. `F` is a nominal former

```handl
F : Pi(i₁ : I₁, …, Pi(iₖ : Iₖ, Type))
```

`F` binds a `sym` nominal (the former's identity). Applying it does
**not** mint a fresh `sym` per call — it produces the canonical
*family-application* object:

```handl
Vec(2, Int)  ==  { "__former__": vecSym, "indices": [2, Int] }
```

Family equality is then ordinary: same former identity + index
equality. `Vec ≠ List` because `vecSym ≠ listSym` (nominal);
`Vec(2, Int) == Vec(2, Int)` because the applications are equal data.
This also resolves the generic-instantiation memoization question left
open in ADR-0106 §9 (`List<Int> == List<Int>`): a family application
is canonical data, not a per-call symbol mint.

### 2. Each constructor is a nominal arrow into the family

```handl
Cⱼ : Pi(x₁ : A₁, …, xₘ : Aₘ, F(j₁…jₖ))
```

`Cⱼ` binds a `sym` (the constructor's nominal identity — `match`
dispatches on *identity*, not on a `{"tag"}` string). Application
produces a tagged object carrying the ctor sym and the field values:

```handl
Succ(Zero)  ==  { "__ctor__": succSym, "pred": Zero }
```

Membership is *declared*: `type_of(Cⱼ(…))` is `F(j⃗)` by the ctor's
result type — no tag inspection, no chain walk. `Succ(3)` fails to
check because `3 : Nat` fails; `Succ(Zero)` checks and yields `Nat`.

### 3. `F.rec` is the dependent eliminator

```handl
F.rec : Pi(m    : Pi(i⃗ : I⃗, F(i⃗) ~> Type),          -- the motive
          Pi(c₁ : S₁[rec], …,                       -- one case per ctor
          Pi(cₙ : Sₙ[rec],
          Pi(i⃗ : I⃗, Pi(v : F(i⃗), m(i⃗, v))))))
```

where `Sⱼ[rec]` is `Cⱼ`'s signature with a `Pi(x_rec, m(…, field))`
argument inserted after each recursive field. For `Nat`:

```handl
Nat.rec : Pi(m   : Nat ~> Type,
            Pi(cZ : m(Zero),
            Pi(cS : Pi(p : Nat, m(p) ~> m(Succ p)),
            Pi(n : Nat, m(n)))))
```

It **computes** — iota reduction is definitional:

```text
Nat.rec(m, cZ, cS, Zero)    ⟶  cZ
Nat.rec(m, cZ, cS, Succ p)  ⟶  cS(p, Nat.rec(m, cZ, cS, p))
```

`match` is surface sugar: `match v { Zero: z, Succ(p): s p }` lowers to
`Nat.rec((n) ~> <result type>, z, (p, ih) ~> s p, v)` — and the
dependent reading (motive `m` may use the scrutinee `n`) is what
per-arm index refinement already implements in `synth_match_arms`: the
refinement is no longer a checker special-case, it is the type of
`F.rec`'s cases. `absurd` remains: an arm is inhabited only if its
refined index equations are satisfiable.

Iota is also where index equality comes from: `Vec(1+1)` ≡ `Vec(2)`
because `1+1` *evaluates* to `2` under the shared evaluator — the
bespoke index-rewriting solver becomes a normalization question, not a
separate algorithm.

### Positivity and termination move to the checker

Strict positivity is checked when the `data` declaration's ctor
signatures are elaborated — the checker has the signatures, so it owns
the check; the `desugar.data` native's positivity pass is retired (or
kept only as an early-error mirror). Termination of `F.rec` uses the
existing decreasing-argument metadata: `F.rec` is structurally
decreasing on `v`'s ctor spine by construction.


## Why

`desugar.data` (`engine-transform-rewrite/src/native/desugar.rs`) lowers `data Nat :: Zero | Succ(pred : Nat)` into a block of three kinds of bindings:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Removed**: type-as-predicate arrows, `{"tag"}` string dispatch, the
- **Kept**: surface syntax (`data … :: … | …`), `match`/`absurd`
- **New capability**: `F.rec` being dependent means *proofs by
- **Migration**: `lib-type` and consumers minting `{"tag": …}` values
- **Boundary**: `data` declarations inside signatures erase at exec per

## Enforcement

- `kit-prelude-lang` — where this is true.
- `engine-transform-rewrite/src/native/desugar.rs` — where this is true.
- `lib-type` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0152-data-nominal-inductive-families.md`](../design/0152-data-nominal-inductive-families.md).
