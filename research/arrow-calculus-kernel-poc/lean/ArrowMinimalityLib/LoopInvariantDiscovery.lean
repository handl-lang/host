/-
# Loop Invariant Discovery: Body Guarantee Seeding and Fixpoint Iteration

## What this file proves

The `StructuralInference.lean` file proves that `inferred_postcondition` is
sound but returns `True` for `loop` (the conservative floor). The
`InvariantDomain.lean` file defines the lattice, transfer function, and
fixpoint, but the transfer is also the floor (`True`).

This file formalizes the **actual loop invariant discovery** that the Rust
implementation performs:

1. **Body guarantee seeding.** The loop body `G : (A×C) ⇝ (B×C)` has a
   structural postcondition `P = inferred_postcondition G` (non-trivial for
   `arr(φ)`, `comp`, etc.). The projection of `P` onto the feedback channel
   `C` is a candidate invariant `I`.

2. **Soundness of the seeded invariant.** The candidate `I` is preserved by
   every step of the loop body — in fact, `I(c')` holds **unconditionally**
   for every output `c'`, because `P` holds of the full output `(v, c')` by
   `inferred_postcondition_sound_full`. This is the "optimistic propagation"
   approach: the body's guarantee establishes the invariant on the output,
   regardless of the input.

3. **Post-fixpoint property.** The transfer function `wp(G, I)` asks: what
   must hold of `c` (the input feedback) to ensure `I(c')` (the output
   feedback)? Since `I(c')` holds unconditionally, `wp(G, I) = True`. The
   fixpoint iteration `I_{n+1} = I_n ∧ wp(G, I_n)` reaches a fixpoint at
   `I_0` in one step: `I_1 = I_0 ∧ True = I_0`.

4. **Strictly stronger than the floor.** For non-trivial bodies (where the
   postcondition constrains the feedback channel), the seeded invariant is
   strictly stronger than `True`.

5. **Initialization via first step.** The invariant is established by the
   first execution of the loop body (optimistic propagation), not by
   requiring it to hold of the initial feedback value.

## Relationship to the Rust implementation

The Rust `infer_r` function does:
- Compute the body's postcondition (structural inference)
- Project it onto the feedback channel
- Seed the fixpoint with this projection
- Iterate: `I_{n+1} = I_n ∧ wp(body, I_n)`
- The fixpoint converges when `wp(body, I_n) = True` (which happens
  immediately for the body-guarantee seeding, since the guarantee is
  unconditional)

The Rust implementation additionally does **WP branching through Choice**
and **structural pattern recognition** (side-channel loops, accumulator
loops, filter loops). These are extensions of the basic mechanism formalized
here; the body-guarantee seeding is the foundation.
-/

import ArrowMinimalityLib.ProofGraphs
import ArrowMinimalityLib.RefinementProofGraphs
import ArrowMinimalityLib.InvariantDomain
import ArrowMinimalityLib.StructuralInference
import ArrowMinimalityLib.IteratedLoop

open ProofGraphs
open RefinementProofGraphs
open InvariantDomain
open StructuralInference

namespace LoopInvariantDiscovery

variable {Atom : Type} {Sig : Ty Atom → Ty Atom → Type}

-- ============================================================================
-- 1. The body guarantee invariant: projecting the postcondition onto C
-- ============================================================================

/-- The loop invariant seeded from the body's structural postcondition.

Given a loop body `G : (A×C) ⇝ (B×C)`, the body has an inferred
postcondition `P = inferred_postcondition G` — a predicate on the output
value `(v, c')`. We project `P` onto the feedback channel `C` by
existentially quantifying over the output value `v`:

  `I(c') = ∃ v, P(Val.pair v c')`

This says: "there exists an output value `v` such that the pair `(v, c')`
satisfies the body's postcondition." Since `inferred_postcondition_sound_full`
guarantees that `P` holds of the *actual* output of every well-typed
execution, `I(c')` holds for every `c'` produced by the body.

This is the **body guarantee seeding** — the first step of the fixpoint
iteration in the Rust implementation. -/
def loop_invariant_from_body
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    : Val → Prop := fun c' =>
  ∃ v, inferred_postcondition den sig_guarantee G (Val.pair v c')

-- ============================================================================
-- 2. Soundness: the seeded invariant is preserved by every step
-- ============================================================================

/-- **Soundness of the body-guarantee-seeded loop invariant.**

For every well-typed execution of the loop body `G · (a, c) ⟹ (v, c')`,
the seeded invariant `I(c')` holds — **unconditionally** (no hypothesis
on `c` is needed).

Proof: By `inferred_postcondition_sound_full`, the body's postcondition
`P` holds of the actual output `(v, c')`. Taking `v` as the existential
witness, `I(c') = ∃ v, P(pair v c')` is satisfied.

This is stronger than what loop invariant preservation requires
(`I(c) → I(c')`): the invariant holds of the output regardless of the
input. This is the "optimistic propagation" principle — the body's
guarantee establishes the invariant on every output. -/
theorem loop_invariant_from_body_sound
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    : ∀ (a v c c' : Val),
      HasType (Val.pair a c) (A ×ₜ C) →
      Eval den G (Val.pair a c) (Val.pair v c') →
      loop_invariant_from_body den sig_guarantee G c' := by
  intro a v c c' ha e
  -- The body's postcondition holds of the actual output (v, c')
  have hp := inferred_postcondition_sound_full
    den sig_guarantee sig_sound sig_type_sound
    G dG (Val.pair a c) (Val.pair v c') ha e
  -- Take v as the witness: I(c') = ∃ w, P(pair w c') is satisfied by w = v
  exact ⟨v, hp⟩

/-- **Preservation form of the soundness theorem.**

The seeded invariant satisfies the standard loop invariant preservation
obligation: if `I(c)` holds before the step, then `I(c')` holds after.
This follows trivially from `loop_invariant_from_body_sound`, which
establishes `I(c')` unconditionally. -/
theorem loop_invariant_from_body_preserved
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    : ∀ (a v c c' : Val),
      Eval den G (Val.pair a c) (Val.pair v c') →
      HasType (Val.pair a c) (A ×ₜ C) →
      loop_invariant_from_body den sig_guarantee G c →
      loop_invariant_from_body den sig_guarantee G c' := by
  intro a v c c' e ha _
  exact loop_invariant_from_body_sound den sig_guarantee sig_sound sig_type_sound
    G dG a v c c' ha e

-- ============================================================================
-- 3. Post-fixpoint: the transfer weakens to True
-- ============================================================================

/-- The **weakest precondition transfer** for the loop body: `wp(G, I)(c)`
is the weakest predicate on the input feedback `c` that ensures `I(c')`
holds for every output `c'`.

  `wp(G, I)(c) = ∀ a v c', Eval G (pair a c) (pair v c') → I(c')`

Since `I(c')` holds unconditionally (by `loop_invariant_from_body_sound`),
`wp(G, I)(c) = True` for any `c`. The fixpoint is reached immediately. -/
def wp_transfer
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    (I : Val → Prop)
    : Val → Prop := fun c =>
  ∀ (a v c' : Val),
    HasType (Val.pair a c) (A ×ₜ C) →
    Eval den G (Val.pair a c) (Val.pair v c') →
    I c'

/-- **The body-guarantee invariant is a post-fixpoint.**

The fixpoint iteration is `I_{n+1} = I_n ∧ wp(G, I_n)`. For the
body-guarantee invariant `I_0 = loop_invariant_from_body G`:

  `I_0 ⟹ wp(G, I_0)` (I_0 is a post-fixpoint: the transfer weakens I_0).

This is because `wp(G, I_0)(c)` requires `I_0(c')` for all outputs `c'`,
and `I_0(c')` holds unconditionally. So `wp(G, I_0) = True`, and
`I_0 ⟹ True` is trivially true.

The fixpoint `I_1 = I_0 ∧ True = I_0` is reached in one step. -/
theorem loop_invariant_post_fixpoint
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    : ∀ c,
      loop_invariant_from_body den sig_guarantee G c →
      wp_transfer den sig_guarantee sig_sound sig_type_sound G dG
        (loop_invariant_from_body den sig_guarantee G) c := by
  intro c _ a v c' ha e
  -- I_0(c') holds unconditionally by soundness
  exact loop_invariant_from_body_sound den sig_guarantee sig_sound sig_type_sound
    G dG a v c c' ha e

/-- **The transfer is trivially satisfied for the body-guarantee invariant.**

For `I = loop_invariant_from_body G`, the transfer `wp(G, I)(c)` is
trivially true: for any execution `G · (a, c) ⟹ (v, c')`, `I(c')` holds
by unconditional soundness. No hypothesis on `c` is needed.

This means the fixpoint iteration `I_{n+1} = I_n ∧ wp(G, I_n)` converges
in one step: `I_1 = I_0 ∧ True = I_0`. -/
theorem wp_transfer_holds_for_body_guarantee
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    (c : Val)
    : wp_transfer den sig_guarantee sig_sound sig_type_sound G dG
        (loop_invariant_from_body den sig_guarantee G) c := by
  intro a v c' ha e
  exact loop_invariant_from_body_sound den sig_guarantee sig_sound sig_type_sound
    G dG a v c c' ha e

-- ============================================================================
-- 4. Strictly stronger than the floor
-- ============================================================================

/-- **The body-guarantee invariant is strictly stronger than the floor
(for non-trivial bodies).**

If the body's postcondition `P` is non-trivial (there exists a `c'` such
that `P(pair v c')` fails for ALL `v`), then the body-guarantee invariant
`I(c')` fails at that `c'` (since `I(c') = ∃ v, P(pair v c')` and no `v`
satisfies `P`).

This is the formal counterpart to `floor_does_not_imply_inferred` in
`StructuralInference.lean`: the structural inference for loops is
strictly stronger than the sound floor. -/
theorem body_guarantee_stronger_than_floor
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (hnontrivial : ∃ c', ∀ v, ¬ inferred_postcondition den sig_guarantee G (Val.pair v c'))
    : ¬ (∀ c', True → loop_invariant_from_body den sig_guarantee G c') := by
  obtain ⟨c', hv⟩ := hnontrivial
  intro h
  obtain ⟨v, hv'⟩ := h c' trivial
  exact hv v hv'

-- ============================================================================
-- 5. Initialization: the invariant holds after the first step
-- ============================================================================

/-- **Initialization via first step.**

The body-guarantee invariant `I` is established by the first execution of
the loop body: if `G · (a, c₀) ⟹ (v, c₁)`, then `I(c₁)` holds (by
soundness). For subsequent steps, `I` is preserved (also by soundness).

The initial feedback value `c₀` need not satisfy `I` — the invariant is
established on the output, not the input.

This is the **optimistic propagation** principle: we don't require the
invariant to hold initially; we require the body to *establish* it on
every output. -/
theorem loop_invariant_initialized_by_first_step
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    (a c₀ v c₁ : Val)
    (ha : HasType (Val.pair a c₀) (A ×ₜ C))
    (e : Eval den G (Val.pair a c₀) (Val.pair v c₁))
    : loop_invariant_from_body den sig_guarantee G c₁ := by
  exact loop_invariant_from_body_sound den sig_guarantee sig_sound sig_type_sound
    G dG a v c₀ c₁ ha e

-- ============================================================================
-- 6. Concrete example: arr(φ) as the loop body
-- ============================================================================

/-- For a loop body `G = arr(φ)` where `φ : (A×C) ⇝ (B×C)`, the
body-guarantee invariant is `I(c') = ∃ v, sig_guarantee φ (Val.pair v c')`.

This is the simplest non-trivial case: the body is a single base arrow,
and its guarantee directly becomes the loop invariant (projected onto C).

**Soundness:** By `sig_sound`, `sig_guarantee φ (den φ (pair a c))` holds
for well-typed input. Since `den φ (pair a c) = pair v c'` (the output is
a pair), `sig_guarantee φ (pair v c')` holds, so `I(c') = ∃ v, sig_guarantee
φ (pair v c')` is satisfied with witness `v`. -/
theorem arr_body_guarantee_sound
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    {A B C : Ty Atom} (φ : Sig (A ×ₜ C) (B ×ₜ C))
    (a c v c' : Val) (ha : HasType (Val.pair a c) (A ×ₜ C))
    (heq : den φ (Val.pair a c) = Val.pair v c')
    : loop_invariant_from_body den sig_guarantee (Graph.arr φ) c' := by
  -- inferred_postcondition (arr φ) = sig_guarantee φ
  -- I(c') = ∃ w, sig_guarantee φ (pair w c')
  -- sig_sound gives: sig_guarantee φ (den φ (pair a c))
  -- By heq: den φ (pair a c) = pair v c'
  have hp := sig_sound φ (Val.pair a c) ha
  rw [heq] at hp
  -- Take v as the witness
  exact ⟨v, hp⟩

-- ============================================================================
-- 7. Iterated loop: invariant holds at the fixpoint step
-- ============================================================================

/-- **The body-guarantee invariant holds at the fixpoint step of an
iterated loop.**

Using the iterated loop semantics (`EvalIter`), the `eLoopZero` case
(the fixpoint step) produces `(v, cₙ)` where `cₙ` is the stable feedback
value. By converting the `EvalIter` evidence to `Eval` and applying
`inferred_postcondition_sound_full`, the invariant `I(cₙ)` holds.

This is the key step for proving the invariant holds at the loop's
fixpoint — the final feedback value satisfies the body's guarantee. -/
theorem loop_invariant_at_fixpoint_step
    (den : ∀ {A B : Ty Atom}, Sig A B → Val → Val)
    (sig_guarantee : SigGuarantee Atom Sig)
    (sig_sound : ∀ {A B : Ty Atom} (φ : Sig A B) (a : Val),
      HasType a A → sig_guarantee A B φ (den φ a))
    (sig_type_sound : @ProofGraphs.SigSound Atom Sig den)
    {A B C : Ty Atom} (G : Graph Atom Sig (A ×ₜ C) (B ×ₜ C))
    (dG : TyOf G)
    (a v cₙ : Val) (ha : HasType a A) (hcₙ : HasType cₙ C)
    (eStep : IteratedLoop.EvalIter den G (Val.pair a cₙ) (Val.pair v cₙ))
    : loop_invariant_from_body den sig_guarantee G cₙ := by
  -- Convert EvalIter to Eval for the body step
  have eEval := IteratedLoop.iter_to_eval den eStep
  -- Build the well-typedness of the input pair
  have hAC : HasType (Val.pair a cₙ) (A ×ₜ C) := HasType.htPair ha hcₙ
  -- Apply inferred_postcondition_sound_full
  have hp := inferred_postcondition_sound_full
    den sig_guarantee sig_sound sig_type_sound
    G dG (Val.pair a cₙ) (Val.pair v cₙ) hAC eEval
  exact ⟨v, hp⟩

-- ============================================================================
-- 8. Summary: what is proven and what remains
-- ============================================================================

/-
## Summary of theorems (all zero `sorry`)

### Core results (zero `sorry`):

1. **`loop_invariant_from_body_sound`** — the body-guarantee invariant
   holds of every output `c'`, unconditionally. This is the key soundness
   theorem: the body's structural postcondition, projected onto the
   feedback channel, is a valid loop invariant.

2. **`loop_invariant_from_body_preserved`** — the standard preservation
   form: `I(c) → I(c')` for every step. Follows trivially from (1)
   because `I(c')` holds unconditionally.

3. **`loop_invariant_post_fixpoint`** — the body-guarantee invariant is
   a post-fixpoint: `I ⟹ wp(G, I)`. The transfer weakens because the
   invariant holds unconditionally.

4. **`wp_transfer_holds_for_body_guarantee`** — the transfer `wp(G, I)`
   is trivially satisfied (no hypothesis on `c` needed). The fixpoint
   iteration converges in one step: `I_1 = I_0 ∧ True = I_0`.

5. **`body_guarantee_stronger_than_floor`** — for non-trivial bodies
   (where the postcondition constrains C), the invariant is strictly
   stronger than `True`.

6. **`loop_invariant_initialized_by_first_step`** — the invariant is
   established by the first execution of the body (optimistic propagation).

7. **`arr_body_guarantee_sound`** — concrete case: `arr(φ)` as the loop
   body, the guarantee becomes the invariant.

8. **`loop_invariant_at_fixpoint_step`** — the invariant holds at the
   fixpoint step of an iterated loop (using `EvalIter` semantics).

### What this formalization establishes:

The **body guarantee seeding** — the core mechanism of the Rust
implementation's loop invariant discovery — is **sound**: the body's
structural postcondition, projected onto the feedback channel, is a
valid loop invariant that holds unconditionally on every output. The
fixpoint converges in one step because the transfer weakens to `True`.
The invariant is strictly stronger than the floor for non-trivial bodies.

This bridges the gap between the compositional postcondition propagation
(formalized in `StructuralInference.lean`) and the loop invariant
discovery (implemented in Rust but previously unformalized). The
`inferred_postcondition` for `loop` remains `True` (the floor), but the
body guarantee invariant provides the strengthened result that the Rust
implementation computes.

### What remains for future work:

- **WP branching through Choice** — the Rust implementation's
  `weakest_precondition` branches through `Choice`/`left` combinators,
  conjoining both arms' guarantees. Formalizing this requires extending
  the transfer function to handle `left`.
- **Structural pattern recognition** — side-channel loops, accumulator
  loops, filter loops. These are heuristics for identifying invariant
  schemas from graph wiring; formalizing them requires defining the
  pattern classes and proving each preserves its schema.
- **Domain membership of the projected invariant** — the existential
  projection `∃ v, P(pair v c')` is in `D` when `P` is in `D` and the
  projection is structural (the Rust implementation verifies this
  case-by-case). A general proof requires showing `D` is closed under
  the specific projections used.
- **Full iterated loop soundness** — proving the invariant holds at
  every intermediate step (not just the fixpoint), which requires
  induction over `EvalIter` with correct handling of `eComp` and `eArr`.
-/

end LoopInvariantDiscovery
