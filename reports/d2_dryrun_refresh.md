# D-2 dry-run refresh — mlopt `095652e` → `adae555`

Worktree: `/home/luis/tmp/graphopt-d2-dryrun` (detached HEAD, graphopt `e4c639b` base).
Real repos untouched; all git ops confined to the worktree. No pushes, no `gh`.

## Result

Two commits on the detached HEAD:

- `aff8902` — original dry-run merge (`packages/mlopt` populated from mlopt `095652e`-era).
- **`00a5d17`** — this refresh: mlopt content updated to **`adae555`** (current `origin/main`
  HEAD — verified `git -C …/mlopt rev-parse HEAD` = `adae55580e29…`, one commit past `095652e`)
  plus dep-pin realignment. Committed (not left dirty) so the refresh delta is a clean
  `git show 00a5d17`; the worktree is clean afterwards.

mlopt `095652e → adae555` touched 10 files; 9 were re-synced into `packages/mlopt`
(`uv.lock` excluded per runbook). No files were added/removed besides the already-committed
`arch-*.hnl`.

## What adae555 changed (the "stage-codec landing")

`mlopt_core.ir` → `mlopt_core.term` / `mlopt_core.term_json` migration across
`emit.py`, `stage.py`, `adapters/{arch_rust,hnl,onnx_rust}.py`, `carrier_expand.py`,
`deliver_game.py`, plus `tests/test_stage.py` (+111 lines) and a `kit-discovery`
`v0.1.3 → v0.1.4` pin bump in mlopt's own pyproject.

## Manual fixes applied in this refresh (beyond rsync/copy)

| File | Fix |
|---|---|
| `packages/mlopt/src/mlopt/emit.py` | re-applied `parents[2]→[4]` + `_REPO` docstring + `_hnl_run_candidates` rewrite (`_REPO/"target"`, `.ci-target/_REPO.name`, `.ci-target/"graphopt"` fallback) — runbook §1.2.1-2 |
| `packages/mlopt/src/mlopt/adapters/onnx_rust.py` | re-applied `parents[3]→[5]` + comment — §1.2.3 |
| `packages/mlopt/tests/test_{onnx,arch}_rust_source.py` | already correct at `parents[4]` (unchanged by adae555) — §1.2.4-5 |
| `packages/mlopt/src/mlopt/stage.py` | re-applied ruff-isort order swap in `_optimizer` (`kit_torch` before `mlopt_orchestrator`) |
| `packages/mlopt/src/mlopt/deliver_game.py` | re-applied `pass  # nosec B110` |
| `packages/mlopt/scripts/train_delivery_prior.py` | **NEW FIX — regression in aff8902**: contained `from mlopt_torch.backend import TorchBackend`; upstream mlopt (both `095652e` and `adae555`) says `kit_torch.backend`. Restored to `kit_torch` + isort order (`scripts/` isn't in mlopt's own ruff scope, so upstream never linted it; graphopt's `ruff check packages` does). |
| `packages/mlopt/pyproject.toml` | sources-table removal re-applied; net result identical to aff8902 (mlopt's only pyproject delta was inside the removed table) |
| root `pyproject.toml` `[tool.uv.sources]` | **pin realignment** (see below) |
| root `.gitignore` | added `/packages/mlopt/arch-*.safetensors` — the safetensors were previously *untracked but not ignored*; `git check-ignore` now confirms coverage |
| `uv.lock` | regenerated via `uv lock` (231 pkgs, clean resolve) |

## Dep-pin realignment (root `[tool.uv.sources]`)

The member now hard-requires eqsat **v0.1.12** (`mlopt_core.term`, `mlopt_core.term_json`
are imported at module top in `emit.py`/`stage.py`/adapters — v0.1.10 lacks them; verified:
old `.venv` raised `ImportError: cannot import name 'term_json'`).

| Package | Was | Now (= mlopt@adae555 table) |
|---|---|---|
| mlopt-core / -orchestrator / -dtypes (eqsat) | v0.1.10 | **v0.1.12** |
| gamekit / benchkit | v0.1.12 | v0.1.14 |
| discoverykit | v0.1.20 | v0.1.23 |
| kit-carriers / kit-cuda | v0.1.3 | v0.1.5 |
| kit-discovery | v0.1.2 | v0.1.4 (test_stage.py:532 needs `hnl_codec` ≥ v0.1.4) |
| kit-host-tensor | v0.1.3 | v0.1.4 |
| kit-onnx | v0.1.2 | **v0.1.4** — forced: kit-onnx v0.1.2 pins eqsat v0.1.10 and `uv lock` failed on the conflict; v0.1.4 pins v0.1.12 (v0.1.5 wants v0.1.13 — not needed) |
| kit-torch | `{ path = "/home/luis/tmp/kit-torch-dryrun" }` | `{ git = "…/kit-torch.git", tag = "v0.1.4" }` — the stale comment claimed no pushed tags; `ls-remote` confirms `refs/tags/v0.1.4` exists |

Every bumped tag verified present via `git ls-remote --tags` before pinning.

## Verification results (all in the worktree)

- `uv lock` — clean, 231 packages.
- `uv sync` + `uv sync --extra hf --extra kokoro` — clean; `.venv/bin/mlopt` exists;
  `mlopt --help` prints `models|doctor|optimize|export|stage`.
- `python -c "import mlopt.stage, mlopt.emit, mlopt.cli"` — OK under eqsat v0.1.12.
- `pytest packages/mlopt/tests -q` — **265 passed, 27 skipped** (skips are the honest
  hnl-run/source-onnx/torch-optional kind).
- `ruff check packages/mlopt` — clean; `ruff format --check` — 45 files formatted.
- `ty check` — clean after extras sync (5 `import-not-found` for transformers/
  huggingface_hub without extras — expected; with `--extra hf --extra kokoro` only 1
  pre-existing deprecation warning in `mlopt-torch`, unrelated).
- `bandit -c .bandit.yaml -r packages/mlopt` — **No issues** (the no-config run shows
  819 Low-severity noise the repo config filters; 9 `# nosec` sites honoured).
- `lint-imports` — 2 contracts kept, 0 broken.
- `parents[N]` census re-verified: `emit.py:2839` = `parents[4]`,
  `onnx_rust.py:80` = `parents[5]`, both test files = `parents[4]`; no other
  `parents[`/`__file__` path math in the member.
- Stale-ref sweep: no `mlopt-product`/`research-mlopt` refs; remaining `graphopt`
  strings are provenance docstrings + the intentional `.ci-target/"graphopt"`
  transition fallback; `bupha.toml:15 dir=/home/luis/GitHub/handl-lang/mlopt` is the
  documented post-rename path.
- `arch-gpt2-small.hnl` (2.6M) + `arch-t5-small.hnl` (243K) committed; both
  `.safetensors` deleted from the worktree (1.2G freed) and now gitignored.

## Not run (flag for the real merge)

- Full `scripts/gates.sh` dev lane over the *whole* repo (domain suite, radon
  ratchet — expected to fail until `complexity_baseline.json --update`, vulture,
  semgrep, coverage floors). Only the member-scoped gates above were run.
- `python -m mlopt doctor` / `hnl mlopt models` smoke (needs `MLOPT_PYTHON` +
  `HANDL_EXT_DIR`; cheap to add at execution).
- Runbook §2.5 repins (post-rename, out of scope here).

## GO / NO-GO

**GO for the content merge** — conditional on the real execution still running the
full §1.5 gate list. The refresh surfaced and fixed one genuine regression
(`mlopt_torch` import in `train_delivery_prior.py`) and one hard blocker the old
dry-run could not have seen (eqsat v0.1.12 API requirement vs root's v0.1.10 pins,
plus the kit-onnx transitive conflict). With pins aligned, `uv sync` + member tests
+ static gates are all green. Open D-items (D1–D3, D6–D10) and the §2 approval gates
remain prerequisites per the runbook, but nothing in the refreshed tree blocks the
merge commit itself.
