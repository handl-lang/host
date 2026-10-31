+++
id = "ADR-0101"
title = "ADR-0100: The Directed-HoTT Limit of the Term Calculus"
status = "open"
priority = "medium"
+++

---
title: The Directed-HoTT Limit of the Term Calculus
type: adr
date: 2026-09-08
status: proposed
status_note: Proposed — the calculus is a directed (∞,1)-category; 1-cells stay the engine.
---

# ADR-0100: The Directed-HoTT Limit of the Term Calculus

## Decision

1. **`Term` is a 2-category**: objects are sorts, 1-cells are arrows, 2-cells
   are rewrites between arrows.
2. **`Language` is a presented 2-category** — its data (sorts, arrows, rewrites)
   presents the theory, not a flat grammar.
3. **A `Model` is a 2-functor** from the presented theory to a semantic
   category; the engine interprets the functor.
4. **The model family** (value, type, exec, meta, transform) are 2-functors
   over the same `Term`; none extends the calculus.
5. **Syntax for rewrites and paths** is data (`law`, `rewrite`, `Path`), not
   Rust arms.
6. **Univalence and transport** are 1-cell operators derived from the
   equality type, not new primitives.
7. **Grades are propositions** — a grade is a proposition about a cell, and
   grading is a 2-categorical construction.
8. **Directed paths**: `Term` is a directed (∞,1)-category; the 2-cells are
   directed rewrites, not invertible in general.
9. **Implementation**: add `Term::Rewrite`, `law`/`rewrite` declarations,
   `Model::rewrite`, make `plugin-engine-transform` the `OptimizerModel`, add
   `transport`/`equiv`, and the `Path` sort.
10. **Keep the 1-categorical engine**: evaluation never requires 2-cells; a
    model may return `Unsupported` for `Rewrite` nodes.

## Why

The calculus had rewrites as an afterthought (a separate transform engine),
so laws, equality and optimization were outside the semantics. The 2-category
reading puts rewrites in `Term` and makes every model a 2-functor, which is the
only way the optimizer and the evaluator share one theory.

## Not doing
- **Keep rewrites outside the calculus** — the split this ADR removes.
- **Require 2-cells to evaluate** — evaluation stays 1-categorical.
- **Invertible 2-cells only** — paths are directed.

## Consequences
- `Term` gains `Rewrite`; models that do not optimize return `Unsupported`.
- `law`/`rewrite`/`Path` become language data.
- The optimizer becomes a `Model`, not a side engine.

## Enforcement
- `engine/` — `Term`, `Model`, `Rewrite`.
- `crates/handl-kit-data-canonical/data/` — `law`/`rewrite`/`Path` rows.
- `engine-transform*` — the `OptimizerModel`.

## Design
+The 2-category presentation, model family, univalence/transport, grades and the
+directed limit:
+[`design/0100-directed-hott-limit.md`](../design/0100-directed-hott-limit.md).
