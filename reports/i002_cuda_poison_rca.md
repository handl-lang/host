# I-002 RCA — nondeterministic CUDA context poison in delivery eval

Static/log investigation (no GPU repro). Full agent transcript condensed;
evidence citations preserved.

## Verdict

Observed v6 poison events are **consistent with the known
`_GraphReplayModule` UAF under a stale runtime** — the mlopt venv still
had buggy `kit-cuda 0.1.0` installed during the v6 window
(`audit_pass_20.md:85-90,206`), even though the pin had moved. **No
post-v0.1.2 poison reproducer exists**: v7 corpus (fixed kit-cuda
`2be4a79`/v0.1.3+) shows only empty-graph warnings, no device asserts.

## Observed events

- **Event A** — `arch:t5-small-cuda` `fuse+graph`: ScatterGatherKernel.cu:257
  index-OOB device assert → sticky context → all later configs recorded
  bogus `lower failed`. Only `fuse+graph` crashes; `fuse`, `graph`,
  `compile`, `compile+graph` each pass individually (safe-order drain).
  Matches the park-params-frees-captured-storage UAF signature exactly.
- **Event B** — v6 run A held-out: `illegal memory access` surfaced at an
  unrelated eager `x.to` inside `DeliveryBoard.__init__` — classic sticky
  error misattribution; faulting cell unidentifiable in the old
  monolithic `held_out` (now per-cell, `deliver_train.py:367-454`).
- Later "failures" are drain-order artifacts, now honestly recorded as
  `"skipped": true` (`deliver_game.py:588-684`, `_DRAIN_KEYS` fix
  `37f5902`).

## Coverage analysis of kit-cuda v0.1.2+ fix

Generic `_GraphReplayModule` path — substantially covered:
`storage_fingerprint` retention (`runners.py:232`), `_params_stable`
recapture (`:369-375`), eager-forever fallback (`:377-423`), alias binding
checks, per-device graph keys, `no_grad` capture hygiene.

### REAL GAP — carrier-executor path NOT covered

`CudaGraphRunner._capture_executor` (`kit_cuda/runners.py:545-570`)
delegates to `BatchedExecutorBase.capture_cuda_graph`
(`graphopt/packages/mlopt-torch/.../executors/base.py:180-220` — now
kit-torch): clones inputs, records `_forward_impl` under
`torch.cuda.graph`, stores `_graph`/`_graph_inputs`/`_graph_out`.
Docstring admits: "parameters are baked in by pointer — in-place
updates are replayed, replacing a parameter is not."

**No `storage_fingerprint`, `_params_stable`, recapture, or
eager-forever fallback.** Replay paths in `kit_carriers`
(`scan_lower.py:646`, `om_lower.py:621`, `omd_lower.py:681`,
`decayed_lower.py:428`) call `g.replay()` directly. Latent today
(`BatchedExecutorBase` has no `_park_params`, so the T5 trigger isn't
exercised) but a real UAF hole if params are ever rebound post-capture.

### Other gaps (lower confidence)

- Fingerprint covers only `_module_tensors` (params/buffers/_param_map/
  _param_values) — Const leaves, closure tensors, executor caches not
  covered; `storage.resize_` edge case.
- `compile+graph` may be compile-only: `CudaGraphRunner.apply` returns
  early when `stats["compiled"]` (`runners.py:696-697`) — log actual
  stats, not config names.
- Package identity drift hazard: `mlopt_torch` vs `kit_torch` aliases,
  unreliable `0.1.0` dist versions — resolved by the now-completed
  kit-torch migration + venv sync.

## Ranked hypotheses

1. **Stale kit-cuda 0.1.0 UAF** — most likely for all observed v6 events.
2. Carrier-executor graph path lacks storage safety — real gap, latent.
3. Generic-path residual for non-fingerprinted tensors — unproven.
4. `fuse+graph` = torch.compile region bodies under CUDA graph — unproven.
5. Eager ATen index OOB — unlikely (eager configs pass).
6. Async error misattribution — methodological certainty.
7. Concurrent context fault — no evidence.
8. Package identity drift — hazard, mostly resolved post-migration.

## Recommended actions (cheapest first)

1. Log runtime identity (torch/kit_cuda/kit_torch `__file__`+commit,
   `isinstance(module, IRModule)`) at every GPU experiment start.
2. One T5 config per fresh subprocess; never continue in-process after
   a poison signature.
3. `CUDA_LAUNCH_BLOCKING=1` + per-device sync around capture/verify/bench.
4. Ablate: fuse-only / graph-only / compile-off-graph-on matrix to
   isolate Inductor-kernel-under-graph vs lifetime.
5. `compute-sanitizer`/`cuda-gdb` only on a confirmed minimal repro.
6. **Per-cell subprocess isolation for census drains + held-out** —
   sticky errors make in-process continuation dishonest (mitigation).
7. **Fix carrier-executor path**: add storage fingerprint/recapture or
   refuse graph capture when params can rebind post-capture.
8. Keep `arch-t5-small-cuda` skipped until `fuse+graph` proves safe on
   confirmed-fixed kit-cuda in fresh subprocesses.

## Status update (post-RCA)

- mlopt venv synced 2026-11-19: kit-cuda v0.1.5 (`e361eb9`), kit-torch
  v0.1.4, kit-discovery v0.1.3, eqsat v0.1.12 — the stale-runtime
  hypothesis is now testable; v7 training running on the fixed stack.
- kit-torch migration complete → identity-drift hazard mostly closed.
