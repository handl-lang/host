---
title: Canonical Term Arrows
type: adr
date: 2026-09-14
status: proposed
status_note: Accepted — the canonical form is `Term`; no function/call/block/assign/object nodes.
---

# ADR-0100: Canonical Term Arrows

## Decision

1. **The canonical internal form is the arrow-calculus `Term`** — no
   `function`, `call`, `block`, `assign` or `object` nodes.
2. **The constructor set is fixed**: `Arr`, `Const`, `Op`, `Seq`, `First`,
   `Second`, `Choice`, `Loop`, `App`, `Handle`.
3. **`x ~> body` lowers point-free** using `arr` and the product combinators;
   a body that is not point-free expressible lifts to a pure `FunctionValue`
   wrapped in `Arr`.
4. **Calls lower to `App`** when the callee is dynamic and to `Seq` when it is
   a known arrow; `do` and `=` lower to `Seq` and are not semantic nodes.
5. **Records, operators and literals lower to `Op`/`Arr`/`Const`**; there are
   no object or operator nodes.
6. **The engine interprets `Term` directly**; `eval.yaml` describes how each
   constructor is interpreted.

## Why

The AST had a node per surface convenience (function, call, block, assign,
object), so every engine re-implemented their semantics. Lowering to `Term`
gives one graph and one interpretation table; sugar disappears at the boundary.

## Not doing
- **Keep surface nodes in the canonical form** — they are sugar.
- **A second evaluator per node kind** — one `Term` interpreter.
- **Keep `=` as a semantic node** — it is threading through `Seq`.

## Consequences
- One canonical graph; engines interpret constructors, not syntax.
- Non-point-free bodies cost a `FunctionValue` lift.
- `eval.yaml` becomes the interpretation table.

## Enforcement
- `kernel/` — `Term` constructors and the interpreter.
- `crates/handl-kit-data-canonical/data/eval.yaml` — constructor schemes.
- Lowering tests (surface → `Term`).

## Design
+The constructor list, lowering rules and the engine switch:
+[`design/0100-canonical-term-arrows.md`](../design/0100-canonical-term-arrows.md).
