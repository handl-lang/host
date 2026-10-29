---
title: Lower `call`/`dimension` application to `App` terms
type: adr
date: 2026-10-17
status: accepted
---

# ADR-0148: Lower `call`/`dimension` application to `App` terms

## Status

Accepted — implemented; extends the ADR-0147 decline-class retirement to
the application family.

## Context

ADR-0147 established the pattern: an `eval.*` scheme whose `proj_child`/
`proj_rest` placeholders sit in non-input positions (`Seq.snd`, `Choice`
arms, `Loop.body`) cannot be instantiated honestly for the arrow calculus,
so the lowerer declines and the runtime falls back to the AST engine.
`eval.while` was retired by a per-kind lowering arm that emits the
`product`/`pair_tag`/`Choice` plumbing directly.

`eval.call` and `eval.dimension` share the same routing shape:

```
Seq[ Seq[ proj_child 0 , route_on_type("string") ],
     Choice[ call_host(proj_rest) , App(id, proj_rest) ] ]
```

— callee in `Seq.snd`, both `proj_rest` placeholders in `Choice` arms:
every position non-input, so every `f(x)` in every `.hnl` program stayed
on the interpreter. Application is the dominant expression form; this
decline was the largest single source of interpreter traffic.

The `Choice` exists because the *env engines* must route a string callee
to `call_host` and an arrow callee to `App`. On the arrow path that
routing is unnecessary: `Term::App` evaluation already classifies the
callee value at run time — `Str` → `call_host_fn` (module registry),
`Arrow` → `apply_app`, `{__term__, __bound__}` record → bound-tuple
extension, `arr`-dispatch object → `arr_dispatch` — and `App.callee` /
`App.args` are input-faithful positions already (each arg term runs on
the node input). No `product`/`pair_tag` plumbing is needed because the
discriminator the `Choice` encodes *is* the callee value, which `App`
inspects itself.

## Decision

`call` and `dimension` nodes lower through `lower_call`
(`crates/handl-kit-arrow-lower/src/lib.rs`), a per-kind arm that emits
`App`/`Op` terms directly:

- **`var` callee bound in the compile-time env** — substitute the bound
  term (quoted via `ensure_quoted` unless already an arrow value), then
  apply the lowered arguments: a binding declared with `arity > 1`
  consumes its first `arity` arguments as one tuple `App` (matching the
  multi-parameter application contract), with any surplus applied as
  curried single-argument `App`s; ordinary bindings apply sequentially
  (`App(App(f, a), b)`).
- **`var` callee bound as a parameter** — project it out of the input
  tuple (`id` for a single parameter, `elem <pos>` for several), then
  sequential `App`s.
- **`var` callee unknown to the env** — `Op{name, args}`: a native-op
  call, resolved at run time through `dispatch_op` (covers host ops and
  `op` declarations alike).
- **`call`-face callee** (a nested `call`/`dimension`, per the ADR-0123
  face check, falling back to `kind == "call"` when a language declares
  no faces) — one `App` over the whole argument list: the callee
  produces a bound-arrow record and tuple application extends
  `__bound__` flatly, matching the engines' `{__term__, __bound__}`
  semantics (`Type.rec(m, c0..)(params.., v)`).
- **any other callee** — sequential `App`s, preserving curried
  application for arrow-valued expressions.

`eval.call`/`eval.dimension` in `eval.yaml` stay unchanged: they remain
the eval schemes for the env-based engines. The two paths agree because
`Term::App` evaluation performs the same dispatch the `Choice` encodes —
the schemes describe *the route*, the `App` term carries it.

## Consequences

- Application — the dominant expression form — compiles to the
  variable-free calculus; `call`/`dimension` nodes no longer force the
  AST fallback. The `concurrent-epoll`/`http_rfc`/`serve-epoll` package
  stack lowers end to end; remaining declines are the intentional ones
  (self-recursive definitions, `return`) plus `eval.type-annot`, whose
  `extend_env` arm still needs the env engine.
- The pilot confirms the refined rule: pair plumbing is needed only when
  a *computed* discriminator selects between child placeholders
  (`if`/`ternary`/`nullish`/`&&`/`||`/`while`). When the discriminator
  is a value the dispatch itself inspects (`call`), the routing lives in
  `App` evaluation and the scheme's `Choice` dissolves entirely.
- `eval.type-annot` is the last ordinary decline of the class
  (`Seq[Seq[proj_child 1, extend_env], proj_child 0]` — the value child
  sits in `Seq.snd`); it declines honestly until it gets the same
  per-kind treatment.
