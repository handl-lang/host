---
title: Step E tail — kit-torch Term-JSON codec migration dry-run
type: report
date: 2026-11-17
status: complete — branch pushed, no main merge (cascade coordination)
scope: codec-site census, mlopt_core.ir → mlopt_core.term repoints, engine-wire profile check
---

# Step E tail — kit-torch codec migration dry-run

Worktree `/home/luis/tmp/kit-torch-stepe`, branch
`stepe/kit-torch-term-codec`, commit `9cabf1af` on top of `fb44685a`
(origin/main, v0.1.3). **Pushed branch only — no main merge**, per
cascade-coordination instruction. Real checkout
`/home/luis/tmp/kit-torch-work` untouched.

Deps unchanged: `pyproject.toml` already pins eqsat `v0.1.12`
(`d67d1fc8`, carries `mlopt_core.term` + `term_json`); `uv sync
--group dev` resolved cleanly, no lockfile changes.

## Verdict: GO — pure view-class repoint, zero codec sites

kit_torch is the cleanest consumer in the tail census: it is a
**view-object producer/consumer only**. There is no Term-JSON codec
anywhere in `src/` — no `term_to_json`/`term_from_json`/`term_json`/
`hnl_codec`/`dag.program`/`__mlopt_leaf__`/`term_to_data` sites. The
only `json.dumps`/`json.loads` in the package is `export.py`'s
safetensors manifest header (lines 506/519/632/758/791) — an artifact
container format, unrelated to term wire. `calibrate.py`'s one
`subprocess` call is `git rev-parse HEAD` for provenance.

## Census detail

| category | sites | disposition |
|---|---|---|
| Term-JSON codec (→ `term_json`) | **none** | n/a |
| Lemma codec `term_to_data`/`term_from_data` | none in src; `tests/test_torch_bridge_edges.py:50-51,1189-1191,2021` | stays `mlopt_core.ir` — tests ride the shim per plan |
| View-class walking (`mlopt_core.ir` term imports) | **20 import sites, 10 files** | all repointed to `mlopt_core.term` |
| Engine-wire emit (const-str/`Arr`/`Foreign` for eqsat-run or source-onnx) | **none** | no profile decision needed — kit_torch never serializes terms to JSON; `TorchSource`/`TorchSink`/`IRModule` pass view objects in-process |

## Migrated sites (commit `9cabf1af`, 10 files, 27 lines)

Class-a construction (torch.export → IR):

- `src/kit_torch/torch_export.py:23` — `IR, Const, Op, Param, TensorType, Var`

Class-b consumers:

- `src/kit_torch/torch_bridge.py:48` — same six names (import block
  re-sorted by ruff after the repoint)
- `src/kit_torch/torch_eval.py:19` + lazy in-function imports at
  695-696 (`Op as _Op`, `Param as _Param`, ×2 pairs), 733/784
  (`TensorType`)
- `src/kit_torch/torch_bindings.py:29` (`Op`), 2445/2461 (lazy
  `_OP_REGISTRY`); docstring at 2417 updated to
  `mlopt_core.term._OP_REGISTRY` (`_OP_REGISTRY` lives in `term.py:516`,
  re-exported by `ir.py:22` — identity verified)
- `src/kit_torch/adapters.py:25` (`IR`)
- `src/kit_torch/calibrate.py:319,366` (lazy `IR, Op, TensorType, Var`)
- `src/kit_torch/executors/base.py:35` (`Op`)
- `src/kit_torch/folds.py:23` (`Op`)
- `src/kit_torch/meta_eval.py:18` (`Const, Op, Param, Var`)
- `src/kit_torch/region_fuse.py:66` (`Const, Op`)

Identity check: `ir.{IR,Const,Op,Param,TensorType,Var,_OP_REGISTRY}`
`is`-identical to `term.*` under eqsat v0.1.12 — the repoint is
API-identical, matching the plan's class-b assessment.

## Wire-profile decisions

None required. Per `stepe_tail_plan.md` §3 matrix row
("kit_torch.torch_export/torch_bridge/torch_eval — view-object
producer/consumer — repoint imports only"), kit_torch sits entirely
on the Python-object side of every wire:

- **eqsat-run wire**: kit_torch does not invoke `eqsat-run` and emits
  no term JSON. The eqsat bridge re-encodes view terms via
  `mlopt_orchestrator.eqsat`'s own `term_json` calls (leaf profile is
  that module's concern — the `const-str→arr` flip at plan Step 5).
- **source-onnx wire**: kit_torch *consumes* source-onnx records only
  indirectly through `mlopt.adapters.onnx_rust`/`arch_rust` decode
  (`term_from_json` → view `IR`), then works on objects. It never
  re-emits Term-JSON, so it inherits whatever leaf profile the
  adapters decode — `Arr` from source-onnx, decode-tolerant.
- **stage records**: not a producer; `kit-prelude`/`mlopt` own the
  envelope.

Consequence for other lanes: kit-torch imposes **no constraint** on
the `const-str→arr` flip or on source-onnx's `Foreign`/nested-dim
alignment. Its only codec-adjacent obligation was binding
`mlopt_core.term`, now done.

## Verification (all in the worktree venv, eqsat d67d1fc8)

- `uv sync --group dev` — clean, no lockfile diff.
- `ruff check src` — clean (5 import-sort fixes auto-applied by
  `--fix`, then `ruff format` — 32 files unchanged).
- `ruff format --check src` — clean.
- `ty check src` — **all checks passed**.
- `lint-imports` — 1 contract kept, 0 broken (59 files, 199 deps).
- Focused pytest (`-m "not slow"`):
  `test_adapters.py test_calibrate.py test_meta.py test_export.py` —
  **84 passed, 1 skipped** (98 s);
  `test_torch_bridge_edges.py test_torch_bridge_audit_fixes.py
  test_fusion_extract.py test_morphisms.py test_calibrate_edges.py` —
  **183 passed, 2 skipped** (16 s).
- Full core lane `pytest tests/ -m "not slow and not e2e" -x` —
  **2966 passed, 107 skipped, 75 deselected** in 216 s (the
  deselected are `slow`/`e2e`-marked; no codec-adjacent failures).

## Go / no-go

**GO for merge** once the cascade agent clears the lane: the diff is
27 lines of import repoints, all gates green, pushed as
`stepe/kit-torch-term-codec` @ `9cabf1af`. No dep bumps, no wire
changes, no test changes needed. The `ir.py` shim remains the compat
surface for the 57 test-file `mlopt_core.ir` imports (incl. the
lemma-codec `term_to_data`/`term_from_data` round-trip tests, which
correctly stay on `ir`).

## Files changed

- `src/kit_torch/{adapters,calibrate,executors/base,folds,meta_eval,region_fuse,torch_bindings,torch_bridge,torch_eval,torch_export}.py` — `mlopt_core.ir` → `mlopt_core.term` (20 sites) + one docstring path fix; ruff import re-sort in 3 files.
