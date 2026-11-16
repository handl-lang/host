# D-2 execution runbook — merge product into graphopt + GitHub renames

Companion to `d2_merge_prep.md` (authoritative plan, §8 checklist).
**Nothing in this document has been executed.** Every command block is
ready to paste; blocks marked **APPROVAL-REQUIRED** perform public or
irreversible actions (GitHub renames, repo archival, filesystem moves of
live checkouts). Pre-flight and read-only audit blocks are safe to run
now.

Recorded anchors (captured 2026-…, this session — re-verify before
execution, §0):

| Anchor | Value |
|---|---|
| mlopt HEAD / origin/main | `24f16b523a961b138319872c9d55f03ce3880802` (`24f16b5`) |
| graphopt HEAD / origin/main | `af46e8f0a8180cc9c4561f4f32ac91ab94aad412` (`af46e8f`) |
| graphopt tag `v0.1.21` | `389e3f22e1b302a3c89bf748f6ee201e477ae32f` |
| mlopt tags | none |
| GitHub names | `handl-lang/mlopt` PRIVATE; `handl-lang/graphopt` PUBLIC; neither archived |

---

## 0. Pre-flight audit findings (already run — read-only)

**mlopt** (`/home/luis/GitHub/handl-lang/mlopt`):
- `main` clean vs `origin/main` — **zero unpushed commits**
  (`git log --branches --not --remotes` empty; HEAD == `ls-remote` HEAD).
- `git status -uall`: **dirty** —
  `M scripts/train_delivery_prior.py`, `M src/mlopt/deliver_train.py`,
  `M tests/test_deliver_train.py`, and untracked
  `arch-{gpt2-small,t5-small}.{hnl,safetensors}`.
  → Must be committed (or stashed) before the merge; the arch-* artifacts
  are the §2 "artifact destination" decision point.
- `stash list` empty; `worktree list` = the checkout only; no nested
  `.git`; single remote `origin` (SSH). No tags.

**graphopt** (`/home/luis/GitHub/handl-lang/graphopt`):
- `main` == `origin/main` (`af46e8f`), **zero unpushed commits on main**.
- `git log --branches --not --remotes` is **non-empty** — but all of it is
  on stale pre-split branches, not `main`:
  `kit-onnx-split` (+3), `split-mlopt-cuda` (+7),
  `split-mlopt-discovery` (+134), and worktree branch `project`
  (`graphopt/project`, **ahead 1** of `origin/project`).
  → Not merge blockers, but the split-branch commits are preserved only
  locally; decide whether to push/delete them before the rename.
- `stash list`: **`stash@{0}` exists** (WIP on `32d1819`, touches
  `bench/drivers/mint/real_model_mint.py`, `mlopt_discovery/*`,
  `torch_bridge.py`, `tests/test_intake_defects.py`).
- `worktree list`: **two** — main checkout + linked worktree
  `/home/luis/GitHub/handl-lang/graphopt/project` (branch `project`).
  `project/.git` is a gitfile → linked worktree, *not* a nested repo.
  → The local `mv graphopt mlopt` must be done with **no processes inside
  `graphopt/project`**, or the worktree must be moved/removed first
  (`git worktree move` or `git worktree remove`).
- Single remote `origin` (SSH). Tags: `v0.1.0`…`v0.1.21` (latest
  `v0.1.21` = `389e3f2`).

Re-run these read-only commands at execution time (freshness check):

```sh
for r in /home/luis/GitHub/handl-lang/mlopt /home/luis/GitHub/handl-lang/graphopt; do
  echo "=== $r ==="
  git -C "$r" log --branches --not --remotes --oneline | head
  git -C "$r" status -uall --porcelain | head -30
  git -C "$r" stash list
  git -C "$r" worktree list
  git -C "$r" remote -v
done
gh repo view handl-lang/mlopt --json name,visibility,isArchived
gh repo view handl-lang/graphopt --json name,visibility,isArchived
```

---

## 1. Pre-flight verification commands (read-only, runnable now)

```sh
# --- mlopt ---
cd /home/luis/GitHub/handl-lang/mlopt
git log --branches --not --remotes          # unpushed commits (expect: none)
git status -uall                            # incl. .env* (currently dirty — see §0)
git stash list                              # expect: empty
git worktree list                           # expect: single entry
git remote -v                               # expect: origin only
git rev-parse HEAD                          # rollback anchor: 24f16b5…
find . -path ./.git -prune -o -name .git -print   # nested repos (expect: none)
gh repo view handl-lang/mlopt --json name,visibility,isArchived,url,defaultBranchRef

# --- graphopt ---
cd /home/luis/GitHub/handl-lang/graphopt
git log --branches --not --remotes          # non-empty: stale split branches only
git log origin/main..main --oneline         # main unpushed (expect: empty)
git status -uall                            # expect: clean
git stash list                              # stash@{0} EXISTS — decide fate
git worktree list                           # TWO entries — project/ linked worktree
git remote -v                               # expect: origin only
git rev-parse HEAD v0.1.21                  # anchors: af46e8f…, 389e3f2…
git tag --sort=-creatordate | head -3       # v0.1.21 latest
gh repo view handl-lang/graphopt --json name,visibility,isArchived,url,defaultBranchRef
```

Also verify no consumer is mid-`uv sync`/`cargo build` against either
checkout (the `mv` later breaks absolute paths like
`host-tensor/tools/aten_extract.py:116` and ~40 bench-driver/docs paths —
see prep §6).

---

## 2. Merge commit sequence (inside graphopt, pre-rename)

All commands run in `/home/luis/GitHub/handl-lang/graphopt` on `main`.
**Commit the mlopt dirty tree first** (or `git -C ../mlopt stash -u`) —
the merge source must be clean so `cp -r`/`git subtree` is faithful.

### 2.1 Bring mlopt content in

```sh
cd /home/luis/GitHub/handl-lang/graphopt
mkdir -p packages/mlopt
# Copy product tree EXCLUDING repo-meta that must not travel:
#   .git, .venv, uv.lock, Cargo.lock (stale — prep §2), arch-* artifacts
#   (destination = decision point D4), .github/workflows if any, ci.sh
#   (member-scoped stage — decision), bupha.toml.
rsync -a --exclude='.git' --exclude='.venv' --exclude='uv.lock' \
      --exclude='Cargo.lock' --exclude='.gitignore' \
      /home/luis/GitHub/handl-lang/mlopt/ packages/mlopt/
# then prune the repo-meta files that rsync carried but shouldn't:
rm -f packages/mlopt/uv.lock packages/mlopt/Cargo.lock
git add packages/mlopt
git status --short | head -40    # review before committing
```

Alternative (history-preserving, heavier): `git subtree add --prefix=packages/mlopt ../mlopt main` — preserves mlopt history in graphopt; prep §8 assumes a content merge, so `rsync` + squash is the default. **Decision D7.**

### 2.2 The four `parents[N]` fixes (exact specs from prep §1)

File-depth after move: `src/mlopt/*.py` → `packages/mlopt/src/mlopt/`
(5 dirs deep → repo root is `parents[4]`); `adapters/*.py` → 6 deep
(`parents[5]`); `tests/*.py` → `packages/mlopt/tests/` (repo root
`parents[3]`, `~/GitHub/handl-lang` is `parents[4]`).

1. `packages/mlopt/src/mlopt/emit.py:2837`
   - `parents[2]` → `parents[4]`
   - Also fix the comment context: `_REPO` is now the **graphopt repo
     root** (will be renamed `mlopt` on disk later — do NOT hardcode the
     dir name).
2. `packages/mlopt/src/mlopt/emit.py:2846-2851` — rewrite
   `_hnl_run_candidates` bases. Before:
   ```python
   sibling = _REPO.parent
   for base in (
       sibling / "graphopt" / "target",
       sibling / ".ci-target" / "graphopt",
   ):
   ```
   After (repo-relative + name-dynamic `.ci-target`, keep `graphopt`
   literal as transition fallback until `ci.sh` flips — prep §1 row 2):
   ```python
   sibling = _REPO.parent
   for base in (
       _REPO / "target",
       sibling / ".ci-target" / _REPO.name,      # survives graphopt→mlopt rename
       sibling / ".ci-target" / "graphopt",     # transition fallback
   ):
   ```
3. `packages/mlopt/src/mlopt/adapters/onnx_rust.py:79-80`
   - comment `parents[3]` → `parents[5]`; `parents[3]` → `parents[5]`.
   - Line 88 `_MLOPT_ROOT.parent / "source-onnx"` then still resolves
     `~/GitHub/handl-lang/source-onnx` ✓. `arch_rust.py` needs **no edit**
     (reuses `_onnx._source_onnx_repo()`).
4. `packages/mlopt/tests/test_onnx_rust_source.py:32`
   `parents[2]` → `parents[4]` (now reaches `~/GitHub/handl-lang`).
5. `packages/mlopt/tests/test_arch_rust_source.py:30`
   `parents[2]` → `parents[4]`.

Mechanical sed (run from graphopt root, review diff after):

```sh
sed -i 's/parents\[2\]/parents[4]/' packages/mlopt/src/mlopt/emit.py
sed -i 's/parents\[3\]/parents[5]/; s/parents\[3\]/parents[5]/' \
    packages/mlopt/src/mlopt/adapters/onnx_rust.py   # verify count == 1 code site + comment
sed -i 's/parents\[2\]/parents[4]/' \
    packages/mlopt/tests/test_onnx_rust_source.py \
    packages/mlopt/tests/test_arch_rust_source.py
```

⚠ The blanket `emit.py` sed is safe only because `parents[2]` occurs once
(verify: `grep -n 'parents\[' packages/mlopt/src/mlopt/emit.py` before
committing). The `onnx_rust.py` sed must hit exactly the `_MLOPT_ROOT`
line — check `grep -n 'parents\[' packages/mlopt/src/mlopt/adapters/onnx_rust.py`.

### 2.3 Root `pyproject.toml` changes (graphopt)

- `name = "mlopt"` (line 6) → `name = "mlopt-meta"` — **fatal-collision
  fix**, matches `gamekit-meta`/`discoverykit-meta` precedent (D1).
- `description` (line 8): "no importable `mlopt` package" is now false —
  rewrite, e.g. `"mlopt meta-dist: verified model optimization — the
  product member is packages/mlopt"`.
- `requires-python` (line 10, `>=3.11`) vs member `>=3.13` — **D3
  decision**; recommended bump root to `>=3.13`.
- Add `"mlopt"` to root `[project].dependencies` and
  `mlopt = { workspace = true }` to `[tool.uv.sources]` **or** document
  `uv sync --all-packages` — verify `.venv/bin/mlopt` exists after
  `uv sync` (load-bearing for kit-prelude `MLOPT_PYTHON` contract, prep §3).
- Re-home extras: copy member `[project.optional-dependencies]`
  `hf`/`kokoro` blocks (`packages/mlopt/pyproject.toml:26-38`) into root
  `[project.optional-dependencies]` so `uv sync --extra hf --extra kokoro`
  works at root (prep §2).
- Member `packages/mlopt/pyproject.toml`: **keep** `[tool.ruff]`
  (line-length 100 — hierarchical override prevents mass reformat),
  `[tool.pytest.ini_options] testpaths=["tests"]`; drop/neutralize its
  `[tool.uv.sources]` git pins that are superseded by workspace
  (`mlopt-torch` → `{ workspace = true }`); keep its own `uv.lock`-free,
  `[tool.hatch.build.targets.wheel]` if the member is ever built
  standalone — verify `uv build --package mlopt` or leave build-backend
  decision to D7.
- Delete nothing else; `uv.lock` regenerates via `uv lock`/`uv sync`.

### 2.4 Docs

- Rewrite `packages/mlopt/AGENTS.md`: `src/mlopt/` →
  `packages/mlopt/src/mlopt/`; graphopt becomes *this repo*; verification
  commands → `uv sync` at root, `pytest packages/mlopt/tests` (or `cd
  packages/mlopt && pytest tests`); MLOPT_PYTHON path stays
  `<merged-repo>/.venv/bin/python`.
- Rewrite `packages/mlopt/README.md:11-15,33-46` quick-start paths.
- `graphopt/AGENTS.md`: add `packages/mlopt` row to Layout.
- Repo-root artifacts `arch-*.{hnl,safetensors}`, `LICENSE*`: move into
  `packages/mlopt/` (D4) or leave at root.
- `packages/mlopt/ci.sh`: re-point at repo root (prep §2 —
  `cd "$(dirname "$0")/../.."`, `uv sync` at root,
  `pytest packages/mlopt/tests`); fold into `scripts/gates.sh` or keep
  member-scoped (D6). Update `bupha.toml` name/dir + shared
  `~/.config/bupha/bupha.toml`.

### 2.5 Gate (prep §8 step 3 — all must pass before rename)

```sh
cd /home/luis/GitHub/handl-lang/graphopt
uv sync && ls .venv/bin/mlopt && .venv/bin/python -m mlopt --help
uv run pytest packages/mlopt/tests -q          # skip-honest
uv run pytest -m "not e2e and not slow" -q     # root lane unaffected
.venv/bin/ruff check packages scripts examples
.venv/bin/ty check                             # new mlopt errors → annotate, don't exclude
.venv/bin/vulture packages scripts examples    # or accept/ignore_names findings
.venv/bin/lint-imports
.venv/bin/bandit -r packages -q
.venv/bin/semgrep --config auto packages/      # per ci.sh invocation
.venv/bin/python scripts/radon_ratchet.py      # EXPECT FAIL → regen baseline deliberately:
#   .venv/bin/python scripts/radon_ratchet.py --update
MLOPT_GATES_LANE=dev scripts/gates.sh
MLOPT_PYTHON=$PWD/.venv/bin/python \
  HANDL_EXT_DIR=/home/luis/GitHub/handl-lang/kit-prelude/target/debug \
  hnl mlopt models                            # kit-prelude smoke
```

Commit the merge: `git commit -m "merge: mlopt product -> packages/mlopt (P3 Step D-2)"`.

---

## 3. GitHub operation commands — **APPROVAL-REQUIRED**

> Public/irreversible surface. Execute only after §2 gate is green and
> the user has answered §6 decisions. Order matters: old-repo rename
> FIRST frees the `mlopt` name on GitHub.

### 3.1 Old mlopt repo — pick ONE fate (D2)

**Option A (recommended): rename aside, archive after soak**
```sh
# APPROVAL-REQUIRED — renames handl-lang/mlopt → handl-lang/mlopt-product
gh repo rename mlopt-product --repo handl-lang/mlopt --yes
# local dir rename (after stash/worktree check — mlopt has none):
mv /home/luis/GitHub/handl-lang/mlopt /home/luis/GitHub/handl-lang/mlopt-product
cd /home/luis/GitHub/handl-lang/mlopt-product && git remote set-url origin git@github.com:handl-lang/mlopt-product.git
# after soak period:
#   gh repo archive handl-lang/mlopt-product --yes   # APPROVAL-REQUIRED
```
Tradeoffs: preserves the repo + issues/history under a clear name;
GitHub auto-redirects `handl-lang/mlopt` URLs to `mlopt-product`
**until** graphopt takes the name (redirects break once a new repo
claims `mlopt` — which is exactly what 3.2 does, so any lingering
`github.com/handl-lang/mlopt` links must be repinned in §3.4/§5).

**Option B: archive in place** — keeps the name `mlopt` occupied →
**blocks** the graphopt→mlopt rename entirely. Not viable unless the
product permanently keeps a different name. Rejected.

**Option C: delete** — frees the name but loses issues/PRs/releases.
Global rules require the repo-deletion checklist (unpushed commits,
status -uall incl `.env*`, stash, worktrees, nested `.git`, extra
remotes) — mlopt currently has **dirty uncommitted work** (§0), so this
is unsafe now. Not recommended.

### 3.2 graphopt → mlopt (GitHub) — **APPROVAL-REQUIRED**

```sh
# APPROVAL-REQUIRED — renames handl-lang/graphopt → handl-lang/mlopt
gh repo rename mlopt --repo handl-lang/graphopt --yes
```
Precondition: §3.1 done (name must be free). GitHub redirects
`graphopt` URLs → `mlopt` automatically.

### 3.3 Local dir rename — **APPROVAL-REQUIRED** (filesystem)

```sh
# Preconditions: graphopt stash@{0} disposition decided (pop or keep —
# stash survives mv fine, it's inside .git), linked worktree
# graphopt/project moved or removed FIRST:
cd /home/luis/GitHub/handl-lang/graphopt
git worktree list                     # expect 2 entries
git worktree move project project     # no-op path; real fix:
#   EITHER: git worktree remove project   (if disposable)
#   OR:     mv project ../mlopt-project-tmp && git worktree repair ../mlopt/mlopt-project-tmp
#   OR simplest: keep worktree inside — after repo mv, run
#       git worktree repair /home/luis/GitHub/handl-lang/mlopt/project
# then:
cd /home/luis/GitHub/handl-lang
git -C graphopt remote set-url origin git@github.com:handl-lang/mlopt.git
mv graphopt mlopt
git -C mlopt worktree repair           # fix project worktree gitdir pointers
```

### 3.4 `handl_deps.env` edits — **APPROVAL-REQUIRED** (config file)

`~/.cache/devin/handl_deps.env`:
- Line 2: `GIT_CONFIG_COUNT=174` → bump to cover new entries (~183).
- Lines ~339-340: **delete** `KEY_168`/`VALUE_168`
  `research-mlopt` insteadOf (stale — repo is now `mlopt`).
- Append new pairs (renumber or reuse freed 168 + new indices):
  ```sh
  export GIT_CONFIG_KEY_168="url.file:///home/luis/GitHub/handl-lang/mlopt.insteadOf"
  export GIT_CONFIG_VALUE_168="https://github.com/handl-lang/mlopt.git"
  # plus the missing repos per prep §6 (eqsat, gamekit, discoverykit,
  # kernelopt, source-onnx, kit-eqsat, engine-eqsat — verify which are
  # actually absent before adding; grep first)
  ```
- `kit-onnx` entry (:174) unchanged.

### 3.5 Repin the §6 URL list (post-rename, same window)

```sh
# uv pins (all still resolve via redirect — repin for canonical-ness):
#   kernelopt/pyproject.toml:56
#   kit-cuda/pyproject.toml:29
#   kit-carriers/pyproject.toml:25
#   kit-discovery/pyproject.toml:29
#   (mlopt/packages/mlopt-torch already workspace=true post-merge)
sed -i 's|github.com/handl-lang/graphopt|github.com/handl-lang/mlopt|g' \
    /home/luis/GitHub/handl-lang/kernelopt/pyproject.toml \
    /home/luis/GitHub/handl-lang/kit-cuda/pyproject.toml \
    /home/luis/GitHub/handl-lang/kit-carriers/pyproject.toml \
    /home/luis/GitHub/handl-lang/kit-discovery/pyproject.toml
# Cargo/.handl.toml:
#   graphopt(now mlopt)/packages/mlopt-experiments/Cargo.toml:6,29,60
#   .handl.toml:34 ; src/bin/conv-bn-fold.rs:55 ; packages/mlopt-kit/.handl.toml:25
# manifest/kit.toml:95,108,152 ; docs/bupha.toml:31-32 + ~/.config/bupha/bupha.toml
# ci.sh self-name strings graphopt→mlopt (~8 sites, prep §6)
# .ci-target/graphopt → .ci-target/mlopt migration (emit.py probes both)
```

Then refresh locks: `uv lock` in each consumer repo; `cargo metadata`
in mlopt-experiments.

---

## 4. Rollback anchors (pre-merge state preserved)

| What | Anchor | Notes |
|---|---|---|
| graphopt pre-merge HEAD | `af46e8f0a8180cc9c4561f4f32ac91ab94aad412` | == origin/main; tag `v0.1.21` = `389e3f2` is the last released state |
| graphopt releases | tags `v0.1.0`…`v0.1.21` | immutable; `git checkout v0.1.21` restores released tree |
| old mlopt product | `24f16b523a961b138319872c9d55f03ce3880802` | == origin/main; **recommend tagging before merge**: `git -C ../mlopt tag d2-pre-merge 24f16b5 && git -C ../mlopt push origin d2-pre-merge` (APPROVAL-REQUIRED push) — or rely on GitHub repo `mlopt-product` remaining intact |
| mlopt dirty work | uncommitted (§0) | must be committed/stashed BEFORE merge or it is lost to the rename |
| graphopt stash | `stash@{0}` | survives everything (lives in `.git`); pop after rename if needed |
| stale split branches | `kit-onnx-split`, `split-mlopt-cuda`, `split-mlopt-discovery`, `project` | local-only commits; push or accept loss before any branch cleanup |
| GitHub rollback | `gh repo rename` is reversible | rename `mlopt`→`graphopt` and `mlopt-product`→`mlopt` restores names; redirects re-point |

---

## 5. Post-rename verification

```sh
# Redirect check — old URLs must still answer:
git ls-remote https://github.com/handl-lang/graphopt.git HEAD   # expect af46e8f… (or newer)
git ls-remote https://github.com/handl-lang/mlopt.git HEAD     # same repo now
git ls-remote git@github.com:handl-lang/mlopt.git HEAD         # SSH path
# Consumer pin proof (one repo, old graphopt URL still in its lock):
cd /home/luis/GitHub/handl-lang/kernelopt && uv lock && uv sync   # proves redirect
# Full refresh after repin:
for r in kernelopt kit-cuda kit-carriers kit-discovery mlopt/packages/mlopt-experiments; do
  (cd /home/luis/GitHub/handl-lang/$r && uv lock 2>/dev/null || cargo metadata --format-version 1 >/dev/null)
done
# Product smoke in merged repo:
cd /home/luis/GitHub/handl-lang/mlopt
MLOPT_PYTHON=$PWD/.venv/bin/python \
  HANDL_EXT_DIR=/home/luis/GitHub/handl-lang/kit-prelude/target/debug \
  hnl mlopt models
.venv/bin/python -m mlopt doctor                     # lift-gap diagnostics, no downloads
# bupha watcher picks up renamed entry:
grep -n 'handl-lang/mlopt' docs/bupha.toml ~/.config/bupha/bupha.toml
```

---

## 6. Decision points the user must answer BEFORE execution

| # | Question | Recommendation |
|---|---|---|
| D1 | Meta-dist name for renamed root `graphopt/pyproject.toml` | **`mlopt-meta`** (gamekit-meta/discoverykit-meta precedent) |
| D2 | Old `handl-lang/mlopt` repo fate | **Rename → `mlopt-product`, archive after soak.** Not delete (dirty tree, issues/history); not archive-in-place (name stays occupied) |
| D3 | `requires-python` floor | Bump root to **`>=3.13`** (dev venv already 3.13; alternative splits mlopt to 3.13-only) |
| D4 | `arch-*.{hnl,safetensors}` + `LICENSE*` destination | Move into `packages/mlopt/` (they're product artifacts); currently **uncommitted** in mlopt — commit or drop first |
| D5 | mlopt dirty tree (`deliver_train.py`, `train_delivery_prior.py`, `test_deliver_train.py`) | Commit to mlopt main before merge (it's pushed-clean otherwise) — or confirm it's throwaway and stash |
| D6 | Member tests in root pytest lane? | **Keep separate initially** (`pytest packages/mlopt/tests`); merge into `gates.sh` after soak |
| D7 | Merge style | Content merge via `rsync`+squash (default) vs `git subtree add` (preserves mlopt history — heavier, changes blame) |
| D8 | `uv sync` member-install semantics | Verify `.venv/bin/mlopt` after `uv sync`; fallback = root dep on `mlopt` workspace member or standardize `--all-packages` |
| D9 | graphopt `stash@{0}` + `project` worktree + stale split branches | Pop/keep stash post-rename; `git worktree move/remove project` before `mv`; push or delete stale branches |
| D10 | `ci.sh` disposition | Keep member-scoped `packages/mlopt/ci.sh` re-pointed at root, or fold stages into `scripts/gates.sh` |

---

## Execution order summary

1. Answer D1–D10 → commit/stash mlopt dirty tree (D4/D5) → push mlopt main.
2. §1 pre-flight re-run (fresh anchors).
3. §2 merge inside graphopt → gate green → merge commit.
4. §3.1 `gh repo rename mlopt-product` (**APPROVAL**) + local `mv`.
5. §3.2 `gh repo rename mlopt` on graphopt (**APPROVAL**).
6. §3.3 local `mv graphopt mlopt` + `worktree repair` + remote set-url (**APPROVAL**).
7. §3.4 handl_deps.env + §3.5 repins + lock refreshes (**APPROVAL** on config edits).
8. §5 post-rename verification; archive `mlopt-product` after soak (**APPROVAL**, later).
