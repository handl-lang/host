# P3 Step D-1e — `mlopt-torch` → `kit-torch` split: DONE

Date: 2026-11-16.  Scope: Phase 1 (kit-torch gates + tag) and Phase 2
(consumer migration, graphopt package removal, locks, tags, report).

## End state

- `handl-lang/kit-torch` is the standalone PyTorch backend
  (`src/kit_torch/`, dist `kit-torch`, import `kit_torch`), split from
  graphopt `packages/mlopt-torch` via `git subtree split` (history
  preserved).
- graphopt no longer ships `packages/mlopt-torch`; its ~23
  cross-stack tests import `kit_torch` as a pinned git dep.
- Every consumer pins the **converged** tag set (see below), not the
  originally-requested `v0.1.0`: the ecosystem performed a second
  cascade ("term repr B", eqsat `v0.1.12`) while this work was in
  flight, and `v0.1.0`'s substrate pins (`eqsat v0.1.9`) could not
  resolve against the rest of the stack.  `kit-torch v0.1.4`
  supersedes `v0.1.0`/`v0.1.1` and is what all consumers lock.

## kit-torch repo (`/home/luis/tmp/kit-torch-work`)

- Commits on main: `653586a0` (split) → `e579e714` (native `inv`
  binding + carrier-optional guards) → `7f0f1f93` (ruff format) →
  `95333d2c` (autotune carrier guard) → `f0c1bb3a` (coverage floor
  76%) → `fa646941` (eqsat v0.1.10 repin) → later `9cabf1af`
  (term-repr-B codec repoint, tagged `v0.1.4`) → `3bb7039e`
  (`v0.1.5`, CUDA-graph storage-fingerprint fix, landed after this
  task's pins were cut).
- Tags (verified via `git ls-remote --tags`):
  - `v0.1.0` → `f0c1bb3a` (annotated `12775c78`)
  - `v0.1.1` → `fa646941`
  - `v0.1.2`, `v0.1.3` → intermediate cascade tags
  - `v0.1.4` → `9cabf1af` — **the pin consumers use**
  - `v0.1.5` → `3bb7039e` — newest, post-dates this task's locks
- Gates (Phase 1, run solo after an earlyoom kill during concurrent
  graphopt testing):
  - `uv sync --group dev` ✓
  - `pytest tests/ -q`: **3038 passed, 108 skipped**, 22 warnings,
    2385s (39m45s)
  - `ruff check src` ✓, `ruff format --check src` ✓
  - `ty check` ✓, `lint-imports` ✓ ("kit_torch binds only its
    declared dep closure KEPT")
  - coverage floor `fail_under = 76` measured on the carved suite
    (4734 stmts / 76%); e2e/cross-stack coverage stays in graphopt.

## Consumer migrations

| repo | commits | pin landed | notes |
|---|---|---|---|
| kit-carriers | rename + isort + `bbadeda` lock | kit-torch `v0.1.4`→converged `v0.1.5` era tags (`34c1339`) | `inv` moved into kit-torch core bindings: `CARRIER_OPS`/`CORE_OPS` expectations updated (`inv` core, `bdiag` carrier-only); 10 registry tests pass. Later convergence commits by ecosystem agent. |
| kit-cuda | rename + isort + lock | kit-torch `v0.1.4`, kit-cuda `v0.1.5` tag | 52 tests passed post-migration. |
| kit-discovery | `a0ba534` rename, `ec3c8eb` isort, `7d507c5` pins, `88d93db` convergence | **`v0.1.3` tag = `88d93db`** (cut by ecosystem agent, includes the rename) | Gate evidence below. |
| kernelopt | `5a1679b` rename, `e05af9f` deps+lock | kit-torch `v0.1.4`, eqsat `v0.1.12` | pushed `177c0d9..e05af9f`; later `7f3647a` codec commit by ecosystem agent. |
| mlopt | `9e2c53e` rename, `095652e` deps+lock | kit-torch `v0.1.4`, eqsat `v0.1.12` | **only `pyproject.toml`, `uv.lock`, and a one-line fix in `scripts/train_delivery_prior.py`** (`mlopt_torch.backend` → `kit_torch.backend`, a stale import in the other agent's now-committed file). No `uv sync` run (training env untouched); untracked `arch-*.{hnl,safetensors}` artifacts left alone. Later `adae555` by ecosystem agent. |
| graphopt | `ab8b8c61` removal | kit-torch `v0.1.4` + converged set | see below. |
| kit-onnx / kit-host-tensor | doc/forbidden-name renames (`9b5e738`, `cad86d9`) + ecosystem repins to `v0.1.4` | — | `mlopt_torch` kept in their import-linter forbidden lists intentionally (transition dual-spelling). |

Converged pin set used everywhere: eqsat `v0.1.12`
(`mlopt-core`/`mlopt-orchestrator`/`mlopt-dtypes`), kit-torch
`v0.1.4`, kit-carriers `v0.1.5`, kit-cuda `v0.1.5`, kit-discovery
`v0.1.3`, kit-onnx `v0.1.4`, kit-host-tensor `v0.1.4`, gamekit/
benchkit `v0.1.14`, discoverykit `v0.1.23`.

## graphopt removal (`ab8b8c61`, tagged `v0.1.22`)

Done in clone `/home/luis/tmp/graphopt-d1e` (branch `d1e-kit-torch`),
rebased twice over concurrent stepe merges (`3cab3d9f`, `0e1c3a5c` —
the stepe agent's `override-dependencies` hold was explicitly "until
D-1e lands"; this commit is that landing, so the overrides were
dropped and all tags adopted).

- Deleted `packages/mlopt-torch/` (33 modules + 2 policy `.pt`
  artifacts) — history preserved in kit-torch.
- Deleted the 65 package-bound test files; **kept ~23 cross-stack
  files** (56 `.py` remain incl. helpers) that exercise `kit_torch`
  through the git dep.
- `tests/numpy_sink.py` (new): the `NumpySink`/`_model` double carved
  out of `test_pluggable_sink.py` (which moved to kit-torch) —
  `test_search_lower_split.py` (stays for its `kit_cuda` edge)
  imports it; without this the suite failed at collection.
- `pyproject.toml`: `kit-torch` git dep replaces the workspace
  member; coverage tables removed (floors live in kit repos);
  import-linter contracts pruned to the remaining domain layering;
  vulture `ignore_names += [cap, dev, max_iterations]` (test-double
  params previously masked by mlopt-torch's own uses of those names).
- Docs/AGENTS/README/CI/scripts/examples/manifest/.gitignore renamed
  `mlopt_torch`→`kit_torch`, `mlopt-torch`→`kit-torch`.
- Stale `packages/mlopt-torch/` egg-info/`__pycache__` debris removed
  from the main checkout after ff-merge.

Graphopt gates on the merged tree (`uv sync --group dev` fresh venv):
- ruff check/format (`packages scripts examples`) ✓
- ty ✓ (include set empty by construction), lint-imports ✓ (2
  contracts KEPT), vulture ✓, bandit ✓ (0), semgrep ✓ (0), radon ✓
- `pytest tests/ -q -m "not slow"`: **1831 passed, 211 skipped,
  3 deselected**, 44 warnings, 882s — full dev lane green.
- Post-rebase smoke (stepe merge + pin resolution): 291 passed on
  `test_search_lower_split`/`test_kit_bridge`/`test_public_api`.

Pushed: branch `d1e-kit-torch` → ff-merged into local canonical
`main` (origin is `file://` for the clone; GitHub `origin/main`
updated via `git push origin main`) → **`v0.1.22` annotated tag on
`ab8b8c61`, verified via `git ls-remote --tags`**.

## kit-discovery gate evidence

Full suite repeatedly OOM-killed (earlyoom) and once disk-full
(`/home` at 97%) due to concurrent agent workloads (kit-discovery-
stepe suite, vitest, graphopt suites).  Ran the documented
`-m "not slow"` lane **sharded by test file**:

- ac: 606 passed, 1 skipped · ad: 618 passed, 1 skipped (rerun after
  infra disk-full) · ae: 466 passed, 1 skipped · af: 413 passed,
  1 skipped · ab: 521 passed · aa: 311 passed + 1 failure
- The single failure (`test_case_tail_seats_typed_bounds`) is a
  `elapsed < 120s` perf sanity assert that took 236s under host load;
  **re-run alone: passed in 2.06s** — load flake, not a regression.
- Static gates all green: ruff, format, ty (3 known lazy-import
  warnings: `bench...law_bench`, `kit_cuda` ×2), lint-imports KEPT.
- `-m slow` subset (3 files: `test_om_decayed_cross`,
  `test_discovery_open_{prior,learned}`) was still running at report
  time (~70 min in; these are the minutes-scale saturations).  The
  src under test is identical between `7d507c5` and tagged `88d93db`
  (deps-only delta), so the not-slow evidence carries to `v0.1.3`.

## Remaining `mlopt_torch`/`mlopt-torch` spellings (all intentional)

- `graphopt/pyproject.toml:177`, `AGENTS.md:13` — historical split
  notes.
- `kit-host-tensor/pyproject.toml:106`, `kit-onnx/pyproject.toml:98,114`
  — transition dual-spelling import-linter forbidden lists (documented
  in their AGENTS files).
- `kit-discovery/build/` — gitignored stale build artifacts.
- `.pyc` caches — regenerated.
- gamekit/discoverykit/eqsat/kit-prelude: **zero** executable refs.

## Caveats / handoff notes

- Consumers pin `kit-torch v0.1.4`, not `v0.1.0` — deliberate
  convergence (see top).  `v0.1.5` (`3bb7039e`) exists now; next
  cascade can adopt it (kit-carriers' in-flight dirty pyproject
  already references it).
- Concurrent agents landed stepe/term-repr-B commits across the
  ecosystem during this task; graphopt's `override-dependencies`
  scaffolding was removed by this commit as designed.
- kit-carriers has unrelated dirty files (stepe lowers + dev pin
  bump) — left untouched.
- graphopt `v0.1.22` tag is on `ab8b8c61`; local canonical checkout
  `/home/luis/GitHub/handl-lang/graphopt` is clean at that commit.
