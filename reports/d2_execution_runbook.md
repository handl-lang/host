# D-2 execution runbook — merge product into graphopt + GitHub renames

Companion to `d2_merge_prep.md` (authoritative plan, §8 checklist). **Nothing in this document has been executed.** Blocks marked **APPROVAL-REQUIRED** perform public/irreversible actions (GitHub renames, archival, filesystem moves of live checkouts).

> **Preflight deltas applied** — rewritten from verified findings in `d2_gh_preflight.md` (same session): gh auth PASSED (luisfuturist, `admin:org`/`repo`/`delete_repo` scopes — no blockers); anchors refreshed (mlopt `24f16b5`→`095652e`, graphopt `af46e8f`→`e4c639b`); §3.5 pyproject repin list found STALE (zero `graphopt.git` pyproject pins — all 10 dep URLs are rename-safe via redirects); arch-*.safetensors (622M+588M) exceed GitHub's 100 MB limit — D4 partially resolved; worktree/branch counts grew; `handl_deps.env` off-by-one + stale entry documented. Repo facts: `handl-lang/mlopt` PRIVATE, `handl-lang/graphopt` PUBLIC, `handl-lang/mlopt-product` 404 (name free).

| Anchor | Value |
|---|---|
| mlopt HEAD / origin/main | `095652e707e92e61d2ee615c535d40cb04a46b66` (`095652e`) |
| graphopt HEAD / origin/main | `e4c639b6dcb084b3c7a52bcc308793844d5333ec` (`e4c639b`) |
| graphopt tag `v0.1.21` | `389e3f22e1b302a3c89bf748f6ee201e477ae32f` |
| mlopt tags | none |

## 0. Pre-flight audit findings (verified by preflight — re-run at execution)

**mlopt** (`/home/luis/GitHub/handl-lang/mlopt`): `main` clean vs `origin/main`, **zero unpushed commits**; earlier dirty tree now committed (`095652e`, `9e2c53e`, `37f5902`); `status -uall` shows only untracked `arch-*.{hnl,safetensors}` (D4); `stash` empty; single remote `origin` (SSH); no tags; no nested `.git`. **NEW linked worktree**: `/home/luis/tmp/mlopt-stepe` on `stepe/mlopt-stage-codec` — **~83 commits, no upstream** (local-only; push-vs-accept-loss is D9).

**graphopt** (`/home/luis/GitHub/handl-lang/graphopt`): `main` == `origin/main` (`e4c639b`), clean; `stash@{0}` still present (survives `mv` — lives in `.git`). **FOUR worktrees**: main; `graphopt/project` (branch `project`, ahead 1 of `origin/project`); `/home/luis/tmp/graphopt-d2-dryrun` (detached `aff8902`); `/home/luis/tmp/graphopt-stepe` (`stepe/graphopt-bench-codec`). **Local-only branches** (no upstream): `stepe/graphopt-bench-codec` ~1319, `d1e-kit-torch` ~1317, `split-mlopt-discovery` 134, `split-mlopt-cuda` 7, `kit-onnx-split` 3, `project` +1 — survive `mv` in `.git` but have no remote backup (D9). Single remote `origin` (SSH); tags `v0.1.0`…`v0.1.21`.

Freshness check (run before execution):

```sh
for r in /home/luis/GitHub/handl-lang/mlopt /home/luis/GitHub/handl-lang/graphopt; do
  echo "=== $r ==="
  git -C "$r" log --branches --not --remotes --oneline | head
  git -C "$r" status -uall --porcelain | head -30
  git -C "$r" stash list; git -C "$r" worktree list; git -C "$r" remote -v
done
gh repo view handl-lang/mlopt --json name,visibility,isArchived
gh repo view handl-lang/graphopt --json name,visibility,isArchived
```

Also verify no consumer is mid-`uv sync`/`cargo build` against either checkout — the `mv` breaks absolute paths (`host-tensor/tools/aten_extract.py:116`, ~40 bench-driver/docs paths — prep §6).

## 1. Merge commit sequence (inside graphopt, pre-rename)

All commands run in `/home/luis/GitHub/handl-lang/graphopt` on `main`.

### 1.1 Bring mlopt content in

```sh
cd /home/luis/GitHub/handl-lang/graphopt
mkdir -p packages/mlopt
# Excludes: .git, .venv, uv.lock, Cargo.lock, .gitignore,
#   arch-*.safetensors (622M+588M — EXCEED GitHub 100MB limit, must NOT
#   be committed; D4), .github/workflows if any, ci.sh, bupha.toml.
rsync -a --exclude='.git' --exclude='.venv' --exclude='uv.lock' \
      --exclude='Cargo.lock' --exclude='.gitignore' \
      --exclude='arch-*.safetensors' \
      /home/luis/GitHub/handl-lang/mlopt/ packages/mlopt/
rm -f packages/mlopt/uv.lock packages/mlopt/Cargo.lock
git add packages/mlopt && git status --short | head -40
```

**D4 (partially resolved by facts):** commit the two `.hnl` files (`arch-gpt2-small.hnl` 2.6M, `arch-t5-small.hnl` 243K) into `packages/mlopt/`; `.safetensors` are rsync-excluded and must be gitignored in the merged repo (add `arch-*.safetensors` to `packages/mlopt/.gitignore` or root `.gitignore`). `LICENSE*` → `packages/mlopt/`. Alternative history-preserving merge: `git subtree add --prefix=packages/mlopt ../mlopt main` — **D7**.

### 1.2 The four `parents[N]` fixes (preflight §8: exactly these sites)

File-depth: `src/mlopt/*.py` → `packages/mlopt/src/mlopt/` (root = `parents[4]`); `adapters/*.py` → 6 deep (`parents[5]`); `tests/*.py` → `packages/mlopt/tests/` (root `parents[3]`, `~/GitHub/handl-lang` = `parents[4]`).

1. `packages/mlopt/src/mlopt/emit.py:2837` — `parents[2]` → `parents[4]`. `_REPO` is now the graphopt repo root (renamed `mlopt` on disk later — do NOT hardcode the dir name).
2. `packages/mlopt/src/mlopt/emit.py:2846-2851` — rewrite `_hnl_run_candidates` bases:
   ```python
   sibling = _REPO.parent
   for base in (
       _REPO / "target",
       sibling / ".ci-target" / _REPO.name,      # survives graphopt→mlopt rename
       sibling / ".ci-target" / "graphopt",     # transition fallback
   ):
   ```
3. `packages/mlopt/src/mlopt/adapters/onnx_rust.py:79-80` — comment + code `parents[3]` → `parents[5]` (line 88 `_MLOPT_ROOT.parent / "source-onnx"` still resolves `~/GitHub/handl-lang/source-onnx` ✓). `arch_rust.py` needs **no edit**.
4. `packages/mlopt/tests/test_onnx_rust_source.py:32` — `parents[2]` → `parents[4]`.
5. `packages/mlopt/tests/test_arch_rust_source.py:30` — `parents[2]` → `parents[4]`.

```sh
sed -i 's/parents\[2\]/parents[4]/' packages/mlopt/src/mlopt/emit.py
sed -i 's/parents\[3\]/parents[5]/' packages/mlopt/src/mlopt/adapters/onnx_rust.py
sed -i 's/parents\[2\]/parents[4]/' \
    packages/mlopt/tests/test_onnx_rust_source.py \
    packages/mlopt/tests/test_arch_rust_source.py
# verify: grep -n 'parents\[' on each file before committing
```

### 1.3 Root `pyproject.toml` changes (graphopt)

- `name = "mlopt"` (line 6) → `name = "mlopt-meta"` — fatal-collision fix (D1, `gamekit-meta` precedent).
- `description`: rewrite — "no importable `mlopt` package" is now false.
- `requires-python` (`>=3.11`) vs member `>=3.13` — **D3**; recommended bump root to `>=3.13`.
- Add `"mlopt"` to root deps + `mlopt = { workspace = true }` in `[tool.uv.sources]` **or** standardize `uv sync --all-packages` (D8); verify `.venv/bin/mlopt` exists after sync.
- Re-home member extras `hf`/`kokoro` (`packages/mlopt/pyproject.toml:26-38`) into root `[project.optional-dependencies]`.
- Member `packages/mlopt/pyproject.toml`: keep `[tool.ruff]` + `[tool.pytest.ini_options]`; point `[tool.uv.sources]` git pins at workspace members where applicable.
- `uv.lock` regenerates via `uv lock`/`uv sync`.

### 1.4 Docs

- Rewrite `packages/mlopt/AGENTS.md` and `README.md:11-15,33-46` paths (`src/mlopt/` → `packages/mlopt/src/mlopt/`; graphopt = this repo; MLOPT_PYTHON = `<merged-repo>/.venv/bin/python`).
- `graphopt/AGENTS.md`: add `packages/mlopt` row to Layout.
- `packages/mlopt/ci.sh`: re-point at repo root (`cd "$(dirname "$0")/../.."`, `uv sync` at root, `pytest packages/mlopt/tests`); fold into `scripts/gates.sh` or keep member-scoped (D10).
- `bupha.toml` (travels into `packages/mlopt/`): re-point `dir`/`name` post-rename; same for `~/.config/bupha/` shared config (D10).

### 1.5 Gate (all must pass before rename)

```sh
cd /home/luis/GitHub/handl-lang/graphopt
uv sync && ls .venv/bin/mlopt && .venv/bin/python -m mlopt --help
uv run pytest packages/mlopt/tests -q
uv run pytest -m "not e2e and not slow" -q
.venv/bin/ruff check packages scripts examples
.venv/bin/ty check
.venv/bin/vulture packages scripts examples
.venv/bin/lint-imports
.venv/bin/bandit -r packages -q
.venv/bin/semgrep --config auto packages/
.venv/bin/python scripts/radon_ratchet.py   # EXPECT FAIL → regen baseline deliberately (--update)
MLOPT_GATES_LANE=dev scripts/gates.sh
MLOPT_PYTHON=$PWD/.venv/bin/python \
  HANDL_EXT_DIR=/home/luis/GitHub/handl-lang/kit-prelude/target/debug \
  hnl mlopt models
```

Commit: `git commit -m "merge: mlopt product -> packages/mlopt (P3 Step D-2)"`.

## 2. GitHub operation commands — **APPROVAL-REQUIRED**

> gh auth already verified (luisfuturist; `admin:org`, `repo`, `delete_repo` — no auth blockers). Execute only after §1 gate green and §5 decisions answered. Old-repo rename FIRST frees the `mlopt` name.

### 2.1 Old mlopt repo — pick ONE fate (D2)

**Option A (recommended): rename aside, archive after soak**
```sh
# APPROVAL-REQUIRED — renames handl-lang/mlopt → handl-lang/mlopt-product
gh repo rename mlopt-product --repo handl-lang/mlopt --yes
mv /home/luis/GitHub/handl-lang/mlopt /home/luis/GitHub/handl-lang/mlopt-product
cd /home/luis/GitHub/handl-lang/mlopt-product && \
  git remote set-url origin git@github.com:handl-lang/mlopt-product.git
# after soak: gh repo archive handl-lang/mlopt-product --yes  (APPROVAL)
```
GitHub auto-redirects `handl-lang/mlopt` URLs to `mlopt-product` until 2.2 claims the name — repin lingering links in §2.5. **Option B (archive in place)** blocks the rename — rejected. **Option C (delete)** unsafe (local-only stepe branch, issues/history) — not recommended.

### 2.2 graphopt → mlopt (GitHub) — **APPROVAL-REQUIRED**

```sh
# Precondition: §2.1 done (name verified free — 404 today).
gh repo rename mlopt --repo handl-lang/graphopt --yes
```
GitHub redirects `graphopt` URLs → `mlopt` automatically.

### 2.3 Local dir rename — **APPROVAL-REQUIRED** (filesystem)

FOUR linked worktrees on the graphopt side must be removed/repaired — preflight confirmed `git worktree repair` is needed for ALL, not just `project/`. Disposition each BEFORE `mv` (D9):

```sh
cd /home/luis/GitHub/handl-lang/graphopt
git worktree list    # expect FOUR entries
git worktree remove project                             # or: git worktree move project <path>
git worktree remove /home/luis/tmp/graphopt-d2-dryrun   # detached, disposable
git worktree remove /home/luis/tmp/graphopt-stepe       # push stepe branch first (D9)
# mlopt side: /home/luis/tmp/mlopt-stepe lives OUTSIDE the renamed dir —
#   only its branch stepe/mlopt-stage-codec needs a push/accept decision (D9).
cd /home/luis/GitHub/handl-lang
git -C graphopt remote set-url origin git@github.com:handl-lang/mlopt.git
mv graphopt mlopt
git -C mlopt worktree list   # verify no dangling gitdir pointers remain
```

### 2.4 `handl_deps.env` edits — **APPROVAL-REQUIRED** (config file)

`~/.cache/devin/handl_deps.env` (preflight §6 — verified): `GIT_CONFIG_COUNT=174` (line 2) but **175 `GIT_CONFIG_KEY_*` lines exist** — pre-existing off-by-one; set COUNT to the actual total. Lines 339-340: **delete** `KEY_168`/`VALUE_168` (`research-mlopt` insteadOf → nonexistent dir) — stale. **No insteadOf entries exist for ANY mlopt-stack repo** (graphopt, mlopt, eqsat, gamekit, discoverykit, kernelopt, kit-*, source-onnx, kit-prelude). Add at minimum post-rename:

```sh
export GIT_CONFIG_KEY_<N>="url.file:///home/luis/GitHub/handl-lang/mlopt.insteadOf"
export GIT_CONFIG_VALUE_<N>="https://github.com/handl-lang/mlopt.git"
export GIT_CONFIG_KEY_<N+1>="url.file:///home/luis/GitHub/handl-lang/mlopt-product.insteadOf"
export GIT_CONFIG_VALUE_<N+1>="https://github.com/handl-lang/mlopt-product.git"
```

Optionally the rest of the stack — all exist under `~/GitHub/handl-lang/` **except kit-torch**, which lives at `/home/luis/tmp/kit-torch-work` (pin that path if added).

### 2.5 Repin URLs (post-rename, same window)

**Correction from preflight:** zero `pyproject.toml` files reference `graphopt.git` — the old sed list (kernelopt/kit-cuda/kit-carriers/kit-discovery pyprojects) is dead; those pins were re-homed to `eqsat`/`gamekit`/`kit-*`/`discoverykit`/`lib-kernel-ir` during the D-1 splits, all rename-safe via GitHub redirects. Real repin surface:

```sh
# mlopt/packages/mlopt-experiments/Cargo.toml:6,29,60  (git = "…/graphopt", [patch])
# mlopt/packages/mlopt-experiments/src/bin/conv-bn-fold.rs:55  (doc comment raw.githubusercontent)
# mlopt/manifest/kit.toml:18,95,108,152                  (name/repo/artifact strings)
# mlopt/packages/mlopt-kit/.handl.toml:25               (comment only)
# mlopt/ci.sh  (~10 self-name sites: :13,39,45,50,52,53,56,60,67,71 incl.
#               .ci-target/graphopt + ../graphopt symlink logic)
# ~/.config/bupha/bundles/ci-gate.toml:492-493           (dir=…/graphopt,
#               name="handl-lang/graphopt", secret_env=BUPHA_WH_HANDL_LANG_GRAPHOPT)
# packages/mlopt/bupha.toml                              (committed manifest)
# .github/workflows/{bench,ci}.yml                       (clean — no graphopt string)
sed -i 's|github.com/handl-lang/graphopt|github.com/handl-lang/mlopt|g' \
    packages/mlopt-experiments/Cargo.toml manifest/kit.toml
# then hand-edit ci.sh self-name strings + ci-gate.toml dir/name/secret_env
# and packages/mlopt/bupha.toml dir
```

Refresh locks: `cargo metadata` in mlopt-experiments; `uv lock` only in repos whose pins actually changed (none expected in pyprojects — redirects cover them).

## 3. Rollback anchors (pre-merge state preserved)

| What | Anchor | Notes |
|---|---|---|
| graphopt pre-merge HEAD | `e4c639b6dcb084b3c7a52bcc308793844d5333ec` (`e4c639b`) | == origin/main at preflight — re-check at execution; `v0.1.21` = `389e3f2` last released |
| old mlopt product | `095652e707e92e61d2ee615c535d40cb04a46b66` (`095652e`) | == origin/main; recommend tagging `d2-pre-merge` + push before rename (APPROVAL) or rely on `mlopt-product` staying intact |
| mlopt local-only work | `stepe/mlopt-stage-codec` ~83 commits | no remote backup — push or accept loss (D9) |
| graphopt local-only work | `stepe/graphopt-bench-codec` ~1319, `d1e-kit-torch` ~1317, `split-*`, `kit-onnx-split`, `project`+1 | survive `mv` in `.git`; push or accept loss (D9) |
| graphopt stash | `stash@{0}` | survives everything |
| arch-*.safetensors | untracked, 1.2 GB | never committed (100 MB limit); rsync-excluded |
| GitHub rollback | `gh repo rename` reversible | rename back restores names; redirects re-point |

## 4. Post-rename verification

```sh
git ls-remote https://github.com/handl-lang/graphopt.git HEAD   # redirect proof
git ls-remote https://github.com/handl-lang/mlopt.git HEAD     # same repo now
git ls-remote git@github.com:handl-lang/mlopt.git HEAD
cd /home/luis/GitHub/handl-lang/kernelopt && uv lock && uv sync  # old-pin redirect proof
cd /home/luis/GitHub/handl-lang/mlopt
MLOPT_PYTHON=$PWD/.venv/bin/python \
  HANDL_EXT_DIR=/home/luis/GitHub/handl-lang/kit-prelude/target/debug \
  hnl mlopt models
.venv/bin/python -m mlopt doctor
grep -n 'handl-lang/mlopt' docs/bupha.toml ~/.config/bupha/bundles/ci-gate.toml
```

## 5. Decision records D1–D10

| # | Question | Status / recommendation |
|---|---|---|
| D1 | Meta-dist name for root `graphopt/pyproject.toml` | **`mlopt-meta`** (`gamekit-meta` precedent) — open |
| D2 | Old `handl-lang/mlopt` repo fate | **Rename → `mlopt-product`, archive after soak** — name verified free (404); mlopt clean except untracked arch-* |
| D3 | `requires-python` floor | Bump root to **`>=3.13`** — open |
| D4 | `arch-*` + `LICENSE*` destination | **PARTIALLY RESOLVED**: `.hnl` (2.6M/243K) committable → `packages/mlopt/`; `.safetensors` (622M/588M) exceed GitHub 100MB → rsync-excluded + gitignored, never committed |
| D5 | mlopt dirty tree | **RESOLVED** — all modifications committed (`095652e`); only untracked arch-* remain (D4) |
| D6 | Member tests in root pytest lane? | Keep separate initially; fold into `gates.sh` after soak — open |
| D7 | Merge style | `rsync`+squash (default) vs `git subtree add` (preserves history) — open |
| D8 | `uv sync` member-install semantics | Verify `.venv/bin/mlopt`; fallback root dep on workspace member — open |
| D9 | Local-only branches + worktrees + stash | **NEW SCOPE**: graphopt has 4 worktrees (project, tmp/graphopt-d2-dryrun, tmp/graphopt-stepe) + mlopt has tmp/mlopt-stepe. **Recommend: push `stepe/graphopt-bench-codec` (banked Step-E branch) and `stepe/mlopt-stage-codec`; for `d1e-kit-torch` (~1317 commits) document then delete after verifying its content is contained in the cascade's landed work; remove/repair all worktrees per §2.3; keep `stash@{0}`** |
| D10 | `ci.sh`/`bupha.toml` disposition | Re-point member `ci.sh` at root or fold into `gates.sh`; `bupha.toml` `dir` must be updated post-rename — open |

## Execution order summary

1. Answer D1–D3, D6–D10 → resolve D9 branch dispositions (push stepe branches; d1e-kit-torch verify-then-delete) → commit arch-*.hnl per D4.
2. §0 freshness re-run (anchors may have moved).
3. §1 merge inside graphopt → gate green → merge commit.
4. §2.1 `gh repo rename mlopt-product` (**APPROVAL**) + local `mv`.
5. §2.2 `gh repo rename mlopt` on graphopt (**APPROVAL**).
6. §2.3 worktree removals + local `mv graphopt mlopt` + remote set-url (**APPROVAL**).
7. §2.4 handl_deps.env + §2.5 repins + lock refreshes (**APPROVAL** on config edits).
8. §4 post-rename verification; archive `mlopt-product` after soak (**APPROVAL**, later).
