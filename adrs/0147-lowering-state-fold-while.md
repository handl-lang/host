---
title: Lower the `while` state fold to `Loop` + explicit protocol ops
type: adr
date: 2026-10-17
status: accepted
---

# ADR-0147: Lower the `while` state fold to `Loop` + explicit protocol ops

## Status

Accepted — implemented; pilot for retiring the "non-input position"
decline class.

## Context

`handl-kit-arrow-lower` instantiates a node's `lang.eval` scheme by
substituting `proj_child`/`proj_rest`/`Child` placeholders with the node's
lowered children. A placeholder is only honest where the position receives
the *node's own input*: in the env-based engines a placeholder re-evaluates
the AST child in the ambient env and ignores its incoming value, while a
lowered child is an arrow that consumes its input. Positions fed a computed
value — `Seq.snd`, `Choice` arms, `Loop.body`, `First`, `App` args —
therefore decline, and the runtime falls back to AST eval.

`eval.while` is `Seq[proj_child 0, Loop[loop_step(proj_child 1)]]`: the
step placeholder sits in `Loop.body`. `while` is the dominant control
construct of the arrow dialect (the `concurrent-epoll` scheduler, HTTP
server, codec and client packages fold state with it — ~40 sites), so the
decline kept every one of those functions on the interpreter.

## Decision

Lower `while` in `handl-kit-arrow-lower::lower_while` (the `lower_if`/
`lower_loop` precedent: per-kind arms for constructs whose schemes are
env-engine-shaped):

```
while init { step }   →   Seq[ init , Loop[ body ] ]
```

`Loop`'s body runs on the `[in, st]` pair (a 2-array — `proj_child`
projects it). `body` reproduces the `loop_step` protocol in combinators:

1. **current** = `st == null ? in : st`, expressed as
   `product(eq(proj_child 1, null), id) >>> pair_tag >>> Choice(proj_child
   0, proj_child 1)` — `pair_tag` turns `[cond, pair]` into `Either pair`,
   so the chosen arm still sees the pair and projects `in`/`st`.
2. The step arrow applies to `current` destructured **by param name**
   (`array[field_access p0, field_access p1, …]` for multi-param steps;
   `id` for a single param) — the `build_loop_step_args` contract.
3. A new `loop_outcome` op maps the step result: `{state: v}` → `right(v)`
   (continue), `{out: v}` → `left(v)` (terminate), bare `v` → `left(v)` —
   exactly `loop_step_outcome` in `plugin-engine-exec`. It is implemented
   in both `plugin-engine`'s term interpreter and `apply_native`, so the
   emitted term keeps its meaning whether it runs on the arrow interpreter
   or embedded in a `__term__`/handler term under the tree-walk.

A step that captures enclosing parameters (e.g. the predicate `p` in
`drop_where`) lowers to a `{__term__, __bound__}` bound-record producer
whose `__bound__` positions project the *enclosing* input. Under `Loop`
the incoming is `[in, st]`, not the enclosing input — so the producer is
hoisted: `Seq[ product(init, step), Loop[body'] ]`, and `body'` applies
the materialized record (`App`) to the name-destructured state. This is
the same hoist `lower_loop` already uses for its record-producing steps.

Steps that are not function literals decline to the AST fallback, as
before — the scheme is unchanged for the env engines.

## Consequences

- `while` folds compile to the variable-free calculus; the printed
  "falling back to AST eval" declines in the arrow suites drop to zero
  (`ws-c` had 3, all `while`).
- `eval.while` stays as the tree-walk scheme — the two paths now agree on
  the same protocol by construction (`loop_outcome` ≙
  `loop_step_outcome`).
- This is the template for the remaining decline class (`eval.ternary`,
  `eval.nullish`, `eval.type-annot`, `eval.dimension`): where a scheme
  needs a computed value alongside the node input, carry the input through
  `product`/`pair_tag` pairs and let arms project — never substitute a
  child placeholder where a computed value flows.
- `match`/`data` are untouched: they still need the `desugar.*` natives
  (per-constructor iteration is a macro job, not a scheme shape).
