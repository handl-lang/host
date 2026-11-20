---
title: Step E tail — codec migration dry-runs for kit-host-tensor + kit-carriers
type: report
date: 2026-11-17
status: dry-run complete — zero codec sites found in either repo; no source changes
scope: Term-JSON codec census of the two leaf-kit repos against eqsat v0.1.12
---

# Step E tail — kit-host-tensor + kit-carriers codec dry-runs

Follows `stepe_tail_plan.md` §3 (both repos are tabled as
"view consumers … optional repoint / ride shim") and
`stepe_graphopt_bench.md`. Both repos already pin eqsat `v0.1.12`
(`d67d1fc8`) in `pyproject.toml`; both worked directly on `main`
(clean trees, no worktrees needed). **No code was changed — the
honest result is "rides shim."**

## kit-host-tensor (`1f5912a`, main)

**Term-JSON sites: none.** Full grep of `src/` + `tests/` +
`scripts/` for `term_to_json` / `term_from_json` / `term_json` /
`hnl_codec` / `dag.program` / `term_to_data` / `term_from_data` /
`__mlopt_attrs__` / `const-str` / `Arr`: zero hits.

What the repo actually does with terms:

- `src/kit_host_tensor/sink.py:30,148,175` — view-class imports only
  (`IR`, lazy `Param`/`Op`). `lower()` walks the **object** term
  (`_param_leaves`, `_check_supported` — pure `isinstance` walking,
  lines 143-193). No serialization.
- `src/kit_host_tensor/executor.py:24` — `Const, Op, Param, Var` view
  classes; `Const.value` is an in-memory Python value, never a JSON
  leaf payload.
- `src/kit_host_tensor/bindings.py:57` — `Const` import for arg-list
  builders only.
- `src/kit_host_tensor/wire.py` — the `{"__tensor__": …}` /
  `{"__tensor_bin__": "<base64 LE>"}` f64 codec (ADR-0179). This is
  the **runtime op-call payload** codec: `_binding.call` JSON-encodes
  *argument arrays* for `handl_host_tensor.call("tensor.<op>", …)`
  and decodes results (`executor.py:132-146`). It never touches a
  `Term` — leaf *values* cross as `__tensor__` records, term *nodes*
  never cross at all.

**ADR-0179 interaction check (task item 3):** the `__tensor__` /
`__tensor_bin__` records are a *different* wire from Term-JSON leaf
records (`Arr`/`const-str`). Term-JSON leaf payloads describe
`Const`/`Var`/`Param` *terms* inside a serialized program;
`wire.py` describes *array values* inside a single op call. There is
no path by which a `term_json`-encoded leaf could reach
`decode_result`, and none by which a `__tensor__` record could be
misread as a leaf. The two codecs are orthogonal — no migration, no
interaction, nothing to gate on.

Remaining `mlopt_core.ir` imports are class-b view usage covered by
the eqsat `ir.py` re-export shim (`ir.Op is term.Op` verified
`True` under the resynced venv). Bulk repoint to `mlopt_core.term`
is plan Step 3 / E-6 hygiene, not codec work — deferred per plan.

## kit-carriers (`34c1339`, main)

**Term-JSON sites: none.** Same grep over `src/` + `tests/` +
`scripts/`: zero hits for every codec name. The 10 class-b src files
(`om.py`, `om_lower.py`, `om_fused.py`, `xcarrier.py`, `omd_lower.py`,
`scan_lower.py`, `scan_fused.py`, `decayed_lower.py`, `trace.py`,
`trace_lift.py`, `decode_laws.py`, `decode_geom.py`) import view
classes + `op_def` only — the ops-registry machinery, not a codec.

Serialization surface check:

- `tests/test_rulecache.py` exercises `mlopt_core.rulecache.RuleCache`
  — a **substrate** (eqsat) module, not a kit-carriers codec site.
  It serializes synthesized law *data* (pure data, "no callable is
  ever serialized" per the test docstring), not Term-JSON.
- No `json`/`pickle`/`dumps`/`loads` use anywhere in `src/`.
- `machinery.py`/`__init__.py` build `RuleSet`s of `Rewrite`s —
  objects, never wire records.

Same conclusion: rides the `ir.py` shim; `mlopt_core.ir →
mlopt_core.term` repoint is optional E-6 hygiene.

## Environment note

Both venvs were stale (mlopt-core installed from eqsat `v0.1.10` /
`4e2dc877` despite the `v0.1.12` pin). `uv sync --group dev`
re-resolved to `d67d1fc8` in both repos — no lockfile changes
committed (uv reported only package swaps, `git status` clean after
sync; verified below).

## Verification

| repo | pytest | ruff check | ruff format | ty | lint-imports |
|---|---|---|---|---|---|
| kit-host-tensor | **64 passed** (0.55 s) | clean | 9 files clean | clean | 1 kept, 0 broken |
| kit-carriers | **834 passed** (101.7 s) | clean | 15 files clean | clean | 1 kept, 0 broken |

Post-sync `git status` clean in both repos — no commits were
necessary (nothing changed). Nothing pushed.

## Go / no-go

- **kit-host-tensor: GO (no-op).** No Term-JSON codec exists; the
  sink/executor consume view terms in-process and `wire.py` is the
  orthogonal ADR-0179 runtime-value codec. Nothing to migrate; the
  `ir.py` shim covers the 3 class-b files until E-6.
- **kit-carriers: GO (no-op).** All 10+ class-b files ride the shim;
  the only serialization-adjacent surface (`RuleCache`) lives in
  `mlopt_core` and is out of scope for the consumer migration.
- **Ordering note for the parent:** neither repo gates on the
  kit-discovery `hnl_codec` shim landing (they never import it) or
  on the eqsat `const-str → arr` engine flip (they never emit
  Term-JSON). They can be repinned/verified independently of Steps
  1–5.
