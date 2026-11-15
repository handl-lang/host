---
title: ADR-0174 exit-criteria audit — pass 2 (fix verification)
type: report
date: 2026-11-15
---

# ADR-0174 exit-criteria audit — pass 2 (fix verification)

All ten pass-1 findings verified FIXED. No invariant violations in new
surface (engine-eqsat guard/shape/cost, onnx guards, stage versioning).
Residuals N1-N4 all LOW/INFO. Verdict: fix loop can close after one
docs pass (N1).

## Per-finding verification (all FIXED)

- **F1** ADR status/docs honesty → FIXED (N1 caveat: two residual notes stale again)
- **F2** deliver-hooks bypass → FIXED: `delivered=rep.passed`,
  `delivered_unverified` on fail; tests pin it (graphopt dcea864, kernelopt bf20ed3)
- **F3** stage deliver artifact → FIXED: KitRunner smoke before write,
  `artifact_executed` honest, tamper re-gate (mlopt b377d0d)
- **F4** meta_arena census → FIXED: TestAmbientCensus pins
  supported ⊆ REGISTRY + rule-pattern census (discoverykit c60fde6)
- **F5** codec tables → FIXED: test_surface_table_drift.py guards
  _SPECS/modelproj/torch_bindings; test_onnx_codec_drift.py (978 lines)
  guards ONNX_BINDINGS/ONNX_LIFTS (graphopt c5948e4, 1984235)
- **F6** kit-mlopt docs+deliver → FIXED: protocol landed docs,
  pipeline.deliver exposed, StageFn rejects extra args (3e3278f)
- **F7** reshape_flatten → FIXED: renamed reshape_flatten_demo,
  UNSOUND doc, no pack, tests only (engine-eqsat f9567d4)
- **F8** compile-cuda dep → FIXED: git dep + dev-link patch (3fba1b3)
- **F9** optimize_model decline → FIXED: dedicated VerifyDeclinedError
  catch, verified=False + verify_max_abs/rel/accepted_by (mlopt b377d0d)
- **F10** measurement nits → FIXED: scorecard "(unverified)" annotation,
  whole-IR drift check, strict arity decline, "declined" label,
  task-contract documented (graphopt 0213111, mlopt 17b01d0, eqsat 3dd83c7)

## New surface audit

- engine-eqsat stages 2-3 (3f2f07a): CLEAN — Unknown/Invalid shapes
  poisonous-propagate, predicates decline on unproven input, flops
  +∞ on unbound/non-finite/malformed, GuardError::Unsupported named at
  compile time. Parity spot-checks match cond.py.
- graphopt 1984235 onnx drift guards: thorough, behavioral, honest
  about what's NOT claimed (IR-arity vs host-arity).
- mlopt 8e02cb0 stage versioning: RECORD_VERSION=1, tolerant v0 read,
  future-version named decline.
- hnl-run pipeline flow: deliver re-gates on real lower(verify=True);
  JSON verified claim never trusted.

## New findings (all LOW/INFO)

- **N1** ADR-0174 invariant-1 residual note + invariant-4 interim note
  stale again (F2/F5 fixed after my pass-1 edit) — docs pass needed.
- **N2** cost.rs conv `groups` UInt>u32::MAX clamps instead of declining
  (unreachable realistically; decline cleaner).
- **N3** Pred::Leaf treats non-sourced Op as leaf — matches to_kernel,
  no live divergence, documented.
- **N4** deliver smoke runs program text under ambient dispatch, not
  the shipped file byte-for-byte (documented scope; run-check not
  numeric re-verify).

## Updated scorecard

| criterion | pass1 | pass2 |
|---|---|---|
| prod ready | PARTIAL | PASS |
| documented | FAIL→PARTIAL | PASS− (N1) |
| working | PASS | PASS |
| verified | BIG IMPROVEMENT | PASS |
| tested | PASS | PASS |
| useful | unchanged | PASS |
| elegant | IMPROVING | PASS− (tables pending P3 retirement by design) |

**Exit assessment**: no invariant violations, no unguarded tables, no
undocumented seams beyond N1-N4. "Done" = audit outcome — this pass
finds the checklist clean modulo one docs edit.
