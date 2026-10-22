# ADR-0152: `data` is a nominal former + constructor arrows + dependent eliminator

- Status: Accepted
- Date: 2026-10-21
- Repos: `kernel` (`handl-value`), `kit-prelude-lang` (grammar, desugar
  rules), `engine-transform-rewrite` (`desugar.data` native),
  `engine-typecheck-nbe`, `lib-type`

## Context — what `data` does today and why it is confusing

`desugar.data` (`engine-transform-rewrite/src/native/desugar.rs`) lowers
`data Nat :: Zero | Succ(pred : Nat)` into a block of three kinds of
bindings:

```handl
-- 1. THE TYPE ITSELF IS A PREDICATE ARROW
Nat := __v ~> loop(...)                      -- folds over v, checks tags

-- 2. constructors are arrows producing string-tagged objects
Zero := {"tag": "Zero"}
Succ := pred ~> {"tag": "Succ", "pred": pred}

-- 3. the eliminator is a first-order fold
Nat.rec := (motive, cZero, cSucc) ~> <fold, decreasing on `pred`>
```

This encoding has three conflations that make it hard to reason about:

1. **The type is a runtime predicate, not a type.** `Nat` is an arrow
   that inspects `{"tag": ...}` objects. "Being a Nat" (a static fact)
   is conflated with "passing a tag-checking fold" (a runtime test).
   `x : Nat` cannot mean "`Nat`'s parent chain contains `x`" — it means
   "calling `Nat(x)` returned true", an entirely different notion that
   only works for tag-shaped values and says nothing statically.
2. **Constructor identity is a string.** `{"tag": "Succ"}` compares
   structurally; the nominal `sym` machinery (ADR-0106) does not apply,
   so `match` dispatch is a string compare and two unrelated `data`
   declarations with a same-named ctor collide in tag space.
3. **The eliminator is not dependent.** `Nat.rec` folds to a *value*;
   its motive is not `Nat ~> Type`. Induction — "to prove `P(n)` for
   all `n`, supply `P(Zero)` and `P(n) ~> P(Succ n)`" — is not
   expressible. The checker's dependent `match` index-refinement
   (`synth_match_arms`, `eq_n` path witnesses) has *no corresponding
   value-level eliminator*: it is a bespoke checker special-case, which
   is also why index normalization (`Vec(1+1)` vs `Vec(2)`) needs a
   custom rewriting solver instead of falling out of computation.
4. **Strict positivity lives in the desugar native**, a syntactic
   pre-check the checker must trust — the checker itself never sees
   constructor signatures in a form it can verify.

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

## Worked example — `data Nat :: Zero | Succ(pred : Nat)`

| Surface | Elaborates to | Checks |
|---|---|---|
| `Nat` | `natSym : Type` | `Nat : Type` |
| `Zero` | `{"__ctor__": zeroSym} : Nat` | `Zero : Nat` |
| `Succ` | `Pi(pred : Nat, Nat)` | `Succ(Zero) : Nat`, `Succ(3)` ✗ |
| `match n { Zero: 0, Succ(p): p }` | `Nat.rec((_)~>Nat, 0, (p,_)~>p, n)` | `: Nat` |
| `plus0 : Pi(n:Nat, Path(n+0, n))` | `Nat.rec((n)~>Path(n+0,n), refl, (p,ih)~>…, n)` | a *proof* by induction — the same eliminator |

`match` on an indexed family (`Vec(n, a)` scrutinee) needs no new rule:
checking `Vec.rec`'s `cVNil : m(0, VNil)` case unifies the scrutinee's
index with `0` for that arm — the `eq_n` witness is the transport the
type of `m` demands, not a bespoke binding.

## Membership vs subtyping caveat (normative)

Under `sym`'s single parent chain, `x : T` and `T <: U` must be read
differently or `Zero instanceof Type` comes out true (Zero→Nat→Int→…→Type):

- `x : T` — the **head** of `x`'s nominal parent chain is `T`
  (direct membership; for ctor values this is the declared family).
- `T <: U` — `U` occurs in `T`'s parent chain **restricted to type
  nodes** (subtyping).
- `x : T` then propagates through supertypes: `x : T` and `T <: U`
  gives `x : U` — the judgment the checker implements, not a single
  `instanceof` walk.

## Consequences

- **Removed**: type-as-predicate arrows, `{"tag"}` string dispatch, the
  `Nat := __v ~> loop` membership encoding, the bespoke
  match-index-refinement special-casing, the index-normalization
  solver's raison d'être (it becomes ordinary normalization).
- **Kept**: surface syntax (`data … :: … | …`), `match`/`absurd`
  surface, `Type.rec` naming, strict-positivity enforcement (moved to
  the checker), decreasing-argument termination metadata.
- **New capability**: `F.rec` being dependent means *proofs by
  induction are ordinary arrows* — `Pi(n:Nat, Path(n+0,n))` is
  inhabited by `Nat.rec` with motive `(n) ~> Path(n+0, n)`. Proofs
  require nothing beyond `data` + `Pi` + `Path`.
- **Migration**: `lib-type` and consumers minting `{"tag": …}` values
  for `data` types move to ctor-sym objects; the `__tag__` bridge
  (ADR-0106 conformance note) covers the transition period.
- **Boundary**: `data` declarations inside signatures erase at exec per
  ADR-0151 — the former/ctor/rec bindings are what exec sees; the
  family/ctor *types* are typecheck-owned metadata.
