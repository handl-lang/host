# ADR-0061: Generic Pattern Match

## Status

Accepted

Pays the last remaining ADR-0058 debt: pattern matching (`try_pattern`
in `match`/`handle`). Follows the direction established by
[ADR-0058 — Generic Data-Driven Engines](./0058-generic-data-driven-engines.md)
§"Existing hardcoded logic is debt", and the template of
[ADR-0059 — Generic Object Key Evaluation](./0059-generic-object-key-evaluation.md)
and [ADR-0060 — Generic Call Dispatch](./0060-generic-call-dispatch.md).

## Context

ADR-0058 established that eval engines should be fixed interpreters of
`Language.eval` data. ADR-0059 migrated object-key resolution to a
desugar rule. ADR-0060 migrated `call` dispatch to a scheme description.
The remaining ADR-0058 debt is **pattern matching** — the `try_pattern`
function and its helpers (`try_array_pattern`, `try_object_pattern`,
`rest_binder_name`) are duplicated across all three engines with
identical logic:

- **Tree-walk**: `crates/plugin-engine-eval-tree-walk/src/lib.rs`
- **Bytecode**: `crates/plugin-engine-eval-bytecode/src/vm.rs`
- **Cranelift**: `crates/plugin-engine-eval-cranelift/src/lib.rs`

Each engine's `match`/`handle` prim calls `try_pattern`, which
interprets a pattern AST node structurally against a scrutinee value:

1. **Wildcard** — `var` node named `_` or `_*` → always matches.
2. **Binder** — `var` node with another name → binds name to scrutinee,
   always matches.
3. **Literal** — any other node → evaluate and compare by `==`.
4. **Array destructuring** — `array` node with sub-pattern children +
   optional `rest` node → positional match, `...rest` captures tail.
5. **Object destructuring** — `object` node with `entry` children +
   optional `rest` node → key match, `...rest` captures unmatched keys.

The pattern forms are hardcoded: adding a new pattern form (e.g. `or`
patterns, `is` type tests, range patterns) requires editing all three
engines in lockstep — exactly the violation ADR-0058 names.

### Why pattern matching is harder than `call` or `object`

ADR-0060 migrated `call` by expressing its two-way callee dispatch as a
`Choice` in `eval.yaml`. That worked because `call`'s dispatch is
**flat**: one value, one type test, two branches. The `Choice`
combinator is a natural fit.

Pattern matching is **recursive with stateful bindings**:

- An `array` pattern contains sub-patterns, each of which may itself be
  an `array` or `object` pattern, recursively. Each sub-pattern may bind
  variables into the environment.
- The number of arms is variable (one per `match` clause), and each arm
  must be tried in order until one matches — a sequential search, not a
  two-way `Choice`.
- A failed arm must discard its bindings (scope pop), while a successful
  arm's bindings persist into the body evaluation (scope kept).
- `...rest` captures are computed from the *remaining* elements/keys
  after positional/entry matching, requiring access to the scrutinee's
  structure mid-recursion.

The kernel combinators (`Arr`, `Seq`, `First`, `Choice`, `Loop`, `App`)
can in principle express this — `Loop` for the arm-by-arm search,
`Choice` for match/no-match, `First` for pair threading of
bindings — but the resulting scheme would be deeply nested, require new
native ops for pattern-node-kind dispatch and binding, and be harder to
read than the Rust it replaces. The cost/benefit ratio is unfavorable
today: the pattern forms are stable, and the duplication is the
immediate problem.

## Decision

### De-duplicate: extract `try_pattern` into a shared module

Extract `try_pattern`, `try_array_pattern`, `try_object_pattern`, and
`rest_binder_name` into a shared module in
`handl-plugin-engine-eval` (the abstraction crate all three engines
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

## Consequences

- **One implementation, not three.** `try_pattern` and its helpers live
  in `handl-plugin-engine-eval::pattern_match`. Adding a new pattern
  form (or-patters, type tests, ranges) is a one-place change.

- **Engines converge further.** Each engine's `prim_match`/`prim_handle`
  is reduced to the arm-iteration loop + body evaluation. All pattern
  interpretation is shared.

- **`PatternMatchCtx` trait is minimal.** Two methods (`define_binding`,
  `eval_pattern_node`) — the smallest interface that captures the
  engine-specific operations pattern matching needs. No new kernel
  combinators, no new `ArrSource` variants.

- **Tests are preserved.** The existing pattern matching tests
  (`match_wildcard_pattern`, `match_binder_pattern`,
  `match_literal_pattern`, `match_array_destructuring`,
  `match_array_rest`, `match_object_destructuring`,
  `match_object_rest`, `handle_dispatches_to_matching_clause`,
  `handle_no_clause_returns_body`) and integration tests in
  `fp_integration.rs` become the conformance baseline. The behavior they
  assert is preserved exactly; only the location of the implementation
  changes.

- **ADR-0058's pattern matching debt is paid.** ADR-0058 §"Existing
  hardcoded logic is debt" named pattern matching as the last deferred
  item. This ADR pays it by eliminating the 3x duplication and
  consolidating pattern-form dispatch into one shared module. The full
  data-driven migration (schemes in `eval.yaml`) is documented as future
  work with a clear path.
