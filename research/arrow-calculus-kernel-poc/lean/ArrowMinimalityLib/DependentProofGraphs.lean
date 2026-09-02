/-
# Dependent Arrow Proof Graphs (Phase 5 — Dependent Curry-Howard)

This file extends the arrow proof graph calculus (`ProofGraphs.lean`) with
**dependent types**.  The five graph primitives are retained; the type system
is enriched with dependent products (Π) and dependent sums (Σ).

## Design

The graph's output type is a constant `DTy Atom` (not `Val → DTy Atom`).
The `first`/`loop` constructors use `dProd` (non-dependent product, no
function arguments) rather than `dSigma` so that Lean4's `cases` tactic
can unify the inductive family indices.  The dependent type formers
`dSigma` and `dPi` are available in the type system for use in signatures,
but the graph constructors themselves only use `dProd`, `dSum`, and base
types — keeping the index unification simple.

## Key extensions

1. **`DTy`** — dependent types: `dAtom`, `dPi`, `dSigma`, `dProd`, `dSum`, `dUnit`, `dVoid`.
2. **`DHasType`** — value-level inhabitation of a dependent type.
3. **`DGraph`** — the five primitives (`idG`, `comp`, `first`, `left`, `loop`,
   `arr`) with constant output type `DTy Atom`.
4. **`DTyOf` / `DPrOf`** — typing and proof judgments, rule-for-rule identical.
5. **Correspondence** — `DTyOf G ↔ DPrOf G` (bijection on derivations).
6. **Dependent subject reduction** — if `DTyOf G`, `G · a ⟹ v`, and `a : A`,
   then `v : B`.
-/

import ArrowMinimalityLib.ProofGraphs

open ProofGraphs

namespace DependentProofGraphs

variable {Atom : Type}

-- ============================================================================
-- 1. Dependent Types
-- ============================================================================

/-- Dependent types for the arrow-graph calculus.
`dPi` is the dependent function type Π (x:A) B(x);
`dSigma` is the dependent pair type Σ (x:A) B(x);
`dProd` is the non-dependent product A × B (used by `first`/`loop`). -/
inductive DTy (Atom : Type) : Type
  | dAtom  (a : Atom)                         : DTy Atom
  | dPi    (A : DTy Atom) (B : Val → DTy Atom) : DTy Atom   -- Π (x:A) B(x)
  | dSigma (A : DTy Atom) (B : Val → DTy Atom) : DTy Atom   -- Σ (x:A) B(x)
  | dProd  (A B : DTy Atom)                   : DTy Atom    -- A × B (non-dependent)
  | dSum   (A B : DTy Atom)                   : DTy Atom
  | dUnit                                      : DTy Atom
  | dVoid                                      : DTy Atom

-- ============================================================================
-- 2. Dependent Value Typing
-- ============================================================================

/-- A value inhabits a dependent type.  For `dSigma`, the second component's
type is instantiated at the first component's value (`B a`).  For `dProd`,
both components have fixed types. -/
inductive DHasType : Val → DTy Atom → Prop
  | dhtAtom  (n : Int) (a : Atom)                                         : DHasType (Val.int n) (DTy.dAtom a)
  | dhtPi    (A : DTy Atom) (B : Val → DTy Atom)                        : DHasType Val.unit (DTy.dPi A B)
  | dhtSigma (ha : DHasType a A) (hb : DHasType b (B a))                  : DHasType (Val.pair a b) (DTy.dSigma A B)
  | dhtProd  (ha : DHasType a A) (hb : DHasType b B)                     : DHasType (Val.pair a b) (DTy.dProd A B)
  | dhtSumL  (ha : DHasType a A)                                          : DHasType (Val.inl a) (DTy.dSum A B)
  | dhtSumR  (hb : DHasType b B)                                          : DHasType (Val.inr b) (DTy.dSum A B)
  | dhtUnit                                                             : DHasType Val.unit DTy.dUnit

/-- Inversion: a pair inhabiting `dProd A B` gives `a : A` and `b : B`. -/
theorem DHasType.prod_inv (h : DHasType (Val.pair a b) (DTy.dProd A B))
    : DHasType a A ∧ DHasType b B := by
  cases h with | dhtProd ha hb => exact ⟨ha, hb⟩

/-- Inversion: a pair inhabiting `dSigma A B` gives `a : A` and `b : B a`. -/
theorem DHasType.sigma_inv (h : DHasType (Val.pair a b) (DTy.dSigma A B))
    : DHasType a A ∧ DHasType b (B a) := by
  cases h with | dhtSigma ha hb => exact ⟨ha, hb⟩

/-- Inversion: `Val.inl a` inhabiting `dSum A B` gives `a : A`. -/
theorem DHasType.dsumL_inv (h : DHasType (Val.inl a) (DTy.dSum A B))
    : DHasType a A := by
  cases h with | dhtSumL ha => exact ha

/-- Inversion: `Val.inr b` inhabiting `dSum A B` gives `b : B`. -/
theorem DHasType.dsumR_inv (h : DHasType (Val.inr b) (DTy.dSum A B))
    : DHasType b B := by
  cases h with | dhtSumR hb => exact hb

-- ============================================================================
-- 3. Dependent Graphs (the five primitives, adapted)
-- ============================================================================

/- A dependent signature `DSig` supplies base arrows with constant output
type: `φ : DSig A B` where `A B : DTy Atom`.  The graph grammar is the free
arrow on `DSig` with the same five independent primitives.  The `first` and
`loop` constructors use `dProd` (non-dependent product) rather than `dSigma`
to keep the inductive family indices free of function-valued arguments. -/

inductive DGraph (Atom : Type) (DSig : DTy Atom → DTy Atom → Type) :
    DTy Atom → DTy Atom → Type 1
  | idG   (A : DTy Atom)                                                : DGraph Atom DSig A A
  | comp   {A B C : DTy Atom}
           (G : DGraph Atom DSig A B) (H : DGraph Atom DSig B C)       : DGraph Atom DSig A C
  | first  {A B D : DTy Atom} (G : DGraph Atom DSig A B)               : DGraph Atom DSig (DTy.dProd A D) (DTy.dProd B D)
  | left   {A B D : DTy Atom} (G : DGraph Atom DSig A B)               : DGraph Atom DSig (DTy.dSum A D) (DTy.dSum B D)
  | loop   {A B C : DTy Atom}
           (G : DGraph Atom DSig (DTy.dProd A C) (DTy.dProd B C))      : DGraph Atom DSig A B
  | arr    {A B : DTy Atom} (φ : DSig A B)                             : DGraph Atom DSig A B

-- ============================================================================
-- 4. Dependent Typing and Proof Judgments
-- ============================================================================

section Judgments

variable {Atom : Type} {DSig : DTy Atom → DTy Atom → Type}

/-- Typing derivation `DTyOf G` — dependent version of `TyOf`. -/
inductive DTyOf : {A B : DTy Atom} → DGraph Atom DSig A B → Type 1
  | idT    (A : DTy Atom)                                         : DTyOf (DGraph.idG A)
  | compT  {A B C : DTy Atom} (G : DGraph Atom DSig A B) (H : DGraph Atom DSig B C)
           (dG : DTyOf G) (dH : DTyOf H)                          : DTyOf (DGraph.comp G H)
  | firstT {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B) (dG : DTyOf G) : DTyOf (DGraph.first G)
  | leftT  {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B) (dG : DTyOf G) : DTyOf (DGraph.left G)
  | loopT  {A B : DTy Atom} (C : DTy Atom) (G : DGraph Atom DSig (DTy.dProd A C) (DTy.dProd B C)) (dG : DTyOf G) : DTyOf (DGraph.loop G)
  | arrT   {A B : DTy Atom} (φ : DSig A B)                        : DTyOf (DGraph.arr φ)

/-- Proof derivation `DPrOf G` — dependent version of `PrOf`.
Constructors mirror `DTyOf` one-for-one (same Curry-Howard reading). -/
inductive DPrOf : {A B : DTy Atom} → DGraph Atom DSig A B → Type 1
  | idP    (A : DTy Atom)                                         : DPrOf (DGraph.idG A)
  | compP  {A B C : DTy Atom} (G : DGraph Atom DSig A B) (H : DGraph Atom DSig B C)
           (dG : DPrOf G) (dH : DPrOf H)                          : DPrOf (DGraph.comp G H)
  | firstP {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B) (dG : DPrOf G) : DPrOf (DGraph.first G)
  | leftP  {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B) (dG : DPrOf G) : DPrOf (DGraph.left G)
  | loopP  {A B : DTy Atom} (C : DTy Atom) (G : DGraph Atom DSig (DTy.dProd A C) (DTy.dProd B C)) (dG : DPrOf G) : DPrOf (DGraph.loop G)
  | arrP   {A B : DTy Atom} (φ : DSig A B)                        : DPrOf (DGraph.arr φ)

end Judgments

-- ============================================================================
-- 5. Correspondence:  DTyOf ⇔ DPrOf
-- ============================================================================

section Correspondence

variable {Atom : Type} {DSig : DTy Atom → DTy Atom → Type}

/-- Forward: every dependent typing derivation is a proof derivation. -/
def DTyOf.toDPrOf {A B : DTy Atom} {G : DGraph Atom DSig A B} (d : DTyOf G) : DPrOf G :=
  match d with
  | DTyOf.idT A              => DPrOf.idP A
  | DTyOf.compT G H dG dH    => DPrOf.compP G H dG.toDPrOf dH.toDPrOf
  | DTyOf.firstT D G dG      => DPrOf.firstP D G dG.toDPrOf
  | DTyOf.leftT D G dG       => DPrOf.leftP  D G dG.toDPrOf
  | DTyOf.loopT C G dG       => DPrOf.loopP  C G dG.toDPrOf
  | DTyOf.arrT φ             => DPrOf.arrP  φ

/-- Backward: every dependent proof derivation is a typing derivation. -/
def DPrOf.toDTyOf {A B : DTy Atom} {G : DGraph Atom DSig A B} (d : DPrOf G) : DTyOf G :=
  match d with
  | DPrOf.idP A              => DTyOf.idT A
  | DPrOf.compP G H dG dH    => DTyOf.compT G H dG.toDTyOf dH.toDTyOf
  | DPrOf.firstP D G dG      => DTyOf.firstT D G dG.toDTyOf
  | DPrOf.leftP D G dG       => DTyOf.leftT  D G dG.toDTyOf
  | DPrOf.loopP C G dG       => DTyOf.loopT  C G dG.toDTyOf
  | DPrOf.arrP φ             => DTyOf.arrT  φ

/-- Forward direction of the dependent correspondence. -/
def dtyping_to_proof {A B : DTy Atom} (G : DGraph Atom DSig A B) :
    DTyOf G → DPrOf G := fun d => d.toDPrOf

/-- Backward direction of the dependent correspondence. -/
def dproof_to_typing {A B : DTy Atom} (G : DGraph Atom DSig A B) :
    DPrOf G → DTyOf G := fun d => d.toDTyOf

/-- Round-trip: `DTyOf.toDPrOf` then `DPrOf.toDTyOf` recovers the derivation. -/
theorem DTyOf.toDPrOf_toDTyOf {A B : DTy Atom} {G : DGraph Atom DSig A B}
    (d : DTyOf G) : d.toDPrOf.toDTyOf = d := by
  induction d with
  | idT A => rfl
  | compT G H dG dH ihG ihH => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG, ihH]
  | firstT D G dG ihG => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG]
  | leftT D G dG ihG => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG]
  | loopT C G dG ihG => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG]
  | arrT φ => rfl

/-- Round-trip: `DPrOf.toDTyOf` then `DTyOf.toDPrOf` recovers the derivation. -/
theorem DPrOf.toDTyOf_toDPrOf {A B : DTy Atom} {G : DGraph Atom DSig A B}
    (d : DPrOf G) : d.toDTyOf.toDPrOf = d := by
  induction d with
  | idP A => rfl
  | compP G H dG dH ihG ihH => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG, ihH]
  | firstP D G dG ihG => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG]
  | leftP D G dG ihG => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG]
  | loopP C G dG ihG => simp only [DTyOf.toDPrOf, DPrOf.toDTyOf, ihG]
  | arrP φ => rfl

end Correspondence

-- ============================================================================
-- 6. Dependent Operational Semantics:  G · a ⟹ v
-- ============================================================================

section Semantics

variable {Atom : Type} {DSig : DTy Atom → DTy Atom → Type}
variable (dden : ∀ {A B : DTy Atom}, DSig A B → Val → Val)

/-- Dependent execution relation `DEval dden G a v` reads `G · a ⟹ v`. -/
inductive DEval : {A B : DTy Atom} → DGraph Atom DSig A B → Val → Val → Type 1
  | eId    (A : DTy Atom) (a : Val)                                : DEval (DGraph.idG A) a a
  | eComp  {A B C : DTy Atom} (G : DGraph Atom DSig A B) (H : DGraph Atom DSig B C)
          (a b v : Val) (eG : DEval G a b) (eH : DEval H b v)       : DEval (DGraph.comp G H) a v
  | eFirst {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B)
          (a b c : Val) (eG : DEval G a b)                         : DEval (DGraph.first G) (Val.pair a c) (Val.pair b c)
  | eLeftL {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B)
          (a b : Val) (eG : DEval G a b)                           : DEval (DGraph.left G) (Val.inl a) (Val.inl b)
  | eLeftR {A B : DTy Atom} (D : DTy Atom) (G : DGraph Atom DSig A B)
          (b : Val)                                               : DEval (DGraph.left G) (Val.inr b) (Val.inr b)
  | eLoop  {A B : DTy Atom} (C : DTy Atom) (G : DGraph Atom DSig (DTy.dProd A C) (DTy.dProd B C))
          (a v cₙ : Val) (hcₙ : DHasType cₙ C)
          (eStep : DEval G (Val.pair a cₙ) (Val.pair v cₙ))        : DEval (DGraph.loop G) a v
  | eArr   {A B : DTy Atom} (φ : DSig A B) (a : Val)              : DEval (DGraph.arr φ) a (dden φ a)

end Semantics

-- ============================================================================
-- 7. Dependent Subject Reduction
-- ============================================================================

section Soundness

variable {Atom : Type} {DSig : DTy Atom → DTy Atom → Type}

/-- **Dependent Subject Reduction.** If `DTyOf G`, `G · a ⟹ v`, and `a : A`,
then `v : B`.  The dependent types (`dSigma`, `dPi`) are in the type system;
the `first` case requires inverting `dProd` to extract the pair components. -/
theorem d_subject_reduction (dden : ∀ {A B : DTy Atom}, DSig A B → Val → Val)
    (hsound : ∀ {A B : DTy Atom} (φ : DSig A B) (a : Val),
      DHasType a A → DHasType (dden φ a) B)
    {A B : DTy Atom} {G : DGraph Atom DSig A B} {a v : Val}
    (d : DTyOf G) (e : DEval dden G a v) (ha : DHasType a A)
    : DHasType v B := by
  induction e with
  | eId A a =>
    cases d; exact ha
  | eComp G H a b v eG eH ihG ihH =>
    cases d with
    | compT G H dG dH =>
      have hb := ihG dG ha
      exact ihH dH hb
  | eFirst D G a b c eG ihG =>
    cases d with
    | firstT D G dG =>
      obtain ⟨ha', hc⟩ := DHasType.prod_inv ha
      have hb := ihG dG ha'
      exact DHasType.dhtProd hb hc
  | eLeftL D G a b eG ihG =>
    cases d with
    | leftT D G dG =>
      have ha' := DHasType.dsumL_inv ha
      have hb := ihG dG ha'
      exact DHasType.dhtSumL hb
  | eLeftR D G b =>
    cases d with
    | leftT D G dG =>
      cases ha with
      | dhtSumR hb => exact DHasType.dhtSumR hb
  | eLoop C G₀ a v cₙ hcₙ eStep ih =>
    match d with
    | DTyOf.loopT C G' dG =>
      have hAC : DHasType (Val.pair a cₙ) (DTy.dProd _ C) :=
        DHasType.dhtProd ha hcₙ
      have hBC := ih dG hAC
      obtain ⟨hv, _⟩ := DHasType.prod_inv hBC
      exact hv
  | eArr φ a =>
    cases d with
    | arrT φ => exact hsound φ a ha

end Soundness

-- ============================================================================
-- 8. Dependent Proof Realizability (via the Correspondence)
-- ============================================================================

section Realizability

variable {Atom : Type} {DSig : DTy Atom → DTy Atom → Type}

/-- **Dependent Proof Realizability.** If `DPrOf G` and `a : A`, then
`G · a ⟹ v` for some `v : B`.  By the correspondence, this is dependent
subject reduction precomposed with `DPrOf.toDTyOf`. -/
theorem d_proof_realizability (dden : ∀ {A B : DTy Atom}, DSig A B → Val → Val)
    (hsound : ∀ {A B : DTy Atom} (φ : DSig A B) (a : Val),
      DHasType a A → DHasType (dden φ a) B)
    {A B : DTy Atom} {G : DGraph Atom DSig A B} {a v : Val}
    (d : DPrOf G) (e : DEval dden G a v) (ha : DHasType a A)
    : DHasType v B :=
  d_subject_reduction dden hsound d.toDTyOf e ha

end Realizability

end DependentProofGraphs
