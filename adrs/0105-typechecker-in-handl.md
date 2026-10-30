---
title: Typechecker in Handl
type: adr
date: 2026-09-10
status: accepted
status_note: Accepted — the typechecker models live in handl; Rust keeps the engine.
---

# ADR-0105: Typechecker in Handl

## Decision

1. **The typechecker's models are handl**; Rust keeps only the engine/driver.
2. **What stays in Rust**: the engine, VM primitives and the adapter that loads
   the model.
3. **What moves to handl**: the type model (judgment ops) plus the scope and
   totality models.
4. **Bootstrap**: Rust embeds the `.hnl` model with `include_str!`, loads it into
   a `HandlModule`, and calls its `typecheck` function.
5. **The adapter is thin** — the model is the implementation, not a wrapper.
6. **Parity is proven against the Rust model** by the existing type-model
   tests.

## Why

The typechecker was the last stage with its semantics in Rust, so a language
change meant an engine change and the rules could not be inspected. A handl
model plus a thin adapter keeps the engine generic and makes typing data.

## Not doing
- **Keep the typechecker in Rust** — the last stage outside language-as-data.
- **Embed the model as a Rust port** — the `.hnl` source is embedded and loaded.
- **A fat adapter** — it only loads and calls.

## Consequences
- The type model is handl; the engine is generic.
- Bootstrap depends on `include_str!` + `HandlModule`; no separate install step.
- Parity tests gate the switch.

## Enforcement
- `project/design/adr0103/typecheck_model.hnl`, `scope_model.hnl`,
  `totality_model.hnl`.
- `engine-typecheck-nbe` — the engine + adapter.
- Type-model parity tests.

## Design
+Architecture, Rust/handl boundary, bootstrap and the model suite:
+[`design/0105-typechecker-in-handl.md`](../design/0105-typechecker-in-handl.md).
