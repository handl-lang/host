/-
# Algorithmic Completeness of Invariant Inference

This file formalizes the algorithmic completeness of the invariant inference
algorithm. The `InvariantDomain.lean` file defines the sound floor
(`analyze_spec = True`) and proves it sound but not complete. The
`LoopInvariantDiscovery.lean` file formalizes the body-guarantee seeding
mechanism. This file addresses the question: **is the body-guarantee
invariant the strongest preserved D-invariant?**

## Results

1. **Theorem 1 (`arr_body_guarantee_equals_projected`)**: For a loop body
   `G = arr(phi)`, the body-guarantee invariant equals `sig_guarantee phi`
   projected onto the feedback channel C. This is definitionally true
   (by `rfl`). [Zero sorry]

2. **Theorem 2 (`conjecture2_is_false`)**: The conjecture that the
   body-guarantee invariant is implied by every preserved D-invariant is
   **FALSE** in general, as shown by `conjecture2_is_false` (zero sorry):
   `True` is a preserved D-invariant that does not imply the body-guarantee
   when the body's postcondition is non-trivial. The body-guarantee
   invariant is neither the strongest nor the weakest preserved D-invariant
   in general.

3. **Theorem 3 (`analyze_strongest_sound`)**: The meet (conjunction) of all
   preserved D-invariants is itself preserved by G. This is the soundness
   of the strongest invariant. [Zero sorry]

4. **Theorem 4 (`exists_analyze_strongest_not_in_D`)**: The domain D is
   closed under binary conjunction (`conj` / `DJoin`) but NOT under
   arbitrary conjunctions. Since `analyze_strongest` is the arbitrary
   conjunction of all preserved D-invariants, it may not itself be in D.
   This is the closure problem that makes algorithmic completeness an open
   question. [Stated with `sorry` as a genuine open problem — see the
   theorem's docstring for the precise axiom/lemma that is missing]

## Honest assessment

The body-guarantee invariant is a sound, non-trivial D-invariant that is
strictly stronger than the floor. But it is NOT the strongest preserved
D-invariant in general. The strongest preserved D-invariant
(`analyze_strongest`) is well-defined and sound (Theorem 3), but it may not
be in D (Theorem 4), so the structural analysis algorithm cannot in general
discover it. Algorithmic completeness within D is therefore an open problem.
-/

import ArrowMinimalityLib.ProofGraphs
import ArrowMinimalityLib.InvariantDomain
import ArrowMinimalityLib.StructuralInference
import ArrowMinimalityLib.LoopInvariantDiscovery

open ProofGraphs
open InvariantDomain
open StructuralInference
open LoopInvariantDiscovery

namespace AlgorithmicCompleteness

variable {Atom : Type} {Sig : Ty Atom → Ty Atom → Type}

-- ============================================================================
-- Theorem 1: arr body guarantee equals sig_guarantee projected onto C
-- ============================================================================

/-! ## Theorem 1: arr body guarantee is the sig_guarantee projected

For a loop body `G = arr(phi)` where `phi : Sig (A x C) (B x C)`, the
body-guarantee invariant equals the projection of `sig_guarantee phi` onto
the feedback channel C:

  `loop_invariant_from_body (arr phi) c' = exists v, sig_guarantee phi (pair v c')`

This follows definitionally: `inferred_postcondition (arr phi) =
sig_guarantee _ _ phi` by the structural inference definition, and
`loop_invariant_from_body` is the existential projection of the
postcondition onto C.

This identifies the body-guarantee invariant for single-arrow bodies: it is
exactly the base-arrow guarantee projected onto C, which is the strongest
D-invariant for a single-arrow loop body (since `sig_guarantee phi` is the
finest information the signature provides about `phi`'s output, and the
`exProj` constructor of D preserves this information under projection). -/

/-- For `G = arr(phi)`, the body-guarantee invariant equals the projection
of `sig_guarantee phi` onto C. Proven by `rfl` (definitional). -/
theorem arr_body_guarantee_equals_projected
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (φ : Sig (A ×ₜ C) (B ×ₜ C))
    : loop_invariant_from_body den sig_guarantee (Graph.arr φ) =
      fun c' => ∃ v, sig_guarantee (A ×ₜ C) (B ×ₜ C) φ (Val.pair v c') := by
  -- `loop_invariant_from_body (arr phi) = fun c' => exists v, inferred_postcondition (arr phi) (pair v c')`
  -- and `inferred_postcondition (arr phi) = sig_guarantee _ _ phi` by definition.
  -- Since `phi : Sig (A x C) (B x C)`, the inferred type arguments are `(A x C)` and `(B x C)`.
  rfl

/-- For `G = arr(phi)`, the body-guarantee invariant is in D (via `baseArr`
+ `exProj`). The postcondition `sig_guarantee phi` is in D by `baseArr`,
and the existential projection is in D by `exProj`. -/
def arr_body_guarantee_in_D
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (φ : Sig (A ×ₜ C) (B ×ₜ C))
    : InvariantDomain Atom Sig sig_guarantee
        (loop_invariant_from_body den sig_guarantee (Graph.arr φ)) := by
  -- The postcondition sig_guarantee phi is in D via baseArr
  have dPost : InvariantDomain Atom Sig sig_guarantee (sig_guarantee (A ×ₜ C) (B ×ₜ C) φ) :=
    InvariantDomain.baseArr φ (sig_guarantee (A ×ₜ C) (B ×ₜ C) φ) (by rfl)
  -- The existential projection is in D via exProj
  -- But we need to match the definition of loop_invariant_from_body (arr phi)
  -- which is (by Theorem 1) = fun c' => exists v, sig_guarantee phi (pair v c')
  -- This is exactly exProj applied to dPost
  rw [arr_body_guarantee_equals_projected den sig_guarantee φ]
  exact InvariantDomain.exProj _ dPost

-- ============================================================================
-- Theorem 2: body-guarantee strongest in D -- CONJECTURE (shown FALSE)
-- ============================================================================

/-! ## Theorem 2: body-guarantee strongest in D -- CONJECTURE (FALSE)

The conjecture: if `I` is in D and G preserves I, then I implies the
body-guarantee invariant. This would make the body-guarantee invariant the
meet (weakest) of all preserved D-invariants in the implication lattice.

This is NOT true in general. The counterexample is immediate:
- `True` is in D (via `dTrue`)
- `True` is preserved by any G (trivially: `True c -> True c'` holds)
- But `True` does NOT imply the body-guarantee invariant when the body's
  postcondition is non-trivial (by `body_guarantee_stronger_than_floor`)

So `True` is a preserved D-invariant that does not imply the body-guarantee
invariant, contradicting the conjecture. The counterexample is proven with
zero `sorry` in `conjecture2_is_false` below.

The fundamental reason: the body-guarantee invariant is the existential
projection of the body's postcondition, which is a specific structural
invariant. Other preserved D-invariants (like `True`, or guarantees of
different base arrows) need not imply this projection. The body-guarantee
invariant is sound and non-trivial, but it is neither the strongest nor the
weakest preserved D-invariant in general. -/

/-- COUNTEREXAMPLE (zero sorry): The conjecture that the body-guarantee
invariant is implied by every preserved D-invariant is FALSE in general.

Take `I = True`:
- `True` is in D via `dTrue`
- `True` is preserved by any G (the hypothesis `True c` is trivially true,
  and `True c'` is trivially true)
- But `True` does NOT imply the body-guarantee invariant when the body's
  postcondition is non-trivial (by `body_guarantee_stronger_than_floor`)

So `True` is a preserved D-invariant that does not imply the body-guarantee
invariant, contradicting the conjecture. -/
theorem conjecture2_is_false
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (hnontrivial : ∃ c', ∀ v, ¬ inferred_postcondition den sig_guarantee G (Val.pair v c'))
    : ¬ (∀ (I : Val → Prop) (dI : InvariantDomain Atom Sig sig_guarantee I)
          (hpres : ∀ (a v c c' : Val),
            Eval den G (Val.pair a c) (Val.pair v c') → I c → I c'),
        ∀ c', I c' → loop_invariant_from_body den sig_guarantee G c') := by
  -- The counterexample is I = True:
  -- - True is in D via dTrue
  -- - True is preserved by any G (I c = True, so I c -> I c' is trivial)
  -- - But True does not imply the body-guarantee invariant
  --   (by body_guarantee_stronger_than_floor)
  intro h
  -- Apply the conjecture to I = True, dI = dTrue, with trivial preservation
  have h' := h (fun _ => True) InvariantDomain.dTrue
    (fun _ _ _ _ _ _ => trivial)
  -- h' : forall c', (fun _ => True) c' -> loop_invariant_from_body G c'
  -- which is definitionally: forall c', True -> loop_invariant_from_body G c'
  -- This contradicts body_guarantee_stronger_than_floor
  exact body_guarantee_stronger_than_floor den sig_guarantee G hnontrivial h'

/-- The body-guarantee invariant is preserved by G (trivially from the
hypothesis). This is the trivial case of the conjecture: when I = the
body-guarantee invariant itself (which is in D when the postcondition is
in D), the preservation property holds. [Zero sorry] -/
theorem body_guarantee_implies_itself
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dP : InvariantDomain Atom Sig sig_guarantee
      (inferred_postcondition den sig_guarantee G))
    (hpres : ∀ (a v c c' : Val),
      Eval den G (Val.pair a c) (Val.pair v c') →
      loop_invariant_from_body den sig_guarantee G c →
      loop_invariant_from_body den sig_guarantee G c')
    : ∀ (a v c c' : Val),
      Eval den G (Val.pair a c) (Val.pair v c') →
      loop_invariant_from_body den sig_guarantee G c →
      loop_invariant_from_body den sig_guarantee G c' := by
  exact hpres

-- ============================================================================
-- Theorem 3: analyze_strongest is sound
-- ============================================================================

/-! ## Theorem 3: analyze_strongest is sound

`analyze_strongest` is the meet (conjunction) of all preserved D-invariants:
the strongest invariant (in the implication order) that is implied by every
preserved D-invariant.

  `analyze_strongest G v = forall I in D, preserved(G, I) -> I v`

This is the formal definition of the "join of all preserved D-invariants"
mentioned in the `InvariantDomain.lean` documentation. It is the strongest
invariant that any complete algorithm should compute.

SOUNDNESS: the meet of all preserved D-invariants is itself preserved by G.
If `analyze_strongest G c` holds, then for every preserved D-invariant I,
`I c` holds. Since I is preserved, `I c -> I c'` for any well-typed step,
so `I c'` holds. Hence `analyze_strongest G c'` holds. [Zero sorry]

The preservation hypothesis includes `HasType (Val.pair a c) (A ×ₜ C)`
because invariant soundness theorems (e.g. `loop_invariant_from_body_sound`)
require the input to be well-typed. The `Eval` relation is untyped — it
admits ill-typed inputs (e.g. `eId A a` for any `a`, not just `a : A`) —
so requiring preservation only for well-typed executions is both the
standard approach in type-safe program analysis and the condition under
which the body-guarantee invariant can be shown to be preserved. -/

/-- `analyze_strongest` is the meet (conjunction) of all preserved
D-invariants: the strongest invariant that is implied by every preserved
D-invariant.

  `analyze_strongest G v = forall I in D, preserved_well_typed(G, I) -> I v`

This is the "join of all preserved D-invariants" from the
`InvariantDomain.lean` documentation, now formally defined.

The preservation hypothesis ranges over **well-typed** executions only:
it requires `HasType (Val.pair a c) (A ×ₜ C)` in addition to
`Eval den G (Val.pair a c) (Val.pair v c')`. This is necessary because
the `Eval` relation is untyped (it admits ill-typed inputs), while
invariant soundness theorems require well-typedness. -/
def analyze_strongest
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    : Val → Prop :=
  fun v => ∀ (I : Val → Prop) (dI : InvariantDomain Atom Sig sig_guarantee I),
    (∀ (a v c c' : Val),
      HasType (Val.pair a c) (A ×ₜ C) →
      Eval den G (Val.pair a c) (Val.pair v c') → I c → I c') → I v

/-- SOUNDNESS of `analyze_strongest`: the meet of all preserved D-invariants
is itself preserved by G.

If `analyze_strongest G c` holds, then every preserved D-invariant I
satisfies `I c`. Since I is preserved by G, `I c -> I c'` for any well-typed
step `G . (a,c) ==> (v,c')`. So `I c'` holds for every preserved I, which
means `analyze_strongest G c'` holds.

This is the key soundness property: the strongest invariant is preserved
whenever each constituent is preserved. [Zero sorry] -/
theorem analyze_strongest_sound
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (a v c c' : Val)
    (ha : HasType (Val.pair a c) (A ×ₜ C))
    (e : Eval den G (Val.pair a c) (Val.pair v c'))
    (h : analyze_strongest den sig_guarantee G c)
    : analyze_strongest den sig_guarantee G c' := by
  -- analyze_strongest G c' = forall I dI hpres, I c'
  intro I dI hpres
  -- From h: I c (since I is preserved and in D)
  have hIc := h I dI hpres
  -- From hpres: I c -> I c' (using the well-typed execution e)
  exact hpres a v c c' ha e hIc

/-- `analyze_strongest` implies every preserved D-invariant. This is the
COMPLETENESS direction: any preserved D-invariant is implied by the
strongest invariant. [Zero sorry] -/
theorem analyze_strongest_implies_any_preserved
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (I : Val → Prop) (dI : InvariantDomain Atom Sig sig_guarantee I)
    (hpres : ∀ (a v c c' : Val),
      HasType (Val.pair a c) (A ×ₜ C) →
      Eval den G (Val.pair a c) (Val.pair v c') → I c → I c')
    : ∀ v, analyze_strongest den sig_guarantee G v → I v := by
  intro v h
  exact h I dI hpres

/-- `analyze_strongest` is at least as strong as the floor (`True`):
trivially, since `True` is a preserved D-invariant, `analyze_strongest`
implies `True`. [Zero sorry] -/
theorem analyze_strongest_implies_floor
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    : ∀ v, analyze_strongest den sig_guarantee G v → True := by
  intro v _
  trivial

/-- `True` (the floor) is a preserved D-invariant for any G, so
`analyze_strongest` is at least as strong as the body-guarantee invariant
when the body-guarantee is preserved (which it always is, by
`loop_invariant_from_body_sound`). This connects `analyze_strongest`
to the body-guarantee invariant. [Zero sorry] -/
theorem analyze_strongest_implies_body_guarantee
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    (dBodyInv : InvariantDomain Atom Sig sig_guarantee
      (loop_invariant_from_body den sig_guarantee G))
    : ∀ v, analyze_strongest den sig_guarantee G v →
      loop_invariant_from_body den sig_guarantee G v := by
  intro v h
  -- The body-guarantee invariant is preserved by G for well-typed executions.
  -- By loop_invariant_from_body_sound, I(c') holds UNCONDITIONALLY for every
  -- well-typed step (no hypothesis on I(c) needed), so preservation is trivial.
  have hpres : ∀ (a v c c' : Val),
    HasType (Val.pair a c) (A ×ₜ C) →
    Eval den G (Val.pair a c) (Val.pair v c') →
    loop_invariant_from_body den sig_guarantee G c →
    loop_invariant_from_body den sig_guarantee G c' := by
    intro a v c c' ha e _
    exact loop_invariant_from_body_sound den sig_guarantee sig_sound sig_type_sound
      G dG a v c c' ha e
  -- analyze_strongest implies every preserved D-invariant, including the body-guarantee
  exact h (loop_invariant_from_body den sig_guarantee G) dBodyInv hpres

-- ============================================================================
-- Theorem 4: analyze_strongest may not be in D (the closure problem)
-- ============================================================================

/-! ## Theorem 4: analyze_strongest may not be in D (the closure problem)

`analyze_strongest` is the arbitrary conjunction (meet) of all preserved
D-invariants. The domain D is closed under BINARY conjunction (via the
`conj` constructor / `DJoin`), but NOT under arbitrary (potentially
infinite) conjunctions.

Since `InvariantDomain` is an inductive type, every derivation is FINITE:
it uses finitely many `conj` constructors. But `analyze_strongest` ranges
over ALL preserved D-invariants, which may be infinitely many. Their
conjunction cannot in general be expressed as a finite derivation using
only binary `conj`.

This is the fundamental obstacle to algorithmic completeness: even if we
could compute `analyze_strongest`, it might not be in D, so the structural
analysis algorithm (which only searches within D) could not discover it.

Below we:
1. Prove D is closed under binary conjunction (reference to existing `DJoin`)
2. State the open problem: there exists G for which `analyze_strongest` is
   not in D (with `sorry`)
3. Prove that IF `analyze_strongest` were in D, it would be the complete
   algorithm (zero sorry) -/

/-- D is closed under binary conjunction: if I and J are in D, then
`I ∧ J` is in D. This is the `conj` constructor, also named `DJoin`.

This is the ONLY form of conjunction closure D has: binary, finite. The
`analyze_strongest` definition requires ARBITRARY conjunction (over all
preserved D-invariants), which is not covered by this closure. -/
def D_closed_under_binary_conjunction
    (sig_guarantee : SigGuarantee Atom Sig)
    (I J : Val → Prop)
    (dI : InvariantDomain Atom Sig sig_guarantee I)
    (dJ : InvariantDomain Atom Sig sig_guarantee J)
    : InvariantDomain Atom Sig sig_guarantee (fun v => I v ∧ J v) :=
  DJoin dI dJ

/-- OPEN PROBLEM (with `sorry`): There exists a graph G for which
`analyze_strongest G` is NOT in D.

The obstruction: `analyze_strongest` is the arbitrary conjunction (meet)
of all preserved D-invariants. D is an inductive type with only binary
`conj` (and `dTrue`, `baseArr`, `firstLift`, `leftLift`, `exProj`), so
every D-derivation is FINITE — it uses finitely many constructors. But
`analyze_strongest` ranges over ALL preserved D-invariants, which may be
infinitely many. Their conjunction cannot in general be expressed as a
finite derivation using only binary `conj`.

**What axiom/lemma is missing.** To close this gap one would need either:

1. **An infinitary conjunction constructor** in `InvariantDomain`
   (e.g. `bigConj : (ι → InvariantDomain … (I ι)) → InvariantDomain …
   (fun v => ∀ i, I i v)`), which would make D a complete lattice and
   `analyze_strongest` a member by construction. This changes the domain
   definition and would require re-proving all preservation theorems for
   the new constructor.

2. **A finiteness theorem** showing that for every graph G, the set of
   "atomic" preserved D-invariants (base-arrow guarantees that are
   preserved, up to `exProj`/`firstLift`/`leftLift`) is finite, so their
   conjunction is a finite `conj` derivation. This is plausible for
   finite signatures but fails for signatures with infinitely many base
   arrows (e.g. `Sig A B := Nat → Val`).

3. **A concrete counterexample** with an infinite signature where the
   meet of all preserved D-invariants is provably not in D. The existing
   `analyze_incompleteness` counterexample in `InvariantDomain.lean`
   (empty signature, `I = fun _ => False`) does NOT work here because
   with an empty signature all D-invariants are everywhere-true, so
   `analyze_strongest = True` which IS in D. A counterexample requires a
   signature rich enough that `analyze_strongest` is non-trivial but not
   finitely expressible.

The `sorry` marks this as a genuine open problem: the closure of D under
arbitrary (potentially infinite) meets is not guaranteed by the current
inductive definition, and neither a proof nor a concrete counterexample
is currently known.

Note: this is NOT a universal statement (it is NOT true that
`analyze_strongest G` is never in D -- for `G = idG` with a trivial
signature, `analyze_strongest = True` which IS in D). The statement is
existential: there EXISTS a G for which it fails. -/
theorem exists_analyze_strongest_not_in_D
    : ∃ (Atom : Type) (Sig : Ty Atom → Ty Atom → Type)
       (sig_guarantee : SigGuarantee Atom Sig)
       (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
       (A B C : Ty Atom) (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C)),
      ¬ Nonempty (InvariantDomain Atom Sig sig_guarantee
        (analyze_strongest den sig_guarantee G)) := by
  -- OPEN PROBLEM: closure of D under arbitrary meets.
  -- See the docstring above for the precise axiom/lemma that is missing.
  -- The three candidate approaches are:
  -- (1) Add an infinitary conjunction constructor to InvariantDomain.
  -- (2) Prove finiteness of atomic preserved D-invariants (fails for
  --     infinite signatures).
  -- (3) Construct a concrete counterexample with an infinite signature.
  -- None of these is currently carried out.
  sorry

/-- IF `analyze_strongest G` is in D, then it is the complete algorithm:
it is the strongest preserved D-invariant (it implies every preserved
D-invariant, and it is itself preserved and in D).

This is the conditional completeness result: algorithmic completeness
reduces to the closure of D under arbitrary meets. [Zero sorry] -/
theorem analyze_strongest_complete_if_in_D
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dStrongest : InvariantDomain Atom Sig sig_guarantee
      (analyze_strongest den sig_guarantee G))
    : (∀ (a v c c' : Val),
      HasType (Val.pair a c) (A ×ₜ C) →
      Eval den G (Val.pair a c) (Val.pair v c') →
      analyze_strongest den sig_guarantee G c →
      analyze_strongest den sig_guarantee G c') ∧
    (∀ (I : Val → Prop) (dI : InvariantDomain Atom Sig sig_guarantee I)
      (hpres : ∀ (a v c c' : Val),
        HasType (Val.pair a c) (A ×ₜ C) →
        Eval den G (Val.pair a c) (Val.pair v c') → I c → I c'),
    ∀ v, analyze_strongest den sig_guarantee G v → I v) := by
  -- First conjunct: analyze_strongest is preserved (by analyze_strongest_sound)
  -- Second conjunct: analyze_strongest implies every preserved D-invariant
  -- (by analyze_strongest_implies_any_preserved)
  -- The dStrongest hypothesis witnesses that analyze_strongest is in D,
  -- which is needed for it to be a valid D-invariant (but the two properties
  -- hold regardless of D-membership -- they are properties of the predicate).
  refine ⟨?_, ?_⟩
  · exact analyze_strongest_sound den sig_guarantee G
  · intro I dI hpres v h
    exact analyze_strongest_implies_any_preserved den sig_guarantee G I dI hpres v h

-- ============================================================================
-- Summary
-- ============================================================================

/-
## Summary of results

### Proven results (zero sorry):

1. **`arr_body_guarantee_equals_projected`** (Theorem 1): For `G = arr(phi)`,
   the body-guarantee invariant equals `sig_guarantee phi` projected onto C.
   Proven by `rfl`.

2. **`arr_body_guarantee_in_D`**: The arr body-guarantee invariant is in D
   (via `baseArr` + `exProj`).

3. **`conjecture2_is_false`** (Theorem 2 counterexample): The conjecture
   that the body-guarantee invariant is implied by every preserved
   D-invariant is FALSE. Counterexample: `I = True` is preserved and in D
   but does not imply the body-guarantee when the postcondition is
   non-trivial.

4. **`analyze_strongest_sound`** (Theorem 3): The meet of all preserved
   D-invariants is itself preserved by G.

5. **`analyze_strongest_implies_any_preserved`**: `analyze_strongest`
   implies every preserved D-invariant (completeness direction).

6. **`analyze_strongest_implies_body_guarantee`**: `analyze_strongest`
   implies the body-guarantee invariant (when the latter is in D).

7. **`D_closed_under_binary_conjunction`**: D is closed under binary
   conjunction (reference to `DJoin`).

8. **`analyze_strongest_complete_if_in_D`** (Theorem 4 conditional): IF
   `analyze_strongest` is in D, then it is the complete algorithm (strongest
   preserved D-invariant).

### Conjectures / open problems (with sorry):

1. **`exists_analyze_strongest_not_in_D`** (Theorem 4): Open problem that
   there exists a G for which `analyze_strongest` is not in D. The
   obstruction is that D is closed under binary but not arbitrary meets.

### Honest assessment:

The body-guarantee invariant is a sound, non-trivial D-invariant that is
strictly stronger than the floor. But it is NOT the strongest preserved
D-invariant in general (Theorem 2 is false). The strongest preserved
D-invariant (`analyze_strongest`) is well-defined and sound (Theorem 3),
and it implies every preserved D-invariant including the body-guarantee.
But it may not be in D (Theorem 4), because D is only closed under binary
conjunction, not arbitrary meets. Algorithmic completeness within D is
therefore an open problem, contingent on the closure of D under arbitrary
meets.
-/

end AlgorithmicCompleteness
