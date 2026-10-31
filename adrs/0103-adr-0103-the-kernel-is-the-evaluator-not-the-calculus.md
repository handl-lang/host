+++
id = "ADR-0103"
title = "ADR-0103: The Kernel Is the Evaluator, Not the Calculus"
status = "open"
priority = "medium"
+++

---
title: The Kernel Is the Evaluator, Not the Calculus
type: adr
date: 2026-09-10
status: accepted
status_note: Accepted — the kernel is a bytecode VM; the calculus lives in handl models.
---

# ADR-0103: The Kernel Is the Evaluator, Not the Calculus

## Decision

1. **The kernel is the evaluator, not the calculus**: a bytecode VM over values,
   with no named-op surface.
2. **The ISA is small and structural**; anything expressible as data is not a
   native op.
3. **Computation is represented as data** (`TermData` over `serde_json::Value`),
   and that representation is the serialization boundary.
4. **`exec_model.hnl`** is the scheme interpreter over VM primitives.
5. **`arrow_model.hnl`** implements all `Term` tags of the arrow calculus.
6. **`type_model.hnl`** implements the HoTT type judgments.
7. **`meta_model.hnl`** implements module loading and evaluation.
8. **The categorical picture**: composition and the arrow primitives are the
   semantics; evaluation-by-name is not.
9. **Rust keeps the VM and the model adapters**; the models are handl data.

## Why

The kernel conflated the calculus with its evaluator, so the calculus was
re-implemented as Rust enums and `match` arms that duplicated the data. A small
bytecode VM plus handl models keeps the kernel stable and makes the calculus
editable as data.

## Not doing
- **Named ops in the kernel** — they are model data, not VM instructions.
- **Rust `match` arms per `Term` tag** — the models implement the tags.
- **Dual Rust/handl models** — one source of truth, the handl model.

## Consequences
- The ISA is the ABI; changing it is a kernel change, changing a model is data.
- `TermData` becomes the serialization boundary for every engine.
- Performance work moves to lowering and the models, not the kernel.

## Enforcement
- `kernel/` — the bytecode VM and value representation.
- `project/design/adr0103/*.hnl` — `exec_model`, `arrow_model`, `type_model`,
  `meta_model`.
- `engine-exec-*` — the interpreters that agree value-for-value.

## Design
+The insight, architecture, ISA, serialization boundary and the model suite:
+[`design/0103-kernel-is-evaluator-not-calculus.md`](../design/0103-kernel-is-evaluator-not-calculus.md).
