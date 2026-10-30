---
title: Arrow Calculus Kernel
type: adr
date: 2026-08-31
status: accepted
status_note: Accepted — `app` is the primitive; the calculus is the only execution path.
---

# ADR-0052: Arrow Calculus Kernel

## Decision

1. **`app` is the primitive; interpretation is the only execution mechanism.**
2. **Semantic principle: arrows are values** and computation is composition —
   no implicit environment or evaluation-by-name.
3. **Kernel representation**: `Term`/`Value` with the arrow constructors
   (`id`, `compose`, product, curry, …); a closed term is a value.
4. **The lowering table (surface construct → arrow form) is normative**: every
   construct is defined by the arrow it lowers to.
5. **Derived forms** (`let`, `if`, loops) are sugar over the kernel, not
   primitives.
6. **`proc` notation and `x = y` binding are the authoring surface**; they
   lower to arrows like any other sugar.
7. **Big-bang migration**: one cutover, the old `Scheme` removed, no dual path.
8. **Handlers are outside the calculus**; execution is one interpretation of
   the arrow.
9. **Feedback semantics** (causal/strict vs lazy/fixpoint) are specified
   separately, never assumed.
10. **Scope**: kernel + bootstrap + canonical `proc` surface; everything else
    derives.

## Why

The language had several execution paths and a binding model inside the
kernel, so the same program could mean different things depending on which path
ran it. One primitive plus a lowering table closes the calculus: every
construct has exactly one arrow reading.

## Not doing
- **Keep the old `Scheme` beside the kernel** — two execution paths, two
  semantics.
- **Put handlers inside the calculus** — handlers interpret arrows; they are
  not arrow constructors.
- **Assume one feedback semantics** — causal and lazy fixpoint differ; the
  language picks per definition.

## Consequences
- Every surface form must lower; a construct with no arrow reading is not in
  the language.
- The bootstrap and `proc` surface are canonical; other syntax is sugar.
- Handlers stay external and composable.

## Enforcement
- `kernel/` — `Term`/`Value` and the arrow constructors.
- `crates/handl-kit-data-canonical/data/eval.yaml` — the schemes.
- `engine-exec-*` — the interpreters that agree value-for-value.

## Design
+The lowering table, `proc` surface, handlers and feedback semantics:
+[`design/0052-arrow-calculus-kernel.md`](../design/0052-arrow-calculus-kernel.md).
