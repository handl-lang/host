---
title: Concurrent Arrow Calculus
type: adr
date: 2026-09-01
status: accepted
---

# ADR-0072: Concurrent Arrow Calculus

## Status

Accepted

Builds on [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md).

## Context

The arrow calculus is deterministic dataflow: given an input, the graph
produces a unique output. The inference mechanism reads the proof off
the graph structure, which requires that the structure fully determines
the behavior.

Concurrency appears to break this assumption:

- **Non-deterministic scheduling**: the order of operations is not
  determined by the graph
- **Race conditions**: multiple operations on shared state, order-dependent
- **Lock ordering**: deadlock freedom depends on acquisition order
- **Channel communication**: message ordering and buffering

The question is whether the current 5 combinators (Seq, First, Choice,
Loop, App) can express concurrent systems, or whether new primitives
(`Parallel`, `Race`, `Channel`, `Lock`) are required.

## Decision

Concurrency is expressible within the existing calculus. The key
insight: **externalize the non-determinism**. The scheduler is modeled
as part of the input (an event stream), not as part of the graph. The
graph is deterministic; the non-determinism is in which events arrive.

### Core model

A concurrent system is a loop over an event stream:

```
concurrent_system = loop(comp(handle_event, first(id)))
```

Events arrive via the input stream. The shared state is the feedback
channel `C`. `handle_event` uses `left` (Choice) to branch over event
types. The graph itself is deterministic dataflow — given a specific
event, it produces a specific state transition.

The "all possible interleavings" question becomes: **what invariant
holds for ALL possible event streams?** This is exactly what the
fixpoint computes. Because `handle_event` branches over all event types
via Choice, the fixpoint discovers invariants that hold regardless of
which branch is taken — i.e., regardless of event ordering.

This is the same mechanism as state machines (ADR-0073): the invariant
is discovered from the loop body's guarantee via fixpoint iteration.
The only difference is that the "state machine" here has event-driven
transitions, and the events are externally supplied.

### Shared-memory concurrency

Model shared state as the feedback channel `C`. Multiple "threads" are
different event handlers in a `left` (Choice) — each event type
corresponds to a thread's operation on the shared state. The
interleaving is in the event stream (external non-determinism), not in
the graph.

The invariant on `C` is what the fixpoint discovers. "Race condition
safety" means the invariant holds for all event orderings — if the
fixpoint finds an invariant that is preserved by every event handler,
then no interleaving can violate it.

### Message-passing concurrency (CSP/actor)

Channel state is the feedback channel `C`. `receive` and `send` are
atoms with contracts:

- `receive` requires: channel non-empty (precondition)
- `send` guarantees: channel non-empty after (postcondition)

The fixpoint discovers channel invariants (e.g., "the channel is never
empty when we try to receive"). This stays within the deterministic
dataflow model — the non-determinism is in message arrival, not in the
graph structure.

### Deadlock freedom

If the loop body's atoms have preconditions (e.g., `receive` requires
a non-empty channel), the fixpoint discovers the invariant that ensures
the precondition is always met. "Deadlock freedom" means the discovered
invariant implies all preconditions are satisfied for every event in
the stream.

### Inference implications

No new inference rules are required. The fixpoint computation is
unchanged from ADR-0073: it iterates over the loop body's guarantee
until it reaches a fixed point. The only difference is that the loop
body contains a Choice over event types, so the fixpoint must find an
invariant that is preserved by every branch — which is exactly what it
already does for any Choice.

WP branching through Choice handles the disjunction: each event handler
branch contributes its guarantee, and the fixpoint finds the invariant
that all branches preserve.

## Consequences

### Positive

- No new calculus primitives — concurrency uses only Seq, First,
  Choice/left, Loop, App
- No new inference rules — the fixpoint mechanism from ADR-0073
  applies directly
- Shared-state invariants are discoverable (race condition safety)
- Channel invariants are discoverable (message-passing safety)
- Deadlock freedom is provable via precondition preservation
- The graph remains deterministic dataflow, fully within ADR-0070's
  scope — the non-determinism is external (in the event stream), not
  internal (in the graph)

### Negative

- Liveness properties (temporal, not invariant) are not provable — the
  fixpoint discovers invariants, not temporal properties like
  "eventually a response is sent"
- Global properties spanning multiple independent loops are not
  provable — the fixpoint is per-loop, so invariants that require
  reasoning about two or more concurrent loops together are out of
  reach
- The model requires the user to externalize the scheduler as an event
  stream, which is a different programming style than direct
  concurrency primitives

## Status

Accepted. Concurrency is in scope, modeled as dataflow over event
streams. The scheduler is part of the input, not part of the graph.
The fixpoint discovers invariants that hold for all event orderings,
using the same mechanism as state machines (ADR-0073). Liveness
properties and global multi-loop properties remain out of scope.

## Related ADRs

- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
- [ADR-0073 — State Machines as Loops](./0073-state-machines-as-loops.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0069 — Modules](./0069-modules.md)
- [ADR-0071 — Failure-Tracking Arrows](./0071-failure-tracking-arrows.md)
