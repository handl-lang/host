# P3 Step D-1e prep — `mlopt-torch` → `kit-torch` split

> **CORRECTION (parent, post-write):** the recon claimed "D-1d is already
> landed" — verified FALSE against remotes: `handl-lang/kit-discovery`
> has zero tags and graphopt committed `pyproject.toml` still lists
> `mlopt-discovery` as a workspace member. The agent saw the dirty
> worktree cascade, not committed state. D-1d is still in flight.
> Everything else in this report stands.

Read-only recon for the fifth graphopt domain-package dissolution
(D-1 = kit-onnx, D-1b = kit-carriers, D-1c = kit-cuda, D-1d =
kit-discovery — in flight). Pattern source: the four kit repos'
AGENTS.md + pyprojects (all preserved history via `git subtree split`).

## 1. Package census — `graphopt/packages/mlopt-torch`

**Source**: `src/mlopt_torch/` — 33 modules, **24,572 lines** total
(`src/` only; excludes the two bundled `.pt` artifacts):
`__init__` (lazy `__getattr__` surface, ~100 re-export names),
`adapters` (TorchSink/TorchSource + the lazy `kit_carriers` executor
table), `aten_classify` (generated ATen tables), `autotune`,
`backend` (TorchBackend), `calibrate`, `composer`, `contraction_policy`
(+ `artifacts/contraction_policy_{distilled,curriculum}.pt` —
`[tool.setuptools.package-data]`), `device`, `device_placement`,
`dtype_ctx`, `executors/{__init__,base}`, `export`, `folds`,
`graph_policy`, `learned_policy`, `meta_eval`, `meter`,
`model_trajectories`, `models/{__init__,hybrid,ssm}`, `regime`,
`region_fuse`, `report`, `rl`, `runners` (TorchCompileRunner +
RegionFusionRunner; lazy `kit_cuda`), `torch_bindings`,
`torch_bridge` (facade over export/bindings/eval), `torch_eval`,
`torch_export`.

**Declared deps** (`packages/mlopt-torch/pyproject.toml:10-20`):
`mlopt-core`, `mlopt-orchestrator`, `mlopt-dtypes`, `torch>=2.0`,
`numpy>=1.24`. **No kit-* deps** — `kit-carriers` is a deliberate
*undeclared lazy* edge (`adapters.py:183-196`),
`kit-cuda` likewise (`runners.py:18`), `mlopt_native` lazy
(`contraction_policy.py:67`). All three are `ModuleNotFoundError`-guarded.

**Tests**: mlopt-torch has **no in-package tests dir** — the suite is
`graphopt/tests/` (workspace-wide `testpaths`). **88 test files** import
`mlopt_torch` (350 raw matches). Package-bound candidates = files whose
imports stay inside {`kit_torch`, `mlopt_core`, `mlopt_orchestrator`,
`mlopt_dtypes`, torch, numpy, safetensors}: **~65 files** — every
`mlopt_torch`-importing file NOT in the cross-stack set. The ~23 that
stay in graphopt additionally import `kit_discovery`/`kit_carriers`/
`kit_cuda`/`kit_onnx`/`mlopt_host_tensor`/`gamekit`/`benchkit`/
`discoverykit`/`bench` (e.g. `test_torch_integration.py` is
package-bound — mlopt_core+mlopt_torch only; `test_onnx_*`,
`test_substrate_domain_edges`, `test_public_api`, `test_resilience`,
`test_optimize_routing` stay). Judgment calls to re-verify during the
carve: `test_typecheck_smoke.py` (stack-wide smoke — probably stays),
`test_egglog_oracle.py` (needs the `tests/egglog_oracle.py` helper +
opt-in `oracle` group — probably stays), `test_deform_conv2d.py`
(torchvision `importorskip` — dep-closure OK but optional-dep heavy),
`test_task_metrics.py` (numpy only — bound). conftest.py travels with
the `requires_cuda` skip machinery (copy the kit-cuda variant, minus
its `import mlopt_torch.torch_bridge` eager line → becomes
`kit_torch`).

## 2. Dep analysis — who needs whom

**kit-torch's deps are all already-external**: eqsat
(`mlopt-core`/`mlopt-orchestrator`/`mlopt-dtypes` @ v0.1.9 tags) + PyPI
torch/numpy. **Zero dependency on any kit-* repo** ⇒ kit-torch can be
created and tagged **first**, before any consumer moves. No cycle:
kit-torch → (nothing in the kit layer); kit-carriers/kit-cuda/
kit-discovery → kit-torch (one direction, declared).

**Consumers of `mlopt-torch`** (all currently resolve it as a
graphopt subdir pin or workspace member):

| Consumer | Kind | Current pin |
|---|---|---|
| graphopt root | workspace member + coverage `source` + import-linter layers + ci.yml | `{ workspace = true }` |
| kit-carriers | **declared dep** + dev-group direct URL | `graphopt@v0.1.14#subdirectory=packages/mlopt-torch` (pyproject:25) |
| kit-cuda | **declared dep** + dev-group direct URL | `graphopt@v0.1.15#…` (pyproject:29); src imports `mlopt_torch.device{,_placement}` |
| kit-discovery | **declared dep** + dev-group direct URL | `graphopt@v0.1.16#…` (pyproject:29); 23 src files + 28 test files reference it |
| mlopt (product) | **declared dep** + `[tool.uv.sources]` | `graphopt@v0.1.16` subdir (pyproject:64); ~20 `mlopt_torch` refs in `src/mlopt/` |
| kernelopt | **no declared dep** — `[tool.uv.sources]` entry only (pyproject:52), resolves transitively via kit-discovery | `graphopt@v0.1.16` subdir |
| discoverykit | **lazy undeclared** function-level imports (`genkernel.py` ×12, `handler_synth.py`, `proposal_core.py`, …) + import-linter `layers` contract entries + pyproject comments | torch-free by contract; imports stay spelled until renamed |
| gamekit | **lazy undeclared** imports in `players/{random,playbook,neural,distilled}` + `_ARTIFACT = "mlopt_torch/artifacts/contraction_policy_distilled.pt"` path string (distilled/__init__.py:34 — **must become `kit_torch/artifacts/…`**) | stdlib-only by contract |
| eqsat | pyproject import-linter `forbidden_modules` names `mlopt_torch` (pyproject:100) + docstring mentions only | no dep |
| kit-onnx | import-linter forbidden list + docstrings only | leaf, no dep |
| mlopt-host-tensor (graphopt) | docstrings only | no dep |

**Order safety**: D-1e needs **no** sequencing vs D-1d. kit-torch lands
independently; the consumer cascade (kit-carriers → kit-cuda →
kit-discovery → mlopt/kernelopt repins) can happen in any order *after*
the kit-torch tag exists, because each consumer's rename is
self-contained. The only hard edge: **kit-torch's tag must exist
before** kit-carriers/kit-cuda/kit-discovery/mlopt/kernelopt repin —
their dev-group direct URLs and `[tool.uv.sources]` all point at
`handl-lang/graphopt … subdirectory=packages/mlopt-torch` today and
must be rewritten to `handl-lang/kit-torch` (whole-repo tag pin —
kit-torch will be a single-package repo so `subdirectory` drops).

## 3. Import-rename surface (`mlopt_torch`/`mlopt-torch`)

Org-wide grep totals (includes docstrings/comments):

| Repo | Matches | Nature |
|---|---|---|
| graphopt | **2,161** | packages/mlopt-torch self-imports (~most), 88 test files, **101 bench files**, 7 scripts files (`complexity_baseline.json` alone = 574 path-keyed entries — regenerate via `radon_ratchet.py --update`), docs (187), pyproject (11), ci.yml (2: `--source=mlopt_torch`), manifest/kit.toml, examples/quickstart.py |
| kit-discovery | 169 | 23 src files (real imports) + 28 test files + pyproject + AGENTS |
| kit-carriers | 132 | src imports (`om_lower`, `omd_lower`, `scan_lower`, `decayed_lower`, `xcarrier`) + 39 test files + pyproject + conftest |
| kit-cuda | 43 | `runners.py` imports + tests/conftest.py eager `import mlopt_torch.torch_bridge` + test_runners.py |
| mlopt | 32 | `src/mlopt/{cli,stage,deliver,deliver_game,optimize}.py` real imports + pyproject + AGENTS |
| discoverykit | 39 | lazy imports in `genkernel`/`handler_synth`/`proposal_core`/etc + import-linter layers + pyproject comments |
| eqsat | 84 | almost all docstrings; pyproject forbidden_modules entry is the only functional one |
| gamekit | 25 | lazy player imports + the `_ARTIFACT` path string + pyproject comment |
| kernelopt | 13 | `bench/drivers/probes/*` + `tests/test_patterns.py` lazy `TorchSource/TorchSink` imports + pyproject source pin |
| kit-onnx | 11 | docstrings + import-linter forbidden list only |

Bench note: 101 `bench/` files reference `mlopt_torch` — mostly
docstrings/lazy probes, but every one is in graphopt and must be swept.
The bench corpus is the largest single rename chunk after the package
itself.

## 4. `[tool.uv.sources]` subtlety (the D-1b/c lesson)

uv honours sources **only at the workspace root**, and a git dep's own
`[tool.uv.sources]` is ignored by consumers. `mlopt-torch` is special:
it sits *below* three already-split repos, so **no single pin fits all
consumers** — the established convention (kit-carriers pyproject:20-31,
kit-cuda:23-29, kit-discovery:23-29) is:

- **Consumer pyprojects that must change** (in cascade order):
  1. `kit-carriers/pyproject.toml` — dep name `mlopt-torch`→`kit-torch`;
     dev-group URL → `kit-torch` repo (drop `subdirectory`).
  2. `kit-cuda/pyproject.toml` — same; conftest eager import → `kit_torch`.
  3. `kit-discovery/pyproject.toml` — same.
  4. `mlopt/pyproject.toml:64` — source → `kit-torch` repo tag.
  5. `kernelopt/pyproject.toml:52` — source → `kit-torch` repo tag.
  6. `graphopt/pyproject.toml` — remove `mlopt-torch = { workspace = true }`,
     add `kit-torch = { git = "…/kit-torch.git", tag = "<new>" }`,
     update deps/import-linter/coverage/ty/ci.yml.
- **Tag order**: create `handl-lang/kit-torch` repo → subtree split →
  rename → gates green → **tag `v0.1.0`** → only then repin consumers
  (each consumer's next tag bumps after its own rename lands).
- **dev-group direct URLs** in kit-carriers/kit-cuda/kit-discovery must
  be rewritten in the same commit as their src rename (they currently
  pin `graphopt@v0.1.14/15/16` — those tags keep working forever as
  rollback anchors per the impact map §B).

## 5. Ordering recommendation

- kit-torch lands independently; the consumer cascade can happen in any
  order *after* the kit-torch tag exists.
- **No cycle**: kit-torch's dep closure (eqsat ×3 + torch + numpy)
  contains no kit-* repo. The *apparent* cycle (kit-carriers ↔
  mlopt-torch) is already resolved by the undeclared-lazy convention —
  kit-torch keeps `adapters.py`'s `try/except ModuleNotFoundError`
  carrier table and `runners.py`'s lazy `kit_cuda` import **unchanged**;
  do NOT add `kit-carriers`/`kit-cuda` to kit-torch's deps or to its
  `[tool.uv.sources]` (a declared cycle is not installable —
  mlopt-torch/pyproject.toml:21-29 documents this verbatim).
- **Cascade is fan-out, not a chain**: after the kit-torch tag, the
  three kit consumers + mlopt + kernelopt + discoverykit/gamekit lazy
  renames are independent — but each needs its own commit+tag before
  graphopt can drop `packages/mlopt-torch` (graphopt's suite still
  imports `kit_torch` through the git dep, same as it does for the
  other kits).
- **gamekit/discoverykit wrinkle**: both bind `mlopt_torch` lazily and
  undeclared — their renames are *code-only* (no dep change) but they
  still need commits; ideally retag (`gamekit v0.1.12`, `discoverykit
  v0.1.18`) so consumers pick up `kit_torch` spellings, else their lazy
  paths silently decline in post-split envs. eqsat needs only a
  pyproject forbidden-list update (`mlopt_torch`→`kit_torch`, ideally
  listing both during transition) — low urgency, no functional break.

## 6. Execution checklist (mirroring D-1a–d)

**Phase 0 — freeze/verify**
- [ ] `git ls-remote --tags` graphopt (confirm latest tag contains
      `packages/mlopt-torch`), kit-carriers/cuda/discovery,
      mlopt, kernelopt, gamekit, discoverykit, eqsat.
- [ ] `myco status --prompt`; capture the work item.
- [ ] Confirm `handl-lang/kit-torch` name is free on GitHub.

**Phase 1 — kit-torch repo**
- [ ] Create `handl-lang/kit-torch`; `git subtree split` of
      `graphopt:packages/mlopt-torch` (preserves history).
- [ ] Rename: dir `packages/mlopt-torch`→repo-root `src/kit_torch/`;
      `mlopt_torch`→`kit_torch` and `mlopt-torch`→`kit-torch` across all
      `.py`, `pyproject.toml` (name, description, package-data key),
      docstrings. ~hundreds of self-import sites — mechanical sed +
      `ruff check`/`ty` verification. Keep `kit_carriers`/`kit_cuda`/
      `mlopt_native` lazy spellings as-is (already kit_*).
- [ ] Carve package-bound tests from `graphopt/tests/` (~65 files,
      §1) into `kit-torch/tests/`; copy `tests/conftest.py` (requires_cuda
      + Inductor cache; the eager `import kit_torch.torch_bridge` line
      from the kit-cuda variant). Rewrite `mlopt_torch`→`kit_torch` in
      carved files. Leave `tests/egglog_oracle.py` helper + cross-stack
      files in graphopt.
- [ ] New `pyproject.toml` modeled on kit-cuda's: deps
      `mlopt-core`/`mlopt-orchestrator`/`mlopt-dtypes` (eqsat subdir
      pins) + `torch`/`numpy`; `[tool.uv.sources]` for the three eqsat
      pins only (**no mlopt-torch-style consumer-resolution needed —
      kit-torch has no in-family deps**); dev group pytest/ruff/
      coverage/ty/import-linter; coverage floor = measured on the
      carved suite (expect well below 100 — ratchet convention: set at
      measurement, tighten never lower); import-linter `forbidden`
      contract = kit_cuda/kit_carriers/kit_discovery/kit_onnx/
      mlopt_host_tensor/mlopt_native/gamekit/benchkit/discoverykit/
      mlopt — the declared closure is stdlib+torch+numpy+eqsat.
- [ ] `AGENTS.md` (mirror kit-cuda's structure: layout, contracts —
      "mid-stack: below kit_carriers/kit_cuda/kit_discovery, above
      eqsat"; lazy edges documented), `README.md`, `ci.sh` if the kit
      repos have one (check kit-cuda/kit-discovery for the bupha gate
      pattern).
- [ ] Gates: `uv sync --group dev`, `pytest tests/ -q`, `ruff check
      src`, `ruff format --check src`, `ty check`, `lint-imports`,
      `coverage run -m pytest && coverage report` (set floor).
- [ ] Tag `v0.1.0`; push.

**Phase 2 — consumer cascade** (each: rename + repin + gates + tag)
- [ ] **kit-carriers**: `mlopt-torch`→`kit-torch` dep + dev URL →
      `kit-torch@v0.1.0`; rename 132 matches (src imports in 5 law
      modules + 39 test files + conftest + pyproject/AGENTS); bump
      tag → `v0.1.2`.
- [ ] **kit-cuda**: same; conftest eager import → `kit_torch.
      torch_bridge`; 43 matches; tag → `v0.1.1`.
- [ ] **kit-discovery**: same; 169 matches across 23 src + 28 test
      files; tag → `v0.1.1`.
- [ ] **gamekit**: lazy imports → `kit_torch` + `_ARTIFACT` path →
      `kit_torch/artifacts/…`; pyproject comment; tag → `v0.1.12`.
- [ ] **discoverykit**: lazy imports → `kit_torch` in genkernel/
      handler_synth/proposal_core/law_order/meta_arena/coherence2 +
      import-linter layers entries + pyproject comments; tag →
      `v0.1.18`.
- [ ] **eqsat**: pyproject forbidden `mlopt_torch`→`kit_torch` (keep
      both during transition); docstring sweep optional; tag bump per
      eqsat convention.
- [ ] **mlopt**: dep + source → `kit-torch@v0.1.0`; rename ~20 src
      refs + tests + AGENTS; `uv lock`; tag/commit.
- [ ] **kernelopt**: source pin → `kit-torch@v0.1.0`; rename 4 probe/
      test lazy imports; commit.

**Phase 3 — graphopt shrink**
- [ ] Delete `packages/mlopt-torch/`; `git rm` the ~65 carved test
      files (they now live in kit-torch — graphopt keeps the cross-
      stack ~23 + all non-mlopt_torch files).
- [ ] `pyproject.toml`: drop workspace member + `mlopt-torch` dep, add
      `kit-torch` git dep @ v0.1.0; update `[tool.uv.sources]`,
      import-linter `layers` (`kit_cuda` → `kit_torch` → `mlopt_dtypes`)
      + `forbidden` source_modules, `[tool.coverage.run] source` —
      **graphopt's `fail_under = 100` was measured on mlopt_torch;
      after the split there may be no in-repo coverage source left**
      (mlopt-host-torch remains but isn't the coverage target) —
      decide: retarget to `mlopt_host_tensor` or drop the coverage
      gate to the kit repos (D-1a-d precedent: floors moved with the
      packages).
- [ ] `.github/workflows/ci.yml`: `--source=mlopt_torch` → drop or
      retarget; coverage step may need rework (see above).
- [ ] Rename sweep: `bench/` (101 files), `scripts/` (incl.
      `complexity_baseline.json` — `radon_ratchet.py --update` after
      `packages/` shrinks; `mutmut.sh`, `runtime_types.sh`,
      `torch_port_gen.py`, `train_contraction_artifact.py`,
      `calibrate_profile.py`, `verify_vocab_models.py`), `docs/`
      (187), `examples/quickstart.py`, `manifest/kit.toml`,
      `AGENTS.md`, README — all `mlopt_torch`→`kit_torch` /
      `mlopt-torch`→`kit-torch`.
- [ ] `ty` `extra-paths`: remove `packages/mlopt-torch/src`.
- [ ] `uv lock`, full `scripts/gates.sh` (dev lane then full),
      `bash ci.sh doctor|check|test|ci` for the Rust arm (unaffected
      but run it).
- [ ] Tag graphopt next version (consumers' pins on older tags stay
      valid — tags are immutable).

**Phase 4 — docs/board**
- [ ] Update impact-map notes + graphopt AGENTS.md "repo split"
      paragraph (add D-1e).
- [ ] `myco handoff` the work item.

## Risks / watch-items

- **Coverage floor semantics**: graphopt's `fail_under=100` is
  *on mlopt_torch* — the biggest in-repo floor. Removing the package
  removes the floor's subject; the kit-torch floor will be measured on
  the carved subset only (prior splits saw 43–99% — expect kit-torch
  well below 100 since the e2e/law suites stay in graphopt).
- **`conftest.py` eager backend registration**: kit-cuda's conftest
  imports `mlopt_torch.torch_bridge` so subset runs see the registered
  concrete-eval backend — kit-torch's conftest needs the same line
  spelled `kit_torch`.
- **`gamekit` `_ARTIFACT` string** is a *path*, not an import — the
  sweep catches it, but verify the artifact resolution still finds
  `kit_torch/artifacts/*.pt` (package-data key renamed too).
- **Dynamic/string imports**: `import_module`/`importorskip` sites in
  tests (18 files) and lazy binds in discoverykit/gamekit/kit-discovery
  were enumerated; a final `rg "mlopt_torch" ` must return **zero**
  hits org-wide before closing (except intentional historical docs).
- **mlopt product's `emit.py`/`onnx_rust.py`/`arch_rust.py`** sibling-
  path logic is unaffected (repo-layout, not package-name), but its
  `mlopt_torch` imports in stage/deliver/optimize are real and must
  land with the repin.
