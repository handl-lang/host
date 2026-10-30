---
title: Paper–Implementation Alignment Plan
type: adr
date: 2026-09-18
status: accepted
status_note: |-
  Accepted — direction decided in review; execution is phased below.

  Refines [ADR-0119 N-Dimensional Arrow Calculus](./0119-n-dimensional-arrow-calculus.md)
  and `research/PAPER.md`. Supersedes the lambda-flavoured "arrows as closures"
  proposal considered and rejected in the same review.
---


# ADR-0120: Paper–Implementation Alignment Plan

## Decision


Execute the alignment in five phases; ordering is load-bearing:
`1 → 2 → 3 → (4, 5 in parallel)`.

### Phase 1 — Arrows as values

1. A `function` used as a _value_ lowers to `Arr { value: <Term> }` (quote).
   No environment is captured — it was already resolved at lower time.
2. `call(f, x)` lowers to `seq(pair(quote f, x), app)` for env-bound `f`;
   `seq(x, f)` remains the encoding of flow `x >>> f`.
3. `desugar.data`'s recursor is re-encoded as `loop(step)` feedback instead
   of `inner(inner)` self-application (the lambda-calculus encoding).
4. No currying. Parameters are products; if partial application is ever
   proven necessary it is expressed as prefix-splice
   `seq(fanout(const a, id), f)` — pure products, no closures.

### Phase 2 — Composition axes

5. `Term::Seq` gains an `axis` slot (default `0`); `f >>> g` is `>>>[0]`.
6. `>>>[k]` surface syntax is a YAML/grammar change only.
7. Types track globular source/target at every level `≤ n`; the typing rule
   for `a >>>[k] b` is `tgt_k(a) = src_k(b)`.

### Phase 3 — `dim` unification

8. Merge `ArrSource::Foreign(grade)` into morphism dimension; the `~[n]~>`
   bracket means dimension. Open sub-question: grades are names and dims are
   numbers, so handler lookup becomes dimension-typed.
9. Effects are re-read as cells: `op` is a cell, `handle` a cell interpreter
   (`(N+1)`-cell mapping an effectful `N`-cell to a pure one, per ADR-0119 §6).

### Phase 4 — Dependent structure

10. `Family<A>`, `Π`, `Σ` enter via `types.yaml` + `desugar` — extending the
    existing `data`/indexed-family/recursor machinery.
11. Reindexing `f*` is composition (`· >>> f`); `Σ_f ⊣ f* ⊣ Π_f` is expressed
    as laws (`~[2]~>` cells), not engine primitives.

### Phase 5 — Laws as `M[2]` terms

12. `f ~[2]~> g` parses to a `Term` and feeds `law_inference` — no separate
    laws fragment.
13. `Path`/`Equiv`/`Refine`/`witness`/`cell_*`/`whisker_*` are **derived
    arrows** (interchange + `compose`), not engine primitives; the
    `option_b` op surface collapses into the generic combinators.

### Decision gates

- **Strict vs weak:** resolved — weak (proof-relevant). `law_inference` stays
  as the strict fast-path.
- **Globular vs cubical:** resolved — globular. Reopen only if univalence-
  computing equality becomes a requirement; then as a cube-diagram fragment,
  not a rewrite.


## Why

`research/PAPER.md` describes the target architecture: `M[n]<A,B>` dimension-polymorphic morphisms, laws as higher morphisms, `>>>[k]` axis-parameterized composition, `Family`/`Π`/`Σ`, and effects as higher cells. The `Term` kernel (`plugin-engine/src/term.rs`: `Arr`, `Const`, `Op`, `Seq`, `First`, `Choice`, `Loop`, `App`, `Handle`; `dim: Box<Term>` on `Arr`/`Op`) embodies the shape but has gaps that surfaced as concrete failures:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Fixes unblocked by Phase 1:** `module_returns_closure`,
- **Ops removed, not added:** `cell_comp_*`, `whisker_*`, `cell_id`,
- **`eval_rec`/`rec` region:** superseded by `loop`-encoded recursion where
- **Deferred:** cubical Kan (`transport`/`Equiv` computing), strictness
- **Known debt:** `lower_ast`'s `match node.kind` table (ADR-0105) remains

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0120-paper-implementation-alignment.md`](../design/0120-paper-implementation-alignment.md).
