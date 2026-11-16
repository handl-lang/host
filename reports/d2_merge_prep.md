# D-2 merge prep — breakage inventory (mlopt → graphopt `packages/mlopt`)

Read-only recon for P3 Step D-2. Companion to `mlopt_step_d_impact_map.md`.

**5-line summary:**
1. Only **four** `parents[N]` sites break under `src/mlopt` → `packages/mlopt/src/mlopt`: `emit.py:2837` (parents[2]→[4], plus its `graphopt`/`target` + `.ci-target/graphopt` candidate literals at 2846–2851 must become repo-relative), `onnx_rust.py:80` (parents[3]→[5]; `arch_rust.py` inherits via `_onnx._source_onnx_repo()`), and two test files (`test_onnx_rust_source.py:32`, `test_arch_rust_source.py:30`, parents[2]→[4]). All other mlopt modules have zero path math.
2. The `name = "mlopt"` meta-dist collision is **fatal, not cosmetic**: uv workspace `members = ["packages/*"]` (graphopt `pyproject.toml:114`) auto-adopts `packages/mlopt`, and two distributions named `mlopt` cannot coexist — root must be renamed (recommend `mlopt-meta`, matching `gamekit-meta`/`discoverykit-meta` precedent) in the same commit.
3. Tooling blast radius is larger than the path math: graphopt's ruff (line-length 72 vs mlopt's 100 — keep member `[tool.ruff]` for hierarchical override), ty `include=["packages"]`, vulture `paths=["packages","tests"]`, radon `TARGETS=["packages"]` + `complexity_baseline.json`, bandit/semgrep `-r packages`, and `pytest testpaths=["tests"]` (member's own `testpaths` still applies when invoked under `packages/mlopt`) all newly cover the product — each needs a decision or baseline regen.
4. `handl-kit-mlopt` (`kit-prelude/crates/kit-mlopt/src/lib.rs:145-150,286-334`) resolves `MLOPT_PYTHON` → `python3 -m mlopt`; the contract is the **import name**, not the path — it survives the move iff the merged repo's `.venv` has `mlopt` installed (uv member-install semantics + the `hf`/`kokoro` extras that `mlopt/ci.sh:42` relies on must be re-homed to the root project).
5. Rename mechanics: `handl_deps.env` has **no** `graphopt`/`mlopt`/`eqsat`/`gamekit`/`discoverykit`/`kernelopt`/`source-onnx` insteadOf entries at all (only stale `research-mlopt` at lines 339-340 + `kit-onnx` at 351-352; `GIT_CONFIG_COUNT=174` must be bumped); five live `git+https://github.com/handl-lang/graphopt` uv pins (mlopt:64, kernelopt:56, kit-cuda:29, kit-carriers:25, kit-discovery:29) plus three Cargo/`.handl.toml` edges (mlopt-experiments `Cargo.toml:29,60`, `.handl.toml:34`, `conv-bn-fold.rs:55`) follow GitHub redirects but should be repinned; local dir rename breaks `host-tensor/tools/aten_extract.py:116` and ~40 hardcoded `/home/luis/GitHub/handl-lang/graphopt/...` paths in bench drivers/docs JSONs.

---

## 1. `parents[N]` path math — complete census

File-depth reference: today `src/mlopt/<mod>.py` is 3 dirs below repo root (`parents[2]` = root for `emit.py`); `src/mlopt/adapters/<mod>.py` is 4 (`parents[3]` = root); `tests/<t>.py` reaches the *sibling parent* (`parents[2]` = `~/GitHub/handl-lang`). Post-move everything gains exactly **two** levels (`packages/mlopt/` wrapper).

| Site | Today | Resolves to today | Post-move resolves to | Required fix |
|---|---|---|---|---|
| `src/mlopt/emit.py:2837` `_REPO = Path(__file__).resolve().parents[2]` | parents[2] | mlopt repo root `/…/handl-lang/mlopt` | `…/packages/mlopt` (member root) — **wrong** | `parents[4]` → graphopt repo root |
| `emit.py:2846-2851` `_hnl_run_candidates` | `_REPO.parent / "graphopt" / "target"`, `_REPO.parent / ".ci-target" / "graphopt"` | `~/GitHub/handl-lang/graphopt/target/{debug,release}/hnl-run` and `~/GitHub/handl-lang/.ci-target/graphopt/{debug,release}/hnl-run` (the latter **exists** — confirmed `/home/luis/GitHub/handl-lang/.ci-target/graphopt/release/egraph-run.d` etc.) | With parents[4], `_REPO.parent` is still `~/GitHub/handl-lang`, but `sibling/"graphopt"` names the *pre-rename* dir; the product now lives **inside** graphopt so `target/` is `_REPO/"target"` | Rewrite candidates: `_REPO/"target"`, `_REPO.parent/".ci-target"/_REPO.name` (dynamic — survives `graphopt`→`mlopt` dir rename), keep literal `"graphopt"` as a transition fallback until ci.sh's `.ci-target` name flips |
| `src/mlopt/adapters/onnx_rust.py:80` `_MLOPT_ROOT = parents[3]` | parents[3] | mlopt repo root | `…/packages/mlopt` — **wrong** | `parents[5]` → repo root; `_MLOPT_ROOT.parent/"source-onnx"` (line 88) then still resolves `~/GitHub/handl-lang/source-onnx` ✓ |
| `src/mlopt/adapters/arch_rust.py` | none of its own | — | reuses `_onnx._source_onnx_repo()` at lines 96, 109 | **no edit** — fixed transitively |
| `tests/test_onnx_rust_source.py:32` `_SIBLING_FIXTURES = parents[2]/"source-onnx"/"tests"/"fixtures"` | parents[2] | `~/GitHub/handl-lang` → source-onnx sibling | `…/packages` — **wrong** | `parents[4]` → `~/GitHub/handl-lang` |
| `tests/test_arch_rust_source.py:30` `_SIBLING = parents[2]/"source-onnx"` | parents[2] | same | same | `parents[4]` |
| `src/mlopt/{cli,stage,optimize,deliver,deliver_game,deliver_train,doctor,play,weights,carrier_expand,models/*}.py`, `adapters/{hnl,whisper,kokoro,_kokoro_packed}.py`, `scripts/*.py` | — | grep for `__file__`/`parents[`/`/home/`/`../` found **zero** path math | unaffected | none |

Env-var escape hatches already exist for every sibling lookup (`MLOPT_HNL_RUN`, `SOURCE_ONNX_REPO`, `SOURCE_ONNX_LIFT`, `SOURCE_ONNX_ARCH`, `ARCH_WEIGHTS`, `KOKORO_SRC`), so breakage is silent-skip/degrade, not crash — but the *default* sibling resolution must be fixed.

Sibling repos looked up and whether they still resolve post-move (assuming `_REPO`/`_MLOPT_ROOT` corrected to repo root, whose `.parent` = `~/GitHub/handl-lang`):
- `source-onnx` checkout → `~/GitHub/handl-lang/source-onnx` ✓ (sibling, unchanged)
- `hnl-run`/`egraph-run` binaries → `<repo>/target/*` and `~/GitHub/handl-lang/.ci-target/<reponame>/*`; **the `.ci-target` dir name is hardcoded `graphopt` in `graphopt/ci.sh:39`** and must be renamed to `mlopt` (or made dynamic) in the same commit — until then emit.py must probe **both** `.ci-target/graphopt` and `.ci-target/mlopt`.
- `hnl`/`hnl-ext-mlopt` discovery → `HANDL_EXT_DIR`/`~/.handl/ext/`/kit-prelude `target/debug/` — all outside the repo, unaffected.
- `eqsat-run` → resolved inside `mlopt_orchestrator.eqsat` (eqsat repo, git dep) — unaffected by this move; `MLOPT_EQSAT_TIMEOUT` (`optimize.py:16`) unaffected.

## 2. pyproject / workspace

- `graphopt/pyproject.toml:6` — `name = "mlopt"` **must be renamed** (recommend `mlopt-meta`; precedent `gamekit-meta`/`discoverykit-meta`). `[tool.setuptools] packages = []` at :109-110 stays. Without this, uv fails on duplicate dist name `mlopt` once `packages/mlopt` (member via `members = ["packages/*"]` at :114) is added.
- `packages/mlopt` auto-joins the uv workspace (glob `packages/*` + has `pyproject.toml`); no `exclude` entry needed. `mlopt-handl-peer` (:123) is the precedent for a pyproject-bearing excluded member.
- **Member install semantics**: `uv sync` at root installs the root project + deps. For `mlopt` console script/`python -m mlopt` to work in the repo `.venv`, either (a) add `"mlopt"` to root `[project].dependencies` + `mlopt = { workspace = true }` in `[tool.uv.sources]`, or (b) standardize on `uv sync --all-packages` / `uv run --package mlopt`. **Verify actual uv behavior** (`uv sync` then `ls .venv/bin/mlopt`) — this is the load-bearing step for the kit-prelude contract.
- `mlopt/pyproject.toml:61-70` `[tool.uv.sources]` — all git+tag pins remain valid (tags immutable; repo URLs unaffected by the *merge*; the later graphopt→mlopt rename is covered by GitHub redirects — see §6). `mlopt-torch` pin (:64, graphopt `v0.1.16`) can become `{ workspace = true }` post-merge.
- **requires-python conflict**: mlopt `>=3.13` (:6) vs graphopt `>=3.11` (:10). uv resolves one environment; on a 3.11/3.12 interpreter the member cannot install. Options: bump root to `>=3.13` (dev venv is already 3.13) or accept mlopt-only-on-3.13. Decision needed.
- **Extras**: `mlopt/ci.sh:42` runs `uv sync --extra hf --extra kokoro`; those optional-deps (:26-38) live on the *member*, and root `uv sync --extra` cannot see member extras. Re-home `hf`/`kokoro` to root `[project.optional-dependencies]` (or a dependency-group) in the merge commit.
- **`uv.lock`**: mlopt's own `uv.lock` is deleted; graphopt's root `uv.lock` regenerates via `uv sync`/`uv lock`. mlopt's `Cargo.lock` is **stale** (references `hnl-ext-mlopt`, crate moved to kit-prelude) — delete, per impact map §A.4.
- **Member `[tool.*]` configs are inert under the root invocation** but DO apply hierarchically where supported: ruff discovers the *closest* `pyproject.toml` per file → keeping `packages/mlopt/pyproject.toml`'s `[tool.ruff]` (line-length 100, select `E,F,I,UP,B,SIM`) preserves mlopt style and avoids a mass reformat; graphopt's 72-col formatter would otherwise rewrite ~20 files. ty/pytest/coverage/import-linter/vulture/radon all read **root config only**.
- `mlopt/ci.sh` (:41-47) — `cd "$(dirname "$0")"` + `uv sync --extra hf --extra kokoro` + `pytest tests/` must be re-pointed at repo root (`uv sync` at root, `pytest packages/mlopt/tests` or `cd packages/mlopt`); fold into `scripts/gates.sh` or keep as a member-scoped stage. `bupha.toml` (:8-9 `name`/`dir`) and the shared `~/.config/bupha/bupha.toml` entry need the new name/dir.
- Repo-root artifacts `arch-gpt2-small.{hnl,safetensors}`, `arch-t5-small.{hnl,safetensors}`, `LICENSE*` — decide destination (`packages/mlopt/` vs repo root vs drop).

## 3. kit-prelude coupling

- `kit-prelude/crates/kit-mlopt/src/lib.rs:145-150` — stage-ops spawn `[MLOPT_PYTHON or "python3", "-m", "mlopt", "stage", <name>]`; `:286-295` `mlopt_python()` same order for `run_mlopt`; `:312-334` spawn + error message naming `MLOPT_PYTHON`; `:188` `MLOPT_PIPELINE_CMD` escape hatch. `hnl-ext-mlopt/src/lib.rs:14-20,38-49` forwards `hnl mlopt <cmd>` argv verbatim through the same path.
- **Contract = `import mlopt` resolvable in `$MLOPT_PYTHON`'s environment.** The move is path-agnostic *if* the merged `.venv` has the member installed (editable). Post-merge usage becomes `MLOPT_PYTHON=<merged-repo>/.venv/bin/python` — same env contract, new path. No kit-prelude code change needed; docs (`mlopt/AGENTS.md:121-138`, `README.md:33-46`) need the new checkout path.
- `kit-prelude` Cargo deps are `git branch="main"` on kit-prelude itself — unaffected.

## 4. Tests

- `mlopt/tests/` (16 files, no `conftest.py`) → `packages/mlopt/tests/`. pytest run from `packages/mlopt` picks up the member's own `[tool.pytest.ini_options] testpaths=["tests"]` (`pyproject.toml:72-73`) — works standalone; from repo root `pytest packages/mlopt/tests` also works (pytest finds the member ini as rootdir for those args; member defines no custom markers so graphopt's `requires_cuda`/`slow`/`e2e` marker warnings don't apply — mlopt tests use none).
- graphopt root `testpaths=["tests"]` (`pyproject.toml:155`) does **not** collect `packages/mlopt/tests` — product suite stays a separate lane unless deliberately merged into `scripts/gates.sh`/`ci.yml` (decision; note mlopt tests `importorskip` torch/transformers and several skip without `hnl-run`/`source-onnx` binaries — same skip-honest pattern as today).
- Fixture/path couplings: the two `parents[N]` test sites above; env overrides `SOURCE_ONNX_REPO`/`ARCH_WEIGHTS`/`MLOPT_HNL_RUN` already exist. `test_cli.py`/`test_emit.py`/`test_stage.py` resolve `hnl-run` through `emit.find_hnl_run()` — inherits the emit.py fix.

## 5. Docs / AGENTS.md

- `mlopt/AGENTS.md` — rewrite "Layout" (`src/mlopt/` → `packages/mlopt/src/mlopt/`), sibling list (graphopt is now *this repo*), "Running the CLI" paths (`../kit-prelude` still valid — siblings unchanged), verification commands (`uv sync` now at repo root; `pytest tests/` → `pytest packages/mlopt/tests` or cd). `crates/hnl-ext-mlopt` note (:101-108) already says "moved to kit-prelude" — fine.
- `mlopt/README.md:11-15` — "engine lives in eqsat, graphopt, kernelopt" → post-merge graphopt *is* this repo; quick-start `uv sync --extra hf` paths change.
- `graphopt/AGENTS.md` — add `packages/mlopt` row; its own `name = "mlopt"` references (:3 repo name) update at rename.
- Org docs links to `github.com/handl-lang/graphopt` (impact map §A.5.9: host docs ×2, host paper ×2+bib, eqsat/gamekit frozen-api, kernelopt README ×4, graphopt docs/site self-links, pitch slides) — GitHub redirects cover; repin canonical ones later.

## 6. Repo rename mechanics

**Pre-rename checks:** per global repo-deletion checklist on **both** repos: `git log --branches --not --remotes` (unpushed), `status -uall` (incl. `.env*`), `stash list`, `worktree list`, nested `.git`, extra remotes. mlopt has `arch-*.safetensors` artifacts possibly unpushed.

**uv pins follow redirects:** GitHub redirects renamed-repo HTTPS URLs including `git clone`/`ls-remote`, so `git+https://github.com/handl-lang/graphopt.git` deps keep resolving post-rename (uv/libgit2 + `git` CLI both follow the redirect; `tag=`+`subdirectory=` pins are immutable-safe). Verify post-rename: `git ls-remote https://github.com/handl-lang/graphopt.git HEAD` should still answer, and `uv lock`/`cargo metadata` refresh clean. Still repin canonical URLs in the same window (cosmetic but prevents redirect-rot if a *third* repo ever takes `graphopt`).

Live `handl-lang/graphopt` references to repin/update:
- uv: `mlopt/pyproject.toml:64` (→ workspace member anyway), `kernelopt/pyproject.toml:56`, `kit-cuda/pyproject.toml:29`, `kit-carriers/pyproject.toml:25`, `kit-discovery/pyproject.toml:29` (all `mlopt-torch @ …graphopt…@v0.1.1x`).
- Cargo/`.handl.toml`: `graphopt/packages/mlopt-experiments/Cargo.toml:6` (repository), `:29` (git dep), `:60-61` (`[patch]` — URL key only, path `../mlopt-kit` unaffected), `.handl.toml:34` (`git = …/graphopt`, `subdir = "packages/mlopt-kit"`), `src/bin/conv-bn-fold.rs:55` (`raw.githubusercontent.com/handl-lang/graphopt/main/…` fetch fallback), `packages/mlopt-kit/.handl.toml:25` (comment only).
- `manifest/kit.toml:95,108` (`repo = "handl-lang/graphopt"` ×2), `:152` workaround text.
- `docs/bupha.toml:31-32` (name+dir) + shared `~/.config/bupha/bupha.toml`; `mlopt/bupha.toml` → new name/dir.
- `graphopt/ci.sh:39,45,52,56,60,71` — `graphopt` hardcoded ~8× (`.ci-target/graphopt`, `../graphopt` self-symlink, `sed` patch of `../graphopt`) — rename strings when repo renames; `.ci-target` dir migrates `graphopt`→`mlopt` (emit.py must probe both during transition).
- `host-tensor/tools/aten_extract.py:116` — default `--torch-python` hardcodes `…/graphopt/.venv/bin/python`.
- ~40 files with absolute `/home/luis/GitHub/handl-lang/graphopt/...` paths in `graphopt/bench/drivers/probes/awq/*.py` and `docs/**/*.json` reports — regenerable measurement artifacts; low priority, fix on next regen.
- `~/.cache/devin/handl_deps.env`: **no** `graphopt`, `mlopt`, `eqsat`, `gamekit`, `discoverykit`, `kernelopt`, `kit-eqsat`, `engine-eqsat`, `source-onnx` entries exist (grep-verified; only `kit-onnx` :351-352 and stale `research-mlopt` :339-340). Add `mlopt → file:///home/luis/GitHub/handl-lang/mlopt` (+ the missing repos) and bump `GIT_CONFIG_COUNT` (:2, currently 174→~183). **Ordering hazard**: the local dir `~/GitHub/handl-lang/mlopt` is occupied by the *old* product checkout until it's renamed on disk too — sequence the filesystem renames together.
- `conv-bn-fold.rs:66` candidate `../../../mlopt/packages/mlopt-kit/hnl/conv_bn.hnl` — literal `mlopt` dir name; becomes correct exactly when the merged repo's local dir is named `mlopt`. `MLOPT_DIR` env arm (:63-65) expects `<dir>/packages/mlopt-kit/…` — point at merged repo root post-merge.

## 7. Root meta-dist name collision

`graphopt/pyproject.toml:6` `name = "mlopt"` vs member `packages/mlopt` `name = "mlopt"` — **unresolvable as-is** (uv: duplicate package name in workspace). Options: (a) rename root to `mlopt-meta` (recommended — matches `gamekit-meta`/`discoverykit-meta` meta-dist precedent; root ships `packages = []`, nothing imports it); (b) drop `[project]` entirely (breaks `uv sync` at root — not viable); (c) rename member dist (breaks `import mlopt`/`python -m mlopt`/console script — **rejected**, it's the kit-prelude contract). Also update root `description` (:8) which says "no importable `mlopt` package" — post-merge there IS one (the member).

## 8. Ordered execution checklist

0. **Decisions** (impact map §D + this doc): layout `packages/mlopt` (assumed), meta-dist name `mlopt-meta`, keep `mlopt-*` dist names, member ruff config kept, tests stay a separate lane initially, `requires-python` floor, extras re-homing.
1. **Freeze + verify**: `git ls-remote --tags` all repos; `git log --branches --not --remotes`/`status`/`stash`/`worktree list` on mlopt **and** graphopt; `myco status --prompt`.
2. **In graphopt** (pre-rename): merge `mlopt` content → `packages/mlopt/{src/mlopt,tests,scripts}` + member `pyproject.toml` (drop its `[tool.uv.sources]`/workspace-inapplicable sections as needed; keep `[tool.ruff]`, `[tool.pytest.ini_options]`); apply the four `parents[N]` fixes + emit.py candidate rewrite; rename root dist → `mlopt-meta`; add `mlopt` to root deps + `{ workspace = true }` source (or document `--all-packages`); re-home `hf`/`kokoro` extras; delete `mlopt/Cargo.lock`+`uv.lock` equivalents (they travel with old repo — just don't copy them); move `arch-*` artifacts decision; rewrite `mlopt/AGENTS.md`+`README.md` paths; update `graphopt/AGENTS.md`.
3. **Gate**: `uv sync` (verify `.venv/bin/mlopt` + `python -m mlopt --help`); `uv run pytest packages/mlopt/tests -q` (skip-honest); `ruff check packages scripts examples`; `ty check` (new mlopt errors possible — annotate, don't exclude); `vulture` (add `ignore_names` or accept findings); `lint-imports`; `bandit`/`semgrep -r packages`; `python scripts/radon_ratchet.py` (**will fail** — regen `complexity_baseline.json --update` deliberately); `scripts/gates.sh` dev lane; `cargo test` (mlopt-kit unaffected); `bash ci.sh`; `hnl mlopt models` smoke with `MLOPT_PYTHON=$PWD/.venv/bin/python` + `HANDL_EXT_DIR=../kit-prelude/target/debug`.
4. **Rename sequence**: `gh repo rename mlopt-product` on old mlopt (frees name; local dir rename `mlopt`→`mlopt-product` after checklist) → merge/rename graphopt→`mlopt` on GitHub → local `mv graphopt mlopt` (check worktrees first) → update `handl_deps.env` (add `mlopt`+missing repos, drop `research-mlopt`, bump COUNT) → repin the §6 URL list → `docs/bupha.toml`+shared bupha config → `graphopt/ci.sh`→`mlopt/ci.sh` self-name strings → `.ci-target/graphopt`→`.ci-target/mlopt` migration note → archive `mlopt-product` after soak.
5. **Post-rename verify**: `git ls-remote` old URL (redirect), `uv lock`/`cargo metadata` refresh in kernelopt/kit-*/mlopt-experiments, one consumer `uv sync` (e.g. kernelopt) on old pins to prove redirect, then repin.

## Actions the recon could not perform (parent must handle)

- All git operations — unpushed-commit/stash/worktree/remotes checks on mlopt+graphopt, `git ls-remote --tags`, rename commands, local dir moves.
- `uv`/tool behavior verification — whether plain `uv sync` installs non-dep workspace members (determines if root must dep on `mlopt`); ruff hierarchical config confirmation; ty/vulture/radon/bandit actual findings on the merged tree.
- `~/.config/bupha/bupha.toml` (shared watcher) — outside repos; must add/update the `handl-lang/mlopt` entry alongside `docs/bupha.toml:31-32`.
