# Prior Art & Novelty Assessment

## Status: Positioning document

Date: 2026-08-31

This document records the result of a prior-art search on the core
ideas in this project, and identifies what is *not* novel versus what
*might be* novel. It is intended to keep the project's claims honest
and to scope the open research question worth investigating further.

---

## The broad idea is NOT novel

The claim

> *Explicit representation makes monadic computation inspectable.*

is **established territory**. There is substantial prior art involving:

- free monads,
- freer monads,
- explicit monadic ASTs,
- explicit binds,
- algebraic effects,
- AST-based effect systems.

These approaches already demonstrate that computations hidden behind
higher-order continuations can instead be represented explicitly and
inspected or transformed. A **2026 article** discusses explicit Free
Monad representations and using explicit structure to preserve
properties that would otherwise be hidden by opaque continuations.

**Implication:** SAE / this project must **not** claim novelty for
"explicit representation makes monadic computation inspectable." That
is known.

---

## What might be novel (the narrow claim)

The potentially interesting claim is much narrower:

> **SAE combines full dynamic/monadic composition with Arrow-like
> structural inspection, specifically preserving choice recovery,
> branch-local effect information, and dependency relationships through
> an explicit `Apply` representation.**

The prior-art search did **not** reveal an obvious prior system
demonstrating that exact combination. This is **not proof of novelty**,
but it identifies the right research question.

The potentially novel contribution can be represented as the
conjunction:

```text
Dynamic composition
        +
Explicit Apply representation
        +
Choice recovery
        +
Per-branch effect inference
        +
Dependency-graph tracing
        +
Arrow-like inspectability
        +
Formal kernel properties
```

No single one of these is new. The **combination**, demonstrated
empirically and backed by the Lean4 minimality kernel, is what
appears to be uncharted.

---

## Recommended positioning

### Instead of

> "SAE proves that explicit representation breaks monadic opacity."

### use

> **SAE demonstrates that full dynamic composition need not sacrifice
> structural inspectability when monadic computation is represented
> explicitly. Its explicit `Apply` representation preserves choice
> structure, branch-local effect information, and dependency
> relationships that are normally hidden behind higher-order
> continuations.**

### Instead of

> "The theory is complete."

### use

> **Challenge #11 substantially strengthens the case that SAE occupies
> a point in the design space combining monadic expressiveness with
> structural inspectability.**

### Distinguish three kinds of evidence

- **Formal proofs:** the Lean4 minimality theorems (Challenge #10).
- **Empirical evidence:** the 177 Rust tests, including Challenge #11.
- **Architectural conclusion:** explicit dynamic computation can
  remain inspectable.

Avoid treating the 19 tests of Challenge #11 as a *mathematical
proof*. They are empirical evidence for an architectural conclusion.

---

## The key distinction to keep sharp

> **The problem is not fundamentally "Arrow vs. Monad"; it is opaque
> executable representation vs. explicit inspectable representation.**

A freer/free representation can make dynamic computations explicit as
data, allowing structural analysis without giving up dynamic
composition. The arrow calculus is one route to such a representation;
freer monads are another. The structural advantage belongs to
*explicitness*, not to arrows per se (see `RESULTS_CHALLENGE9.md` and
`THESIS.md`).

---

## Bottom line

Challenge #11 matters, but its novelty is **not** "free/freer monads
are inspectable" — that is already known.

The potentially significant result is whether **SAE's particular
explicit `Apply` semantics simultaneously recover the structural
information we care about while retaining unrestricted dynamic
composition**, and whether that combination has been demonstrated or
formally characterized before.

That is the claim worth investigating with a focused, dedicated
prior-art search — beyond the broad scan that produced this document.

---

## Open follow-up

A focused prior-art search should target specifically:

- systems combining **dynamic/monadic composition** with **Arrow-like
  structural inspection**,
- explicit `Apply`-style nodes that preserve **choice recovery**,
  **per-branch effect inference**, and **dependency-graph tracing**,
- formal characterizations of that combination (not just informal
  implementations).

Until that search is done, the novelty claim remains **plausible but
unverified**.
