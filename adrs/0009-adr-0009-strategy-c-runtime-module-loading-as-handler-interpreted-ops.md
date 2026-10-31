+++
id = "ADR-0009"
title = "ADR-0009: Strategy C — Runtime Module Loading as Handler-Interpreted Ops"
status = "open"
priority = "medium"
+++

---
title: Strategy C — Runtime Module Loading as Handler-Interpreted Ops
type: adr
date: 2026-10-16
status: proposed
status_note: Accepted — runtime module loading is the handler, not a strategy.
---

# ADR-0009: Strategy C — Runtime Module Loading as Handler-Interpreted Ops

## Decision

1. **Strategy C is not a separate strategy.** `module_ref` and `project` are
   `arr` ops already emitted by `rewrite_imports`; a handler with clauses for
   them interprets the graph directly. No new `Scheme` variants.
2. **The handler's operation table is the opt-in.** A handler without the
   clauses requires the graph to be normalised (Strategy A/B) or refuses with
   `unknown op`; there is no directive and no capability flag.
3. **Governance is handler configuration**: path allowlist, load-depth limit,
   resolvers (file-only by default; network only if configured), and runtime
   cycle detection on the load stack.
4. **Two static passes work without executing**: cycle detection over the
   import graph, and contract inference over `module_ref`/`project` (permissive
   contracts, tightening via the loader's contract cache).
5. **Graduate to real `Scheme` variants only** when a second handler needs
   them and stringly matching causes real problems; until then the closed
   six-combinator kernel holds.

## Why

Runtime loading looked like a third strategy, so it grew a directive, a
capability flag and a separate code path. The ops already exist; a handler
either interprets them or refuses, which is the same opt-in model as every
other operation.

## Not doing
- **A `-- @strategy runtime` directive** — the handler's table is the switch.
- **A `supports_runtime_loading` flag** — it duplicates the table.
- **New `Scheme` variants** — `arr` ops are enough.

## Consequences
- One graph, two handler behaviours (with/without clauses).
- Governance lives in handler config, not workspace policy.
- Static analysis works on the unnormalised graph.

## Enforcement
- `handl-runtime/` — `module_ref`/`project` ops and the loader.
- Handler config for allowlist/depth/resolvers.
- Cycle-detection and contract-inference tests.

## Design
+The five sections in full, handler clauses, governance and static passes:
+[`design/0009-strategy-c-runtime-module-loading.md`](../design/0009-strategy-c-runtime-module-loading.md).
