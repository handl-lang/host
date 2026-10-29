---
title: Generic Object Key Evaluation
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Depends on the data-driven desugar
  pass from [ADR-0053 — Generic Pratt Parser](./0053-generic-pratt-parser.md)
  §7 (`AstDesugarRule`).
supersedes: [ADR-0052]
---

# ADR-0059: Generic Object Key Evaluation

## Context

The `object` prim constructs an object value from entry nodes. Each entry has
a key child and a value child. The key child can be:

1. A `var` node — use the identifier name as the key string.
2. A `string` node — use the literal string.
3. An `array` node with one child — evaluate the inner expression and use
   its value's string representation (ADR-0057 computed bracket keys).
4. Any other node — evaluate it and use its value's string representation
   (the parenthesized `(expr):` form).

This rule is implemented in **Rust** in each of the three eval engines
(`plugin-engine-exec-tree-walk`, `plugin-engine-exec-bytecode`,
`plugin-engine-exec-cranelift`). Adding a new key form (as ADR-0057 did)
required changing all three engines in lockstep. This violates the
platform's design principle: language rules should be **data**, not engine
code (ADR-0053 §1, ADR-0052 §"Why a kernel").

The desugar pass (ADR-0053 §7) already provides a data-driven `Ast → Ast`
rewrite mechanism: an `AstShapeDef` with a `desugar` rule rewrites nodes of
that kind into a `target_kind` with specified children, before evaluation.
The `if → match` desugar is the canonical example. This pass runs once,
between parse and eval, in `Runtime::parse` — all engines consume the
desugared AST.

## Decision

Move object-key resolution from the eval engines into the **desugar pass**,
expressed as data.

### Step 1: Desugar `[expr]:` entries at parse time

Add a desugar rule on the `entry` AST shape that detects array-keyed entries
and rewrites them into a canonical "computed entry" form before any engine
sees them. The rewrite:

```
entry { children: [array { children: [expr] }, value] }
  →  entry { children: [expr, value] }
```

The array wrapper is stripped; the inner expression becomes the key child
directly. After desugar, no entry's key child is ever an `array` node.

### Step 2: Simplify the engine key rule to two cases

With array keys eliminated by desugar, each engine's `object` prim reduces
to:

- **`var` or `string`** — use the payload string directly (literal key).
- **anything else** — evaluate the expression and use its value's string
  representation (computed key).

This is already the "else" branch that existed before ADR-0057. The
`array`-key branch added by ADR-0057 is removed from all three engines.

### Step 3: Future key forms are desugar rules, not engine changes

Adding a new key form (e.g. template-literal keys, symbol keys) becomes a
desugar rule on the `entry` shape in kit data — no engine code changes.

## Consequences

- **One rule, not three.** The key-resolution logic lives in one desugar
  rule, not in three Rust engines. Adding or changing key forms is a data
  change to `ast.yaml`.
- **Engines get simpler.** Each engine's `object` prim drops the `array`-key
  branch (and eventually can drop the `var`/`string` special cases too, if
  those are also moved to desugar).
- **No runtime cost.** The desugar pass already runs between parse and eval;
  stripping an array wrapper is a single node rewrite, amortized once.
- **ADR-0057's Rust changes become transitional.** The per-engine
  `array`-key branches added by ADR-0057 are removed once the desugar rule
  is in place. ADR-0057's _language design_ (bracket computed keys are
  valid) stands; its _implementation_ is superseded.
- **The parenthesized `(expr):` form needs no desugar** — it already
  produces a `paren` node, which falls into the "evaluate" branch unchanged.
