# ADR-0072: Concurrent Arrow Calculus

## Status

Proposed

Builds on [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md).

## Context

The arrow calculus is deterministic dataflow: given an input, the graph
produces a unique output. The inference mechanism reads the proof off
the graph structure, which requires that the structure fully determines
the behavior.

Concurrency breaks this assumption:
- **Non-deterministic scheduling**: the order of operations is not
  determined by the graph
- **Race conditions**: multiple operations on shared state, order-dependent
- **Lock ordering**: deadlock freedom depends on acquisition order
- **Channel communication**: message ordering and buffering

The current 5 combinators (Seq, First, Choice, Loop, App) cannot express
these. There is no `Parallel`, no `Race`, no `Channel`, no `Lock`.

## Decision

Do NOT extend the arrow calculus with concurrency primitives at this
time. The inference mechanism is fundamentally structural — it reads
the proof off the wiring. Concurrency introduces non-structural factors
(scheduling, timing) that the inference cannot see.

### What might work in the future

**Message-passing concurrency (CSP/actor style)**:

Model concurrency as deterministic dataflow over channels:
```
actor = loop(comp(receive, choice(handle_a, handle_b), send))
```

If `receive` and `send` have contracts about channel state, the fixpoint
could discover invariants like "the channel is never empty when we try
to receive." This requires:
- Modeling channel state as the feedback channel (the loop's C)
- Contracts on `receive` (requires: channel non-empty) and `send`
  (guarantee: channel non-empty after)
- The fixpoint discovers channel invariants

This is the most promising direction because it stays within the
deterministic dataflow model — the non-determinism is in the message
arrival, not in the graph structure.

**Limitation**: this models the channel state invariant, not the
full concurrent behavior. It can prove "receive never blocks" but not
"two actors never deadlock" — the latter requires reasoning about the
global message graph, not just one actor's local state.

### What doesn't work

**Shared-memory concurrency**: multiple arrows writing to the same
state cell, with non-deterministic interleaving. The inference engine
cannot reason about interleavings — it sees one graph, not all possible
execution orders.

**Lock-based concurrency**: deadlock freedom requires reasoning about
lock acquisition order across multiple threads. This is a global
property, not a local graph property.

### Why not now

1. The deterministic dataflow boundary (ADR-0070) already covers most
   business logic, data transformation, and algorithms.
2. Concurrency verification is an open research problem even in
   traditional systems (TLA+, Iris, separation logic).
3. The inference engine would need fundamental extensions:
   - Reasoning over execution traces, not just graph structure
   - Non-deterministic fixpoint computation
   - Global invariant discovery (not just per-loop)
4. The payoff is unclear: even if we could express concurrent arrows,
   the inference might be too weak to prove useful properties.

## Consequences

### Positive

- Honest boundary: we don't claim to handle concurrency
- Focuses effort on deterministic dataflow (the proven domain)
- Message-passing model is identified as the future direction

### Negative

- Concurrent programs cannot be verified with the current system
- Users working in concurrent domains need different tools
- The message-passing direction is identified but not pursued

## Status

This is a boundary declaration, not an implementation plan. The
inference engine targets deterministic dataflow (ADR-0070). Concurrency
is out of scope for the current architecture. The message-passing
direction is noted as future research.

## Related ADRs

- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
- [ADR-0071 — Failure-Tracking Arrows](./0071-failure-tracking-arrows.md)
