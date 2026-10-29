---
title: The IR gains pipeline(depth), and the schedule programme stops there
type: adr
date: 2026-10-29
status: implemented
applies_to: '`lib-kernel-ir` (op 10), `compile-triton` (num_stages), `graphopt` (P24)'
---

# ADR-0166: The IR gains `pipeline(depth)`, and the schedule programme stops there

## Decision

1. `pipeline(depth)` is the **tenth op**. It denotes the **identity** and
   lowers to Triton's `num_stages`. A schedule changes when bytes arrive,
   never which bytes — so all `split_k` variants remain provably equal.
2. It is priced at **zero** (no extra traffic, no launch). The cost model
   is a lower bound and must not pretend to know whether overlap happened.
3. `stage_shared` is **not** added. `num_stages` already stages loads
   through shared memory, and a second annotation with no separate effect
   is decoration.
4. **The schedule-vocabulary programme is closed.** Measured: depth
   1 → 4 moves `down_proj` 52.5 → 52.1 µs and `gate_proj` 52.4 → 52.1 µs
   — 0.4–0.8 %, inside run-to-run noise. No further schedule ops.
5. The 47 % → 86 % gap to Marlin is therefore **not load latency**. It is
   the unpack/`dot` arithmetic, and it is not reachable by annotating this
   kernel shape.

## Why

ADR-0165's review said a data-only IR "can only express naive schedules,
which will lose to the hand kernel every time". That was a prediction about
latency hiding, and it was cheap to test: the most likely lever was
`num_stages` on the k-loop of a memory-bound GEMV. It does nothing. Two
readings were possible — the vocabulary is inadequate, or the schedule was
never the bottleneck — and the measurement picks the second. Recording it
stops the next session from re-running the same sweep, and stops the pitch
from promising a schedule compiler the hardware does not reward here.

## Not doing

- **More schedule ops** (`stage_shared`, pipelining combinators) — each
  would need its own measured effect, and the first one is zero.
- **A `pipeline` that changes meaning** — it would break the property that
  all `split_k` terms denote one function, which is the IR's only proof.
- **Deleting `pipeline`** — it is the instrument that produced the negative
  result; the finding is not reproducible without it.
- **Claiming the IR is a compiler** — it is a config-search substrate with a
  faithful lowering, and `pitch` wording should say so.

## Enforcement

- `lib-kernel-ir/src/lib_kernel_ir/ops.py` — `pipeline` present; `OPS` is 10.
- `lib-kernel-ir/tests/test_ops.py` — denotes the identity, cost unchanged,
  depth < 1 refused.
- `compile-triton/src/{term,codegen}.rs` — depth read from the term,
  emitted as `num_stages=PIPELINE_DEPTH`; cargo tests assert both.
- `graphopt/docs/reports/p24_m2_schedule.json` — the 48-cell sweep.
