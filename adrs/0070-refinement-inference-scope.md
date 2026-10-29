---
title: Refinement Inference Scope — Deterministic Dataflow
type: adr
date: 2026-09-01
status: accepted
status_note: |-
  Accepted

  Builds on [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md),
  [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md),
  and the refinement inference engine in `crates/plugin-ext-refinement/`.
---

# ADR-0070: Refinement Inference Scope — Deterministic Dataflow

## Context

The refinement inference engine (`plugin-ext-refinement`) derives
semantic guarantees from arrow graph structure with zero user
annotations. It works by:

1. Reading the graph (which combinators are wired together)
2. Looking up axioms (what the leaf atoms guarantee)
3. Composing (Seq conjoins, First preserves side-channel, Choice branches, Loop finds fixpoint)
4. Checking implications (can we prove guarantee ⟹ postcondition?)

This is deterministic structural derivation — no search, no heuristics,
no machine learning. The graph IS the proof.

The question: **what kinds of programs does this work for?**

## Decision

The inference mechanism applies to **deterministic dataflow** — programs
where the graph structure fully determines the behavior. This is the
scope boundary for the current architecture.

### In scope

- **List operations**: sort, map, filter, fold, insert, take, drop, zip,
  scan, reduce. Proven: 16 arrows, all inferred with zero annotations.
- **Parsers (recursive descent, no backtracking)**: Pratt parsing,
  token consumption, precedence climbing. Proven: 1 arrow (Pratt parser),
  infers LengthSubOne (tokens consumed) and Nested ListLength (terminates).
- **State machines (data invariants)**: counters, bounded buffers,
  protocol states. The state machine is a loop over event handlers;
  the fixpoint discovers data invariants like Count ≥ 0.

### Out of scope (current architecture)

- **Parsers with backtracking**: need failure as a first-class effect.
  The current Choice combinator doesn't model failure propagation.
  See ADR-0071.
- **Concurrent systems**: need non-deterministic scheduling and
  synchronization. Arrows are deterministic dataflow. See ADR-0072.
- **Non-deterministic computation**: the inference assumes the graph
  structure determines the output. Non-determinism breaks this.

### Why this boundary

The inference reads the proof off the graph structure. This requires
that the graph structure fully determines the program's behavior. If
the behavior depends on factors not visible in the graph (scheduling,
failure, external state), the inference cannot see them.

The good news: deterministic dataflow covers most business logic, most
data transformation, and most algorithms. The cases that don't fit
(concurrency, backtracking) are exactly where traditional verification
also struggles.

## Consequences

### Positive

- Clear scope: the inference engine targets deterministic dataflow.
- Proven results: 17+ arrows across 3 domains (lists, parsers, state
  machines), all inferred with zero annotations.
- Compositional: contracts flow through imports (ADR-0069).
- The boundary is honest — we don't claim to handle what we can't.

### Negative

- Concurrency, backtracking, and non-determinism are excluded.
- Users working in those domains need different verification approaches.
- Extending to those domains requires new arrow calculus extensions,
  not just new axioms.

## Related ADRs

- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md)
- [ADR-0071 — Failure-Tracking Arrows for Backtracking Parsers](./0071-failure-tracking-arrows.md)
- [ADR-0072 — Concurrent Arrow Calculus](./0072-concurrent-arrow-calculus.md)
- [ADR-0073 — State Machine Invariants via Fixpoint Discovery](./0073-state-machine-invariants.md)
