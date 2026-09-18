# ADR-0120: Paper–Implementation Alignment Plan

## Status

Accepted — direction decided in review; execution is phased below.

Refines [ADR-0119 N-Dimensional Arrow Calculus](./0119-n-dimensional-arrow-calculus.md)
and `research/PAPER.md`. Supersedes the lambda-flavoured "arrows as closures"
proposal considered and rejected in the same review.

## Context

`research/PAPER.md` describes the target architecture: `M[n]<A,B>`
dimension-polymorphic morphisms, laws as higher morphisms, `>>>[k]`
axis-parameterized composition, `Family`/`Π`/`Σ`, and effects as higher cells.
The `Term` kernel (`plugin-engine/src/term.rs`: `Arr`, `Const`, `Op`, `Seq`,
`First`, `Choice`, `Loop`, `App`, `Handle`; `dim: Box<Term>` on `Arr`/`Op`)
embodies the shape but has gaps that surfaced as concrete failures:

- `module_returns_closure` and the `Nat.rec` self-application (`inner(inner)`
  in `desugar.data`'s `build_recursor`) fail because the calculus cannot apply
  an arrow to an arrow-value.
- `option_b`/`path_transport` expose hardcoded per-dimension ops
  (`cell_comp_h`, `cell_comp_v`, `whisker_*`, `cell_id`, `witness`, `refine`)
  that violate the single-calculus rule.
- The `dim` field doubles as an effect-grade string for `handle` routing and a
  morphism-dimension index — two concepts in one slot.

Three decisions from the design review drive the plan:

1. **The language is a pure arrow/combinator calculus.** No lambdas, no
   closures, no currying. There are no free variables to capture: the lowerer's
   `env` is compile-time substitution into products/projections
   (`id`/`proj_child`), not runtime scope. An "arrow value" is a quoted `Term`
   (`arr(Term)`); application is `app`; recursion is `loop`.
2. **Laws are proof-relevant ("weak") cells, not erased equalities.**
   `α : f ~[2]~> g` is a first-class value composable via `>>>[k]` and
   relatable via `~[3]~>`. (`law_inference` normalization remains as the
   strict fast-path where a canonical form suffices.)
3. **Cell shape is globular.** `M[n]<A,B>` with `A,B ∈ M[n-1]` is already the
   globular presentation, matches the directed/non-invertible nature of
   computations and rewrites, and requires no new cell vocabulary. Cubical
   Kan structure (`hcomp`, `transport`, `Equiv`) is deferred: it is only
   needed for univalence-style _computing_ equality, which fits an
   ∞-groupoid fragment better than the general directed calculus.

A fourth consequence follows from 1–3: **Kan filling is generalized
composition.** In a weak globular setting, "every composable pasting diagram
has a composite" is the Kan condition — so `hcomp` is not a new primitive.
`compose(pasting_diagram)` (the n-ary generalization of `Seq`) covers it:

| Hardcoded op                     | Becomes                                |
| -------------------------------- | -------------------------------------- |
| `cell_comp_h` / `cell_comp_v`    | `>>>[0]` / `>>>[1]` — binary `compose` |
| `whisker_left` / `whisker_right` | `id >>>[k] α` — derived, not an op     |
| `cell_id`                        | `id` — already primitive               |
| `hcomp`                          | `compose(open_diagram)` — general case |

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

## Consequences

- **Fixes unblocked by Phase 1:** `module_returns_closure`,
  `data_recursor_step_case_one`, `data_recursor_recurse_depth_two`, and the
  `indexed_family_*` recursor paths — all currently blocked on
  arrow-application-of-arrows.
- **Ops removed, not added:** `cell_comp_*`, `whisker_*`, `cell_id`,
  `hcomp`-style ops fold into `compose`/`>>>`/`id` — one generic combinator
  replaces ~5 hardcoded ones, per the single-calculus rule.
- **`eval_rec`/`rec` region:** superseded by `loop`-encoded recursion where
  the fixpoint is structural; the region form may remain for general
  fixpoint blocks.
- **Deferred:** cubical Kan (`transport`/`Equiv` computing), strictness
  metatheory, per-axis coherence proofs.
- **Known debt:** `lower_ast`'s `match node.kind` table (ADR-0105) remains
  hardcoded pending a `lower` scheme fragment per `AstShapeDef`; the
  typecheck model's fuel circuit breaker is tactical, not a fix.
