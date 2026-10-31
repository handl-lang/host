+++
id = "ADR-0061"
title = "ADR-0061: Generic Pattern Match"
status = "open"
priority = "medium"
+++

---
title: Generic Pattern Match
type: adr
date: 2026-09-01
status: accepted
status_note: |-
  Accepted

  Pays the last remaining ADR-0058 debt: pattern matching (`try_pattern`
  in `match`/`handle`). Follows the direction established by
  [ADR-0058 — Generic Data-Driven Engines](./0058-generic-data-driven-engines.md)
  §"Existing hardcoded logic is debt", and the template of
  [ADR-0059 — Generic Object Key Evaluation](./0059-generic-object-key-evaluation.md)
  and [ADR-0060 — Generic Call Dispatch](./0060-generic-call-dispatch.md).
---


# ADR-0061: Generic Pattern Match

## Decision


### De-duplicate: extract `try_pattern` into a shared module

Extract `try_pattern`, `try_array_pattern`, `try_object_pattern`, and
`rest_binder_name` into a shared module in
`handl-plugin-engine-exec` (the abstraction crate all three engines
already depend on). The shared module exposes a `PatternMatchCtx` trait
that abstracts the two engine-specific operations pattern matching
needs:

- **`define_binding`** — bind a name to a value in the current scope
  (for binder and rest patterns). Each engine's `ScopeStack` implements
  this.
- **`eval_pattern_node`** — evaluate a pattern AST node as a runtime
  value (for literal patterns). Each engine's eval function implements
  this.

The shared `try_pattern` function takes `&Ast`, `AstRef` (the pattern
node), `&Value` (the scrutinee), and `&mut impl PatternMatchCtx`. All
pattern-node-kind dispatch (wildcard, binder, array, object, literal)
lives in this one function. The three engines' `prim_match`/`prim_handle`
call the shared `try_pattern` instead of their own copies.

This eliminates the 3x duplication: adding a new pattern form is now a
one-place change in the shared module, not three engine edits.

### What stays in Rust (for now)

The `prim_match` and `prim_handle` functions stay in each engine. They
contain the arm-iteration loop (evaluate scrutinee, iterate arms,
push/pop scope, call `try_pattern`, evaluate body on match). This logic
is thin — it's the `Choice`/`Loop` structure that the kernel combinators
would express — and it delegates all pattern interpretation to the
shared module.

The `match` vs `handle` semantic difference (match errors on no arm,
handle returns the body value) stays in the engine-level prim, where it
belongs: it's a control-flow decision, not a pattern-form decision.

### Path to full data-driven (future work)

A future ADR can migrate pattern matching fully into `eval.yaml` when
the kernel gains the combinators needed to express it cleanly. The
likely shape:

1. **Desugar patterns into match-op AST nodes.** Each pattern form
   becomes a node kind with a scheme: `pattern_wildcard` (always
   matches), `pattern_binder` (binds name), `pattern_literal` (eval +
   compare), `pattern_array` (positional + rest), `pattern_object`
   (keyed + rest). The desugar pass rewrites `arm` children from
   arbitrary AST nodes into these canonical pattern nodes.

2. **Express `match`/`handle` as `Loop` + `Choice`.** The arm-by-arm
   search becomes a `Loop` whose body tries one arm: `Seq` of
   `try_pattern_arm` (a native op that calls the shared `try_pattern`
   and returns an `Either` — `Left(bindings)` on match, `Right(())` on
   no match) and a `Choice` that either breaks the loop (match → eval
   body) or continues (no match → next arm).

3. **The shared `try_pattern` becomes the `try_pattern_arm` native op.**
   The pattern-form dispatch stays in Rust (it's a fixed interpreter of
   pattern-node data), but the arm-iteration and scope management move
   to the scheme level.

This path is deferred because the kernel combinators needed for
variable-length arm iteration (`Loop` with early exit via `Choice`) are
not yet exercised in this pattern, and the desugar infrastructure for
patterns would need new `AstShapeDef` entries for each pattern form. The
de-duplication in this ADR is the prerequisite: it consolidates the
pattern-form dispatch into one place, so the future migration changes
one module, not three engines.


## Why

ADR-0058 established that eval engines should be fixed interpreters of `Language.eval` data. ADR-0059 migrated object-key resolution to a desugar rule. ADR-0060 migrated `call` dispatch to a scheme description. The remaining ADR-0058 debt is **pattern matching** — the `try_pattern` function and its helpers (`try_array_pattern`, `try_object_pattern`, `rest_binder_name`) are duplicated across all three engines with identical logic:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **One implementation, not three.** `try_pattern` and its helpers live
- **Engines converge further.** Each engine's `prim_match`/`prim_handle`
- **`PatternMatchCtx` trait is minimal.** Two methods (`define_binding`,
- **Tests are preserved.** The existing pattern matching tests
- **ADR-0058's pattern matching debt is paid.** ADR-0058 §"Existing

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0061-generic-pattern-match.md`](../design/0061-generic-pattern-match.md).
