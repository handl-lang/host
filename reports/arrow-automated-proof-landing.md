# Landing Report: Automated Proof for Arrow Programs — Dω Generalization + Decidable Loop-Free Fragment

**Date:** 2026-09-07
**Scope:** `research/arrow-calculus-kernel-poc/lean/ArrowMinimalityLib/`
**Status:** Landed — `lake build` clean (24 jobs), zero `sorry`, axiom-checked.

---

## Summary

Two formalization milestones landed in the Lean4 corpus, both addressing the
question *"can we universalize automated proof of arrow-semantics programs?"*:

1. **`AlgorithmicCompletenessOmega.lean`** — the invariant domain for
   annotation-free loop-invariant inference was widened from `D` to `Dω`,
   converting Theorem 4's *conditional* algorithmic completeness into an
   **unconditional** theorem.
2. **`DecidableFragment.lean`** — the loop-free arrow fragment received a
   mechanized decision procedure that is **sound, terminating, and complete**
   (relative to its property language), realizing the
   "normalize → exact semantics → TRUE/FALSE" pipeline as a theorem.

Both files are imported from the library root (`ArrowMinimalityLib.lean`).

---

## 1. `AlgorithmicCompletenessOmega.lean` — unconditional completeness in Dω

### The obstruction that was removed

`AlgorithmicCompleteness.lean` (Theorem 4) proved only *conditionally* that
`analyze_strongest ∈ D`: it required D to be closed under arbitrary meets,
and documented why that closure cannot be added directly — an infinitary-meet
constructor forces the index type to embed `InvariantDomain` derivations
(`Type 1`), so the index always lives one universe above the inductive
("universe spiral").

### The escape: impredicative `Prop`

The widened domain `InvariantDomainOmega` (`Dω`) is a **Prop-valued**
inductive family with the same six constructors as `D` **plus** an
infinitary-meet constructor

```
dInf : {ι : Type} → (f : ι → Val → Prop) → ((i : ι) → Dω (f i)) →
       Dω (fun v => ∀ i, f i v)
```

Because derivations become proof-irrelevant `Prop`s, the meet's index type
(the subtype of Dω-members *preserved by G*) lives in `Type 0` — the spiral
does not arise.

### Theorems landed (all zero `sorry`)

| Theorem | Content |
| --- | --- |
| `domega_closed_under_meets` | Dω closed under arbitrary meets — Theorem 4's hypothesis, now a theorem |
| `invariant_domain_mono` | `D ⊆ Dω` — full backward compatibility |
| `PreservesWt` / `preserves_meet` | well-typed preservation; closed under arbitrary meets |
| `analyze_strongest_omega_sound` | the strongest preserved Dω-invariant is preserved |
| **`analyze_strongest_omega_in_domain`** | **headline: it is itself in Dω — Theorem 4 unconditional** |
| `analyze_strongest_omega_implies_any_preserved` | it implies every preserved Dω-invariant (so it *is* the strongest) |
| `analyze_strongest_omega_implies_analyze_strongest` | strictly dominates the original `analyze_strongest` |
| `domega_true_mem` | the sound floor lives in Dω |

### Axiom audit

`#print axioms` on all seven: only `propext` (standard). No `sorryAx`.

### Honest scope

Incompleteness is *narrowed*, not eliminated: predicates outside `Dω`
(e.g. iteration-count invariants not expressible via signature guarantees +
meets) remain structurally undetectable. Membership in `Dω` alone does not
imply preservation; soundness is stated relative to constituents'
preservation, matching the established framing of `InvariantDomain.lean`.

---

## 2. `DecidableFragment.lean` — sound/terminating/complete decider for the loop-free fragment

### The fragment

Loop-free arrow programs over a finite domain `Fin n`:

```
Graph n := idG | baseG (f : Fin n → Fin n) | compG a b
```

with exact semantics `Graph.sem : Graph n → (Fin n → Fin n)` by structural
recursion, and a decidable property language

```
PropF n := pTrue | pEq (f : Fin n → Fin n) | pAnd p q
```

### The pipeline, mechanized

"normalize/analyze → exact semantics → prove property → TRUE/FALSE" is the
pair (`Graph.sem`, `PropF.decide`), backed by a finite-enumeration equality
decider (`funEqAux`) whose correctness lemma `funEqAux_eq`
(`funEqAux f g = true ↔ ∀ i, f i = g i`) carries soundness and completeness.

### The three guarantees (all zero `sorry`, axiom-checked)

| Guarantee | Theorem | Axioms |
| --- | --- | --- |
| **Sound** (TRUE → holds) | `PropF.decider_sound` | propext, Quot.sound |
| **Complete** (holds → TRUE) | `PropF.decider_complete` | propext |
| **Terminating** (always TRUE xor FALSE) | `PropF.decider_terminates` | none |

Additionally: `Graph.deq_true_iff` — semantic equality of two arrow programs
is decidable; `funEq f g` yields a `Decidable (f = g)` instance via
`decidable_of_decidable_of_iff`.

### Honest scope (the boundaries, stated in-file)

- **The `arr` restriction is mandatory, not optional**: unrestricted `arr`
  contents = arbitrary computable functions ⇒ Rice's theorem kills any
  TRUE/FALSE prover even loop-free. Completeness is relative to `PropF` —
  the precise sense of "complete for a specified class of
  programs/properties".
- **`ArrowLoop` is the genuine decidability boundary on infinite domains**
  (fixpoint semantics); over finite domains it becomes model-checkable, but
  the one-pass structural recursion used here no longer suffices.
- **`first`/`second`/`fanout`/`ArrowChoice` are conservative extensions**:
  products/sums of finite domains stay finite and componentwise/branchwise
  semantics preserve decidability. Their omission is brevity of the
  mechanization, not a decidability boundary (monomorphic `Fin n` encodes
  products by standard finite pairings).

---

## Build & verification

```
cd research/arrow-calculus-kernel-poc/lean
lake build          # Build completed successfully (24 jobs)
```

Axiom checks (all clean, no `sorryAx`):

```
#print axioms analyze_strongest_omega_in_domain                 -- [propext]
#print axioms analyze_strongest_omega_sound                     -- [propext]
#print axioms analyze_strongest_omega_implies_any_preserved     -- [propext]
#print axioms analyze_strongest_omega_implies_analyze_strongest -- [propext]
#print axioms domega_closed_under_meets                         -- [propext]
#print axioms invariant_domain_mono                             -- [propext]
#print axioms preserves_meet                                    -- (none)
#print axioms PropF.decider_sound                               -- [propext, Quot.sound]
#print axioms PropF.decider_complete                            -- [propext]
#print axioms PropF.decider_terminates                          -- (none)
#print axioms Graph.deq_true_iff                                -- [propext, Quot.sound]
#print axioms funEqAux_eq                                       -- [propext]
```

A stale comment was also corrected en route: `LoopInvariantDiscovery.lean`
line ~817 claims "the hard inversions are marked `sorry`" — the corpus in
fact builds with **zero real `sorry`** (verified by build + axiom check);
the WP-branching-through-`Choice` section is fully proven. No source change
was made to that file.

## Files changed

- **Added** `lean/ArrowMinimalityLib/AlgorithmicCompletenessOmega.lean`
- **Added** `lean/ArrowMinimalityLib/DecidableFragment.lean`
- **Modified** `lean/ArrowMinimalityLib.lean` (two new imports)

## Combined significance

Together the two files give the "specified class" claim both halves:

- **Loop case** (annotation-free inference): algorithmic completeness now
  unconditional within a strictly larger, unconditionally closed domain `Dω`.
- **Loop-free case**: an absolute sound/terminating/complete decision
  procedure for a decidable property language.

The remaining boundary is exactly characterized, not hidden: outside `Dω`
the inference is incomplete by theorem; beyond the decidable fragment,
undecidability (Rice, fixpoints) applies — the automation claim is
"*for a proven-sound, well-characterized and now strictly widened class,
automatically*".
