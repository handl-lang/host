---
title: Audit pass 14 — D-1c kit-cuda split + da1dc25 fixes + radon re-baseline + kit-carriers gates
type: report
date: 2026-11-16
---

# Audit pass 14

Scope: committed state since pass 13 — the D-1c `mlopt-cuda →
handl-lang/kit-cuda` split (kit-cuda `7a4dd44`+`331a136` v0.1.0,
graphopt `869c336` v0.1.16, mlopt `3e1ffbd`, kernelopt
`ef4d20b`+`4e01469`, eqsat `1b4ed13`, gamekit `ba5dd7c`,
discoverykit `18b32bb`, kit-onnx `1aa20df`, kit-carriers
`b4545cf`), the D-1d `mlopt-discovery → kit-discovery` split in
flight, mlopt `da1dc25` (n_enodes DAG count, trainer
`MLOPT_EQSAT_TIMEOUT`, onnx-rust memmap), graphopt `e2a4e21`
(radon build/ exclusion + re-baseline; `20ca7fc` dspec_audit
archaeology-path fix resolves P13-N2), kit-carriers `6265f96`
(coverage floor + dep-closure contract, resolving P13-N1), and the
delivery-prior v5 negative result.

**WIP present and not disturbed:** kit-discovery has a 139-file
uncommitted rename (`src/mlopt_discovery/* → src/kit_discovery/*`,
`RM` staged+modified) plus a `pyproject.toml` rewrite — the actual
D-1d split is *not committed* (see below).  mlopt still has the
untracked `arch-t5-small.{hnl,safetensors}` pair.  An untracked,
gitignored stale `graphopt/packages/mlopt-torch/build/lib/` tree
predates the rename (still spells `mlopt_cuda` in a docstring —
local artifact only, not committed state).

## Per-commit verdicts

### D-1c kit-cuda split — CLEAN mechanics; one gate-wiring gap

- **Remote tags verified live:** kit-cuda `v0.1.0` = `331a136`
  (annotated→`331a1368`); graphopt `v0.1.16` = `869c336`; eqsat
  `v0.1.9` = `414ec4e`; gamekit `v0.1.10` = `63d6eff`; discoverykit
  `v0.1.14` = `4781d0e`; kit-onnx `v0.1.1` = `2784ce8`;
  kit-carriers `v0.1.1` = `a25613f`.  mlopt is private (https
  ls-remote empty; ssh `origin/main` = `da1dc25` confirmed).
- **No stale `mlopt_cuda`/`mlopt-cuda` in committed code** across
  mlopt/kernelopt/eqsat/kit-cuda/kit-carriers/kit-onnx/gamekit/
  discoverykit/source-onnx (only historical docstrings and the
  kit-cuda pyproject "Split from … packages/mlopt-cuda" provenance
  line).  graphopt's one hit is the untracked `build/` artifact.
- **Pins resolve:** graphopt `uv.lock` resolves kit-cuda to
  `331a136…` (`uv.lock:1644-1650`) as a root dep (`:2257`);
  `uv lock --check` → Resolved 216.  mlopt `uv.lock:303-309`
  resolves the same sha; `uv lock --check` → Resolved 78.
  kernelopt pins graphopt `v0.1.16` for `mlopt-torch`/
  `mlopt-discovery` (`pyproject.toml:52,57`); `uv lock --check` →
  Resolved 51.  kit-cuda's own lock resolves `mlopt-torch` at
  `v0.1.15` via its dev-group direct dep — dev-only, one tag behind
  the `v0.1.16` consumers (P13-N3 pattern, INFO).
- **Import graph is honest:** `kit_cuda` imports only `torch`,
  `mlopt_core.failures`, `mlopt_torch.device`,
  `mlopt_torch.device_placement` (`src/kit_cuda/runners.py:85-91`)
  — inside its declared closure; it never imports
  `mlopt_orchestrator` (duck-typed `Runner` protocol).  graphopt's
  remaining `kit_cuda` references are lazy in-function imports
  (`mlopt_discovery/play/{gen,torch_arm}.py`) or docstrings
  (`mlopt_torch/runners.py:9-18` explicitly documents *why* it does
  not re-export).  `import kit_cuda` verified working in graphopt's
  venv.
- **Coverage floor is honest — measured, not trusted:** ran
  `coverage run -m pytest tests/ -q && coverage report -m` in
  kit-cuda → **47 passed, 99% total branch coverage**, the single
  miss `runners.py:320` — exactly what the pyproject comment claims
  (`pyproject.toml` `[tool.coverage.report] fail_under = 99`).
  The host has CUDA (torch 2.15.0+cu132, `cuda.is_available()`
  True), so the `requires_cuda` tests ran rather than skipped.
  Caveat: the floor is only reachable on a CUDA-capable host —
  on a CUDA-less box the gated tests skip at setup and coverage
  drops below 99; same environment-dependence class as every
  `requires_cuda` floor, but worth knowing the number is
  host-conditional.
- **Import-linter contract verified:** `.venv/bin/lint-imports` →
  "kit_cuda is the cuda top … KEPT" (forbids orchestrator, gamekit/
  benchkit/discoverykit, mlopt_discovery, mlopt_host_tensor,
  kit_carriers, kit_onnx — `pyproject.toml` contract block);
  `ruff check`/`ty check` clean.
- **Rename commits are pure name swaps:** gamekit `ba5dd7c`,
  discoverykit `18b32bb` (+`tests/test_smoke.py`), kit-onnx
  `1aa20df`, kit-carriers `b4545cf`, eqsat `1b4ed13`, kernelopt
  `ef4d20b` — each diff is `mlopt_cuda`→`kit_cuda` (or
  `mlopt-cuda`→`kit-cuda`) inside forbidden-module lists,
  docstrings, and comments only.

### D-1d kit-discovery — committed state is pre-split; WIP is large but consistent

- kit-discovery HEAD `1d6429f` is the subtree-split transplant:
  committed `pyproject.toml` still reads `name =
  "mlopt-discovery"`, `version = "0.1.0dev"`, deps without the dev
  group — i.e. **the D-1d rename is entirely uncommitted** (139
  changed paths: every `src/mlopt_discovery/*` →
  `src/kit_discovery/*` rename staged `RM` with further unstaged
  edits, `pyproject.toml` modified, `proposal.py` and
  `intake/__main__.py` deleted in the worktree).
- The repo has **no `origin` remote** — only a local-path `graphopt`
  remote; nothing is pushed.  graphopt still ships
  `packages/mlopt-discovery` as a workspace member and pins nothing
  to kit-discovery — the consumer cascade hasn't started, so no
  committed state is broken.
- Worktree (uncommitted, noted for the parent's awareness only):
  the new pyproject carries `fail_under = 0` in
  `[tool.coverage.report]` — a placeholder floor, and `ci.sh`
  already wires `lint-imports` + `coverage run/report`.  The
  dep-closure forbidden contract lists `mlopt_host_tensor`,
  `kit_onnx`, `benchkit`, `mlopt_native`, `mlopt` — matching the
  AGENTS.md "top of the stack" contract.

### mlopt `da1dc25` — CLEAN; all three fixes verified

- **`n_enodes` DAG count (`deliver_game.py:483-510`)** — verified
  correct under the term algebra's own contract:
  `SearchResult.term` (`mlopt-core/pipeline.py:194-237`) is "the
  member the search extracted under `cost_fn`"; every constructor
  path mints through `Op.make`, which interns via
  `Op._INTERN = WeakValueDictionary()` (`ir.py:189-265`) — "equal
  subterms are the same object" holds *while the DAG is alive*,
  which the walk guarantees (the root `term` reference keeps every
  reachable node alive, so no `id()` ABA reuse mid-walk).  The
  eqsat wire decode also rebuilds via `Op.make`
  (`mlopt_orchestrator/eqsat.py:619`) and `_pyterm_dag` preserves
  node-table sharing; `.hnl` imports mint `Op.make` too
  (`hnl_import.py:498,574,592,1025`).  Only `Op` is composite —
  `Var`/`Const`/`Param` are leaves (`ir.py:100-187`), so pushing
  only `isinstance(a, _IRop)` args is exact; attrs are scalars/
  tuples by convention (no term-valued attrs found).  The feature
  is real: `play.py:82` feeds `st:n_enodes` (capped 8192) into the
  learned-player featurizer — the old `getattr(res,"n_enodes",0)`
  was permanently 0.0.  Residual INFO: the `n < 1_000_000` cap
  saturates silently (undocumented in the comment — a ≥1M-op term
  reports exactly 1M), and two direct `Op(...)` constructions bypass
  `Op.make` (`mlopt_torch/region_fuse.py:716`,
  `kit_carriers/trace_lift.py:765`) — both are lowering-path mints
  outside `res.term`'s provenance, so the count stays exact for
  every term this feature can see; if a future path feeds a
  non-interned term the walk degrades to an honest overcount, never
  a crash.
- **`MLOPT_EQSAT_TIMEOUT` → trainer (`train_delivery_prior.py:159-186`)**
  — verified: `_eqsat_timeout_seconds()` (`optimize.py:19-31`)
  returns `None` unset / raises `ValueError` on garbage, and
  `partial(EqsatEngine, timeout=…)` is a zero-arg factory —
  `_resolve_engine` (`mlopt_orchestrator/optimize.py:404-405`)
  calls any non-`add_term` callable, and `EqsatEngine.__init__`
  accepts `timeout` (`mlopt_orchestrator/eqsat.py:781-803`).
  Reaches the trainer exactly as `optimize_model` does
  (`optimize.py:196-207`).  The `# noqa: PLC2701` private import is
  ugly but functional.
- **onnx-rust memmap (`onnx_rust.py:231-258`)** — verified no
  correctness regression: `np.memmap(mode="r")` slices stay
  file-backed; `np.frombuffer(payload, …)` keeps the memmap alive
  through `ndarray.base`, and `_param_tensor` copies via
  `torch.tensor(np.ascontiguousarray(arr))` — the only resident
  weight bytes, as the comment claims.  `arch_rust.py:456` reuses
  `_onnx._load_safetensors`, so the multi-GB bound-payload path the
  comment cites is covered too.  BF16 widening path unchanged.
  `pytest -k "onnx or safetensors or deliver"` → **82 passed**,
  including the `_load_safetensors` BF16 round-trip.  Trivial nit:
  a 0-byte file now dies inside `np.memmap` ("cannot mmap an empty
  file") instead of the custom "too small" `ValueError` — same
  exception type, different message.

### graphopt `e2a4e21` radon re-baseline — CLEAN; exclusion hides nothing committed

- `scan()` now skips path components `{__pycache__, build, dist,
  .venv, venv}` (`scripts/radon_ratchet.py:48-50`); `TARGETS` is
  `["packages"]` only.  `git ls-files` confirms **zero tracked
  `.py` files** under any of those directory names — the exclusion
  cannot hide committed code (the `packages/mlopt-torch/build/`
  tree that motivated it is gitignored, untracked).
- The baseline diff is the ratchet's own `--update` full-snapshot
  mechanism: removed entries are the modules that became 19-line
  `sys.modules` shims onto discoverykit/gamekit (antiunify,
  bilinear_game, … — real code moved repos with their own gates);
  added entries are pre-existing committed functions (`hnl_import`,
  `vocabdir`, `mlopt_host_tensor/bindings`, `region_fuse`,
  `device_placement`, `modelproj`, `model_corpus`, `handl_engine`,
  `hnl_{surface,codec}`) that had drifted the ratchet red since
  `bc51468`/`98fcf84`.  Max newly-pinned complexity is 53; 95
  functions >11 are now pinned — a one-time amnesty of
  pre-existing complexity, which is what `--update` is for, but it
  does enshrine the current ceiling (noted, not faulted).
- `.venv/bin/python scripts/radon_ratchet.py` → **"complexity
  ratchet ok (2831 functions)"** — green at HEAD.
- `20ca7fc` (same window) resolves P13-N2: `dspec_audit.py`'s
  pinned-rev paths now spell the pre-rename `catopt-*` names with
  an `ORACLE_MODULE` map to current modules.

### kit-carriers `6265f96` — CLEAN; resolves P13-N1

- `fail_under = 89` with the sibling-standard `exclude_also` set;
  `ci.sh` now runs `lint-imports` + `coverage run -m pytest` +
  `coverage report` (the exact gap P13-N1 named).  The 89% figure
  is a stated measurement ("om.py's residual misses are the
  compile/codegen paths exercised through graphopt's suite") —
  not re-measured here (834-test suite); the mechanism is the same
  measure-then-floor pattern kit-onnx used.
- **Dep-closure contract matches reality:** the forbidden list
  (`kit_onnx`, `kit_cuda` post-`b4545cf`, `mlopt_host_tensor`,
  `mlopt_discovery`, `discoverykit`, `gamekit`, `benchkit`,
  `mlopt_native`) vs. actual imports — top-level binds are only
  `mlopt_core`, `mlopt_torch`, `mlopt_orchestrator`, `torch`, and
  stdlib (`math`/`importlib`/`functools`/`dataclasses`/`collections`/
  `typing`); no `mlopt_dtypes` direct import (transitive only, and
  it isn't forbidden — correct).  `.venv/bin/lint-imports` →
  **"kit_carriers binds only its declared dep closure KEPT"**
  (26 files, 92 dependencies analyzed).

### Delivery prior v5 negative result — methodology honest; one internal inconsistency

No committed artifact found: `DELIVERY_PLAYER_WEIGHTS` is still
`{}` (`weights.py`), no results doc exists in mlopt/graphopt/host —
the v5 run is session context, so this is a methodology audit of
the machinery the claim rests on:

- The reward/gate chain is conservative in the right direction:
  `_bench` reports min+median over 50 iters with warmup and
  `cuda.synchronize` (`deliver.py:249-293`); `committed_speedup`
  pays only when `best_med * WIN_MARGIN < eager_med`
  (`deliver_game.py:187-197`); the trainer's oracle and REINFORCE
  reward share that gate (`deliver_train.py:19-29`).  Sub-margin
  configs score exactly 1.0 — exploration cannot beat the eager
  floor, so a *negative* result (no config clears the margin
  consistently → oracle labels ≈ "stop" → nothing learnable →
  weights stay `{}`) is the honest outcome the machinery is built
  to produce, not a suppressed one.
- **Inconsistency worth noting (INFO):** `WIN_MARGIN = 1.02`
  (`deliver_game.py:80`) is a 2% gate while the claimed noise floor
  is ~3% — the margin sits *below* the stated noise floor, so a
  config winning by 2–3% would be counted a win though it's within
  noise.  For the negative conclusion this bias runs the wrong way
  to matter (false-positive wins would *create* apparent signal;
  finding none despite a too-tight gate strengthens the null), and
  single-shot medians without interleaved re-benching are the real
  weakness — but if a v6 retrains, the margin should be ≥ the
  measured noise floor or the bench should interleave eager/config
  reps.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P14-N1 | MEDIUM | `kit-cuda/ci.sh:6-10` vs `kit-cuda/pyproject.toml` `[tool.coverage.report]`/`[tool.importlinter]` | **The 99% coverage floor and the import-linter contract are declared but not gated:** `ci.sh` runs ruff/format/ty/pytest only — no `lint-imports`, no `coverage run/report`. Every sibling split repo wires both into `ci.sh` (kit-onnx, kit-carriers `6265f96`, kit-discovery worktree). AGENTS.md's verification block lists them, so docs are right and the gate script lags — same fix as kit-carriers `6265f96`. |
| P14-N2 | LOW | eqsat `v0.1.9`=`414ec4e` (pinned everywhere) vs untagged `1b4ed13`; graphopt `pyproject.toml:203-211` | **Backend-neutrality guard still spells `mlopt_cuda`:** the rename commit is on eqsat main but untagged, so every consumer resolves the v0.1.9 forbidden list (`torch, mlopt_torch, mlopt_cuda, kit_onnx`) — `mlopt_cuda` no longer exists (dead entry) and `kit_cuda` is unguarded until the next eqsat tag + cascade. Compounding: graphopt's `layers` contract dropped `mlopt_cuda` entirely rather than replacing it — `kit_cuda` is an external git dep outside `root_packages`, so nothing in graphopt now statically forbids `mlopt_torch → kit_cuda` (previously the layers ordering did). A lazy `import kit_cuda` inside `mlopt_torch` would evade both repos' import-linter. |
| P14-N3 | INFO | `kit-cuda/pyproject.toml` dev group | Dev env pins `mlopt-torch@v0.1.15`; consumers are on `v0.1.16`. Dev-only lag, same class as P13-N3 — bump with the next consumer cascade. |
| P14-N4 | INFO | `kit-discovery` (whole repo) | D-1d committed state is the pre-rename transplant: `name = "mlopt-discovery"`, `0.1.0dev`, no `origin` remote, and the 139-file rename + pyproject rewrite (incl. a `fail_under = 0` placeholder floor) is uncommitted WIP. Nothing downstream references it yet — clean, but the split isn't landed. |
| P14-N5 | INFO | `mlopt/deliver_game.py:497-500` | `n_enodes` walk caps silently at 1M (`while stack and n < 1_000_000`) — undocumented saturation; and the correctness argument rests on `Op.make` interning while two direct `Op(...)` sites exist (`region_fuse.py:716`, `trace_lift.py:765`) — both outside `res.term` provenance today, but a future non-interned term source degrades to overcount. |
| P14-N6 | INFO | `mlopt/deliver_game.py:80` | `WIN_MARGIN = 1.02` sits below the ~3% noise floor the v5 negative result cites — see methodology note above; conservative direction for the negative claim, worth aligning before the next training run. |
| P14-N7 | INFO | `graphopt/packages/mlopt-torch/build/` (untracked), `mlopt` 0-byte safetensors edge | Housekeeping: a stale gitignored `build/lib` tree still spells `mlopt_cuda` (regenerate or delete); `_load_safetensors` on a 0-byte file now raises numpy's "cannot mmap an empty file" instead of the custom message (same `ValueError` type). |

No HIGH findings.  P13-N1 is resolved (kit-carriers floor +
contract, verified `lint-imports` KEPT), P13-N2 resolved
(`20ca7fc`), and P13-N3's pattern repeats once in kit-cuda's dev
pin.  The D-1c cascade is installable end-to-end (`uv lock --check`
green in graphopt/mlopt/kernelopt/kit-cuda) with the coverage floor
measured honest (99%, single miss `runners.py:320`, CUDA host) —
its only gap is that `ci.sh` doesn't invoke the gates yet
(P14-N1).  `da1dc25`'s three fixes all check out: the n_enodes
count is exact under `Op.make` hash-consing (the same assumption
the eqsat wire encoder already makes), the timeout partial is a
valid zero-arg engine factory, and the memmap keeps file-backed
pages with copies only at the torch boundary.  The radon
re-baseline is the ratchet's own mechanism and the exclusion can't
touch tracked files.  The delivery-prior negative result is
methodologically credible — the margin gate biases toward false
positives, so finding no signal is a stronger null than the
machinery could easily produce.

**Verdict: PASS with one MEDIUM** — wire `lint-imports` +
`coverage run/report` into `kit-cuda/ci.sh` (the kit-carriers
`6265f96` pattern) and the split is fully gated; everything else
is LOW/INFO bookkeeping (untagged eqsat rename, dev-pin lag,
D-1d still uncommitted, documented-only nits).
