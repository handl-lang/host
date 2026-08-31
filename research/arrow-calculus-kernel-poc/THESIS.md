# Thesis & Research Questions

## Refined Thesis

> **A small arrow calculus based on composition, choice, and feedback is
> sufficient to construct a substantial programming-language architecture
> without taking lambda abstraction or monadic structure as primitive.**

This is narrower than "arrows are a complete foundation for programming
languages." The experiments establish *sufficiency for construction*, not
*minimality* or *necessity*. The distinction matters: sufficiency says
"you can build it"; minimality says "you need exactly these primitives";
necessity says "no smaller basis works."

---

## Evidence Structure

The 121 tests establish a construction arc, not a foundational proof:

```
                    Arrow calculus
                         │
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
      composition       choice       feedback
          │              │              │
          └──────┬───────┴───────┬──────┘
                 ▼               ▼
             abstraction     recursion
                 │
                 ▼
             computation
                 │
          ┌──────┴──────┐
          ▼             ▼
       effects       semantics
          │             │
          ▼             ▼
      handlers      Language
          │             │
          ▼             ▼
   effect typing   generic tooling
```

Each layer *emerges from the representation below*, not from a new primitive.
This is the strongest aspect of the result: the features are not bolted on,
they are *observed in* the arrow substrate.

---

## The Central Formal Claim

The deepest result is the **runtime/static correspondence** for effects:

```
Runtime                         Static

UnhandledOp(op)        ↔       op ∈ ε

handler consumes op    ↔       op removed from ε

pass-through op        ↔       op remains in ε
```

An effect row is not an additional semantic layer. It is a **static
observation of the existing computation representation** — the set of
operations suspended in the freer-arrow tree. Handlers are effect
eliminators (`ε \ ops(H)`), and this typing rule falls directly out of the
pass-through semantics.

This correspondence is the project's central formal claim and should be
elevated accordingly.

---

## The IComp Question

The one architectural blemish is the duplication between `Comp` (runtime,
Church-encoded continuations via `Rc<dyn Fn>`) and `IComp` (static,
defunctionalized continuations via `Box<IComp>`).

**Precise framing:** These are the same object under different encodings of
the continuation. `Op(op, arg, k)` where `k : Value -> Comp` *is* the Church
encoding of `Op(op, arg, K)` where `K` is a defunctionalized continuation
datatype. The closure is what you get when you re-functionalize.

The issue is not that effect inference is impossible over `Comp`; it is that
**opaque host-language closures destroy structural visibility**. The fix is
not a third representation — it is picking the defunctionalized form as
canonical and interpreting it. This is the standard freer-monad move.

- **Engineering choice:** keep both (Comp for execution, IComp for analysis)
- **Formal choice:** canonical freer AST, interpreted into executable arrows

The formal choice is what matters for minimality: you cannot prove
minimality over a representation that hides structure.

---

## The Structural Question (Challenge #9 — RESOLVED)

The 121 tests established that arrows *work*. Challenge #9 tested whether
their **structural advantages survive monadic translation**.

### Answer: The opaque monad is semantically adequate but structurally lossy

Three representations were compared (154/154 tests, 33 new):

| Property | Arrow | Opaque monad | Freer monad |
|---|---|---|---|
| Sequential composition | explicit | explicit | explicit |
| Fanout / independence | explicit | **LOST** | explicit |
| Choice / branching | explicit | **LOST** | explicit |
| Effect set | traversal | **LOST** | traversal |
| Per-branch effects | per branch | **LOST** (union only) | per branch |
| Dependency graph | buildable | **LOST** | buildable |
| Feedback | explicit | **LOST** | detectable |
| Semantic results | correct | correct | correct |

The opaque monad produces the same results but loses all structural
information. The freer monad (a monad with explicit structure) recovers
everything, proving the loss is about **opacity** (closures hide
structure), not about monadic structure per se.

### The precise claim

The structural advantage belongs to **explicit, traversable
representations** (freer arrows, freer monads) over **opaque
representations** (conventional monads with function-based continuations).
The arrow calculus naturally produces explicit representations; the
monadic encoding naturally produces opaque ones.

This explains why the freer-arrow model has been useful throughout this
project: effect inference, generic tooling, and structural optimization
all exploit the explicit structure that the arrow representation preserves.

### The false-dependency phenomenon

When arrow fanout (`f &&& g` — independent) is translated to monadic bind,
the second computation appears to depend on the first's result — a false
dependency created by the bind chain. The arrow's dependency graph
correctly shows no edge; the monad cannot build the graph at all.

---

## Open Questions (Beyond Challenge #9)

### Minimality — RESOLVED ✅

The minimal arrow calculus for a total PL requires **five independent
primitives**: `arr`, `comp`, `first`, `loop`, `left`. No primitive can
be derived from the others.

- **`left` (choice)** — NOT derivable from `arr + comp + first + loop`.
  No constructor introduces `Sum` on the output. (Challenge #10, Lean4)
- **`first` (products)** — NOT derivable from `arr + comp + loop`.
  No constructor introduces `×` on the output. (Challenge #10, Lean4)
- **`loop` (feedback)** — NOT derivable from `arr + comp + first`
  (total setting). No constructor introduces feedback. (Challenge #10, Lean4)
- **`comp` (composition)** — NOT derivable from the rest. Without `comp`,
  no combinator chains two arrows sequentially.
- **`arr` (lifting)** — NOT derivable from the rest. Without `arr`, no
  combinator lifts a pure function into the arrow.

All five proven via the free-arrow (initiality) argument in Lean4. See
`lean/ArrowMinimalityLib/Basic.lean`.

**Caveat:** `loop` is independent only in a *total* setting. In a
language with general recursion (`fix`), `loop` is derivable — but then
general recursion is the primitive, and `loop` is a pattern of use.

### Necessity of `lit`

- Is `lit` genuinely necessary for generic tooling, or derivable?
- It may be "necessary for optimization" (reifying values back into syntax)
  but not "necessary for tooling" (pretty-print, error report, trace work
  without it).

### Translation asymmetry

- Can every lambda term encode as an arrow term of comparable size?
- Are there arrow programs (using `loop` or `choice` essentially) whose
  lambda encodings are awkward or non-compositional?
- If arrows encode lambda cleanly but lambda encodes arrows awkwardly,
  that is the real argument for arrows as the foundation.

---

## Status

| Phase | Challenges | Tests | Status |
|-------|-----------|-------|--------|
| 1 | #1–5 (expressiveness, abstraction, parametricity, typing, recursion) | 44 | ✅ Complete |
| 2 | #6 (effects & handlers) | 20 | ✅ Complete |
| 3 | #7–8 (language protocol, generic tooling) | 26 | ✅ Complete |
| Deep dive | Effect typing (row inference, handler elimination, polymorphic rows) | 31 | ✅ Complete |
| 4 | #9 (structural preservation under monadic translation) | 33 | ✅ Complete |
| 5 | #10 (minimality — Lean4) | 7 theorems | ✅ Complete |
| 6 | #11 (ArrowApply without opacity) | 19 | ✅ Complete |

**Total: 173/173 Rust tests + 7 Lean4 theorems, zero warnings.**

---

## The Corrected Slogan (after Challenges #9 and #11)

The original thesis: "arrows can replace monads."

The corrected thesis:

> **ArrowApply (in the explicit/freer style) gives you monadic power
> WITHOUT the opacity penalty. Ordinary opaque monads bury structure
> in closures; the explicit ArrowApply presentation keeps the
> computation tree inspectable — effects, branching, per-branch
> analysis, and dependency graphs all remain recoverable.**

The isomorphism `ArrowApply ≅ Monad` is about **expressive power**, not
about **representation**. The classic translation produces an opaque
monadic computation. Our result (Challenge #11) shows there exists a
presentation of the same power that stays **transparent**. That is the
non-trivial part, and it is what makes the claim interesting.

It no longer says "arrows are better than monads"; it says "this explicit
ArrowApply presentation gives you monadic power *without* the opacity
penalty."
