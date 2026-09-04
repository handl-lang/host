# ADR-0073: State Machine Invariants via Fixpoint Discovery

## Status

Accepted

Builds on [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
and the refinement inference engine in `crates/plugin-ext-refinement/`.

## Context

A state machine is `State × Event ⇝ State` — it processes events and
updates state. The key verification question: **what invariants hold
on the state across all transitions?**

Examples:
- A counter: `Count ≥ 0` (decrement fails on zero)
- A bounded buffer: `0 ≤ |buffer| ≤ max_size`
- A protocol: `state ∈ {idle, active, done}` (valid states only)
- A resource pool: `allocated ≤ total`

State machines map naturally to arrow graphs:
```
state_machine = loop(comp(handle_event, first(id)))
```

where `handle_event : (State × Event) ⇝ (State × Event)` updates the
state based on the event, and `first(id)` passes the event stream
through as a side channel.

## Decision

State machine invariants are discovered by the **fixpoint iteration**
in the refinement inference engine. No user annotations are required.

### How it works

1. Each transition atom has a **conditional contract**:
   - `state.dec : Int → Int`
     - `requires: Le(0, input)` — can only decrement if ≥ 0
     - `guarantee: Le(0, output)` — result is ≥ 0

2. The state machine is a loop:
   ```
   counter = loop(comp(choice(state.inc, state.dec), first(id)))
   ```

3. The inference engine runs the fixpoint:
   - I₀ = body's guarantee (includes `Le(0, output)` from `state.dec`)
   - wp(body, I₀) = the precondition needed to preserve I₀
   - For `state.dec`: wp = `Le(0, input)` (the precondition)
   - I₁ = I₀ ∧ wp(body, I₀) = `Le(0, output) ∧ Le(0, input)`
   - Fixpoint: the invariant `Le(0, count)` (Count ≥ 0) is discovered

4. The guarantee includes the invariant: `Le(0, output)`

### What works

- **Data invariants**: properties of the state value itself (Count ≥ 0,
  buffer non-empty, valid state enum)
- **Conditional transitions**: atoms with preconditions (can only
  decrement if ≥ 0, can only pop if non-empty)
- **Termination**: if the event stream is finite (consumed via the side
  channel), the loop terminates

### What doesn't work

- **Concurrency invariants**: "the buffer is never accessed by two
  threads simultaneously" — this is a concurrency property, not a data
  property. See ADR-0072.
- **Liveness properties**: "the counter eventually reaches zero" — this
  is a temporal property, not an invariant. The fixpoint discovers
  invariants (safety), not liveness.
- **Global invariants**: properties spanning multiple state machines —
  the inference is per-graph, not cross-graph.

## Consequences

### Positive

- State machine invariants discovered automatically from transition
  contracts
- No user annotations — just write the state machine as an arrow graph
- Works for counters, buffers, protocol states, resource pools
- Conditional contracts (preconditions) enable invariant discovery via
  WP propagation

### Negative

- Only data invariants (safety), not liveness or concurrency
- The invariant is only as strong as the atom contracts — if
  `state.dec` doesn't have `requires: Le(0, input)`, the fixpoint
  won't discover `Count ≥ 0`
- Complex state machines (many transitions, deep nesting) may produce
  large fixpoint iterations

## Implementation

State machine atoms and arrow files are in:
- `contracts/state.hnl` — atoms: state.inc, state.dec, state.push,
  state.pop, state.is_empty, state.idle_to_active, state.active_to_done
- `handl-packages/state/` — arrow files: counter.hnl,
  buffer.hnl, protocol.hnl
- `src/state_tests.rs` — tests verifying invariant discovery

## Related ADRs

- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0072 — Concurrent Arrow Calculus](./0072-concurrent-arrow-calculus.md)
