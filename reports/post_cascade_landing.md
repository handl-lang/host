---
title: Post-cascade landing runbook — banked Step-E + perf work
type: runbook
date: 2026-11-19
revised: post-landing revision — kit-discovery v0.1.3/v0.1.4, kit-torch
  v0.1.4, kit-onnx v0.1.4, source-onnx eb87b63, eqsat v0.1.13 all landed;
  mlopt/kernelopt repin commits pushed; graphopt packages/mlopt-torch
  removal still in flight
status: executable today — remaining work is graphopt bench-codec + repin,
  mlopt stage-codec rebase, kernelopt dry-run, and the D-1e/D-2 merges
scope: ordered landing sequence for all banked Step-E codec branches +
  consumer repins + D-2 timing
---

# Post-cascade landing runbook

Consolidates `stepe_tail_plan.md`, `stepe_mlopt_stage.md`,
`stepe_graphopt_bench.md`, `stepe_kernelopt_codec.md`,
`stepe_kitonnx_codec.md`, `stepe_leafkits_codec.md`,
`stepe_discoverykit_codec.md`, `stepe_kitdiscovery_hnlcodec.md`,
`joint_safe_merge_prep.md`, `repin_cascade_eqsat_v0112.md`, and
`d2_execution_runbook.md` into one executable sequence.

## Delta log — what changed since the original version

The original was written mid-cascade and is now largely stale. Since
then the following **landed** (verified against remotes/tags):

- **kit-discovery `v0.1.3` (`88d93db`) and `v0.1.4` (`8491090`) pushed.**
  `8491090` is the hnl_codec → `mlopt_core.term_json` shim
  ("hnl_codec: delegate to mlopt_core.term_json") — §2 of the old
  runbook is **DONE**, not pending. The `/home/luis/tmp/kit-discovery-stepe`
  worktree no longer exists.
- **kit-torch `v0.1.4` (`9cabf1af`) pushed** — codec repoint
  (`mlopt_core.ir` term-class imports → `mlopt_core.term`) **and** the
  joint-safe preset are both on `main`. `stepe/kit-torch-term-codec` is
  merged (remote branch still exists — cleanup only).
- **kit-onnx `v0.1.4` (`b2c8ff6`) and source-onnx `eb87b63`** landed
  (kit-onnx local checkout still sits on the merged
  `stepe/term-json-repoint` branch — switch back to `main`).
- **mlopt `main` is pushed and clean of pyproject dirt**: `9e2c53e`
  (D-1e rename) + `095652e` ("kit-torch v0.1.4 + eqsat v0.1.12 cascade
  pins; fix stale mlopt_torch import") are on `origin/main`. The cascade
  pyproject bump anticipated `kit-discovery @v0.1.3` — that tag now
  exists (and `v0.1.4`); an optional bump to `v0.1.4` is all that
  remains. Only untracked `arch-{gpt2-small,t5-small}.{hnl,safetensors}`
  remain.
- **kernelopt `main` pushed**: `5a1679b` + `e05af9f` (same cascade pin
  set, `kit-discovery @v0.1.3`) on `origin/main`.
- **eqsat `v0.1.13` (`595bb35`) exists and is on `origin/main`** — the
  `const-str→arr` leaf-record flip ("Step E flip #1") is **no longer in
  flight**; it landed and is tagged. eqsat checkout is clean on
  `stepe-eqsat-arr-leaf` == `origin/main`.
- **D-2 dry-run exists**: `/home/luis/tmp/graphopt-d2-dryrun` is a
  detached-HEAD worktree at `aff8902` ("merge: mlopt product ->
  packages/mlopt (P3 Step D-2) [DRY-RUN]") on top of graphopt `e4c639b`,
  with `packages/mlopt` populated.

**Still genuinely open:**

- graphopt `packages/mlopt-torch` removal — cascade agent mid-edit in
  `/home/luis/tmp/graphopt-d1e` (branch `d1e-kit-torch`, ~303 files
  staged-modified, behind `origin/main` by 1).
- graphopt bench-codec branch `stepe/graphopt-bench-codec` `2159bc0`
  (pushed; codec GO, repin is the pre-D-1e holdout set).
- mlopt `stepe/mlopt-stage-codec` `5bf44fa` (pushed; codec swaps **not**
  on mlopt `main` — `stage.py`/adapters still import `mlopt_core.ir`).
- kernelopt `stepe-codec-dryrun` `3fc0e7d` (local-only, worktree
  `/home/luis/tmp/kernelopt-stepe`; repin half already superseded by
  `e05af9f`).

---

## 1. Prereq checks — verify before executing

```sh
# kit-discovery — DONE state; just confirm clean
git -C /home/luis/GitHub/handl-lang/kit-discovery status -sb   # main...origin/main, clean
git -C /home/luis/GitHub/handl-lang/kit-discovery tag -l 'v0.1.*'  # ..v0.1.4

# kit-torch — codec merged; confirm clean
git -C /home/luis/tmp/kit-torch-work status -sb               # only ?? build/
git -C /home/luis/tmp/kit-torch-work tag -l 'v0.1.*'          # ..v0.1.4

# eqsat — arr-leaf flip landed; confirm clean
git -C /home/luis/GitHub/handl-lang/eqsat status -sb
git -C /home/luis/GitHub/handl-lang/eqsat tag -l 'v0.1.1*'    # v0.1.10..v0.1.13

# graphopt — THE remaining blocker: packages/mlopt-torch must go
git -C /home/luis/GitHub/handl-lang/graphopt status -sb       # expect clean == origin/main
git -C /home/luis/GitHub/handl-lang/graphopt log --branches --not --remotes --oneline | head
ls /home/luis/GitHub/handl-lang/graphopt/packages/           # mlopt-torch absent = D-1e done
# cascade agent progress:
git -C /home/luis/tmp/graphopt-d1e status -sb                 # d1e-kit-torch, ~303 dirty files
# D-2 dry-run worktree (detached aff8902) — informational only

# mlopt / kernelopt — pushed; only untracked arch-* artifacts remain in mlopt
git -C /home/luis/GitHub/handl-lang/mlopt status -s           # ?? arch-*.{hnl,safetensors}
git -C /home/luis/GitHub/handl-lang/kernelopt status -sb      # clean
git -C /home/luis/GitHub/handl-lang/kernelopt worktree list   # /home/luis/tmp/kernelopt-stepe

# no stray override blocks in real checkouts
grep -rn 'override-dependencies' /home/luis/GitHub/handl-lang/*/pyproject.toml 2>/dev/null
#   expect: zero hits in real checkouts
```

**Go/no-go criteria:**

1. `graphopt/packages/mlopt-torch` deleted on `origin/main` **or** the
   D-1e removal branch pushed and queued — the graphopt repin set in
   §4.2 differs accordingly. **This is the only hard blocker.**
2. mlopt `arch-*` untracked files: decide commit-vs-ignore before D-2
   (§5) — untracked files may or may not travel with the rsync; check
   the D-2 runbook's copy semantics.
3. eqsat `v0.1.13` (arr leaf) is on `origin/main` — consumers pinned to
   `v0.1.12` keep working; a later `v0.1.13` repin is an optional
   follow-up, **not** part of this landing.

If any consumer pyproject is dirty with a *different* bump than the
ones banked here, stop — two agents editing the same pyproject will
conflict.

---

## 2. DONE — kit-discovery shim + repin (was the keystone; landed)

Evidence on `origin/main`:

- `88d93db` — "deps: repin eqsat v0.1.12 + kit-torch v0.1.4/carriers
  v0.1.5/gamekit v0.1.14/discoverykit v0.1.23 (post-cascade convergence)"
  → tagged **`v0.1.3`**.
- `8491090` — "hnl_codec: delegate to mlopt_core.term_json (canonical
  Arr leaves, legacy spellings still decode)" → tagged **`v0.1.4`**.

Consumers should target **`kit-discovery @v0.1.4`** (not v0.1.3 — v0.1.4
carries the shim itself; v0.1.3 only carries the repin).

---

## 3. Remaining banked branches — verified status and merge order

| # | branch | repo | pushed? | base | state now |
|---|--------|------|---------|------|-----------|
| 1 | `stepe/graphopt-bench-codec` `2159bc0` | graphopt | **yes** | `e4c639b` = `origin/main` | FF-able; codec GO, repin is pre-D-1e holdout set — see §4.2 |
| 2 | `stepe/mlopt-stage-codec` `5bf44fa` | mlopt | **yes** | `37f5902` (pre-`9e2c53e`) | codec swaps NOT on main; rebase now easy — see §3.2 |
| 3 | `stepe-codec-dryrun` `3fc0e7d` | kernelopt | **no** — worktree `/home/luis/tmp/kernelopt-stepe` only | old `main` `5a1679b` | repin half superseded by `e05af9f`; codec half still unique — see §3.3 |
| 4 | `stepe/kit-torch-term-codec` `9cabf1af` | kit-torch | merged | — | **DONE**: on `main`, `v0.1.4` tagged+pushed; delete remote branch |
| 5 | `stepe/term-json-repoint` | kit-onnx | merged | — | **DONE**: `b2c8ff6` on main, `v0.1.4` tagged; local checkout still on it — `git checkout main && git branch -D stepe/term-json-repoint` |
| 6 | `stepe-canonical-dialect` | source-onnx | merged | — | **DONE**: `eb87b63` on `origin/main`; branch can be deleted |
| 7 | `stepe-eqsat-arr-leaf` `595bb35` | eqsat | merged+tagged | — | **DONE**: on `origin/main`, tag **`v0.1.13`** pushed |

### 3.1 graphopt bench-codec — merge mechanics vs repin reality

The branch is FF-able onto `origin/main` (`e4c639b`) **today**, and its
`pyproject.toml`/`uv.lock` carry exactly the **pre-D-1e holdout pin
set** (kit-carriers/kit-cuda `v0.1.3`, discoverykit `v0.1.20`,
kit-discovery `v0.1.2`-era + override block). While
`packages/mlopt-torch` exists on main, that holdout set **is** the
correct repin (§4.2) — so:

- **While `packages/mlopt-torch` is still on main**: merging
  `stepe/graphopt-bench-codec` as-is lands both codec and the correct
  pre-D-1e repin in one step. This is now the recommended path — the
  "split the branch" option from the old runbook only made sense if
  the post-D-1e repin was imminent, and D-1e is not done.
- **After D-1e lands**: the merged pyproject gets superseded by the
  post-D-1e repin commit (§4.2) anyway — no rework needed.

```sh
cd /home/luis/GitHub/handl-lang/graphopt
git checkout main && git pull --ff-only origin/main
git merge --ff-only origin/stepe/graphopt-bench-codec   # or merge commit
git push origin main
```

Codec gate (from `stepe_graphopt_bench.md`, re-run post-merge):
`pytest tests/test_public_api.py` (257+),
`pytest tests/test_real_model_mint.py` (the split-brain canary — must
stay green on the holdout pins), codec smoke round-trips,
ruff/ty/lint-imports on touched files
(`bench/drivers/audit/{onnx_lifter_differential,eqsat_differential}.py`,
`tests/test_public_api.py`).

### 3.2 mlopt stage-codec — rebase is now trivial; land pre-D-2 (recommended)

mlopt `main` moved from `37f5902` → `9e2c53e` → `095652e`. The repin
half of the old conflict is **already resolved on main** — `095652e`
carries the full eqsat-v0.1.12/kit-torch-v0.1.4 pin set with no
override block. What remains is the codec half: `5bf44fa` touches
`stage.py`, `emit.py`, `adapters/{onnx_rust,arch_rust,hnl}.py`,
`carrier_expand.py`, `deliver_game.py` (still importing
`mlopt_core.ir` on main today) plus a pyproject the branch no longer
needs to own.

```sh
cd /home/luis/tmp/mlopt-stepe            # or any checkout of the branch
git fetch origin && git checkout stepe/mlopt-stage-codec
git rebase origin/main                   # conflicts: pyproject.toml + uv.lock only
#   resolve by taking MAIN's pyproject/lock wholesale — the codec
#   commit should carry NO dep changes; the optional v0.1.3→v0.1.4
#   bump is a separate one-line commit afterwards
uv lock && uv sync
.venv/bin/python -m pytest tests/test_stage.py -q    # 30
.venv/bin/python -m pytest tests/ -q                 # full suite (~265)
git push --force-with-lease origin stepe/mlopt-stage-codec
git checkout main && git merge --ff-only stepe/mlopt-stage-codec
git push origin main
# optional follow-up: kit-discovery v0.1.3 -> v0.1.4 in pyproject + uv lock
git branch -d stepe/mlopt-stage-codec
```

RECORD_VERSION stays **3** — confirmed by the earlier dry-run;
kit-prelude `RECORD_SCHEMA_VERSION=3` unchanged.

### 3.3 kernelopt dry-run — codec half only; repin already landed

`e05af9f` on kernelopt `main` already carries the eqsat-v0.1.12 +
kit-torch-v0.1.4 + kit-discovery-v0.1.3 pin set. `3fc0e7d`'s remaining
unique content is the codec repoint (term_json audit + ir_bridge
repoint). Rebase onto `origin/main`, resolve pyproject by taking
main's, land:

```sh
cd /home/luis/tmp/kernelopt-stepe
git fetch origin && git rebase origin/main   # pyproject/lock: take main's
git push -u origin stepe-codec-dryrun
cd /home/luis/GitHub/handl-lang/kernelopt
git merge --ff-only stepe-codec-dryrun && git push origin main
```

Optional follow-up (same as mlopt): `kit-discovery v0.1.3 → v0.1.4`
one-line bump — the dirty pyproject's anticipated `v0.1.3` tag now
exists **and** `v0.1.4` supersedes it.

Gate: `pytest tests/` (~242), ruff check/format, ty (needs
`../lib-kernel-ir` sibling or the worktree symlink trick).

---

## 4. Repin status — what pins exist NOW (verified)

uv propagates `[tool.uv.sources]` transitively; order rule unchanged:
each repo's tag must exist before consumers pin to it. **Every tag
needed below already exists** — no tagging work remains in this
runbook.

### 4.1 Landed pin sets (verified in pyproject.toml)

- **kit-discovery `v0.1.4`**: eqsat ×3 `v0.1.12`, kit-torch `v0.1.4`
  (dev), kit-carriers `v0.1.5`, gamekit `v0.1.14`, discoverykit
  `v0.1.23`. No overrides.
- **kit-torch `v0.1.4`**: eqsat `v0.1.12` since `03207247`; codec commit
  added nothing dep-wise.
- **mlopt `main` (`095652e`)**: eqsat ×3 `v0.1.12`, kit-torch `v0.1.4`,
  kit-carriers `v0.1.5`, kit-cuda `v0.1.5`, gamekit `v0.1.14`,
  **kit-discovery `v0.1.3`** (bump to `v0.1.4` optional), kit-host-tensor
  `v0.1.4`, discoverykit `v0.1.23`. No `mlopt-torch` source, no
  `override-dependencies`.
- **kernelopt `main` (`e05af9f`)**: same set — eqsat ×3 `v0.1.12`,
  kit-torch `v0.1.4`, kit-carriers `v0.1.5`, gamekit/benchkit `v0.1.14`,
  **kit-discovery `v0.1.3`** (→ `v0.1.4` optional), discoverykit
  `v0.1.23`, lib-kernel-ir `v0.1.1`. No overrides.

### 4.2 graphopt — the only repo still on pre-cascade pins

Actual `graphopt/pyproject.toml` on `main` (`e4c639b`) today:

```toml
mlopt-core/mlopt-orchestrator/mlopt-dtypes = eqsat v0.1.10
gamekit/benchkit  = v0.1.12
discoverykit      = v0.1.20
kit-carriers      = v0.1.3
kit-cuda          = v0.1.3
kit-discovery     = v0.1.2
kit-onnx          = v0.1.2
kit-host-tensor   = v0.1.3
mlopt-torch       = { workspace = true }   # packages/mlopt-torch member
# no kit-torch source entry; no override-dependencies
```

**Pre-D-1e set** (only while `packages/mlopt-torch` exists — i.e. what
`2159bc0` already contains): eqsat stays reachable via holdouts;
kit-carriers/kit-cuda `v0.1.3`, discoverykit `v0.1.20` are the HOLD
pins because `v0.1.4+`/`v0.1.21+` lazy-bind `kit_torch` and would
split-brain against the still-present `mlopt_torch` member. The
override block in that branch is **load-bearing**, not scaffolding.

**Post-D-1e set** (once `packages/mlopt-torch` is deleted on main):

```toml
eqsat ×3            = v0.1.12   # or v0.1.13 — see note
gamekit/benchkit    = v0.1.14
discoverykit        = v0.1.23
kit-carriers        = v0.1.5
kit-cuda            = v0.1.5
kit-discovery       = v0.1.4    # NOT v0.1.3 — v0.1.4 carries the shim
kit-onnx            = v0.1.4
kit-host-tensor     = v0.1.4
kit-torch           = v0.1.4    # NEW source entry (handl-lang/kit-torch)
# NO override-dependencies — every consumed tag carries eqsat v0.1.12
```

Note on eqsat: `v0.1.13` (`595bb35`, arr-leaf emit) is on `origin/main`
and is a strict superset of `v0.1.12` for codec purposes — either pin
resolves; prefer `v0.1.13` for new pins so the arr-leaf dialect is what
gets exercised, but do **not** mix: if any held-out package pins
`v0.1.12` sources, keep graphopt on `v0.1.12` until they move.

Then `uv lock`, `uv sync --extra dev`, graphopt gates
(`test_real_model_mint.py` is the split-brain canary), tag next graphopt
release.

### 4.3 Repos needing nothing

kit-carriers (`v0.1.5`), kit-cuda (`v0.1.5`), discoverykit (`v0.1.23`),
gamekit (`v0.1.14`), kit-host-tensor (`v0.1.4`), kit-onnx (`v0.1.4`),
kit-torch (`v0.1.4`), eqsat (`v0.1.13` tip), engine-eqsat, kit-prelude,
source-onnx — all pinned/tagged. Only verify `uv sync` cleanliness if
touched.

---

## 5. D-2 interaction — updated for landed state

Per `stepe_tail_plan.md` §8 + `d2_execution_runbook.md`, updated:

**Already satisfied on mlopt `main`:**

1. `9e2c53e` (D-1e rename) **pushed** — done.
2. Dirty-tree cleanup **done** — `train_delivery_prior.py` fix rode in
   `095652e`; pyproject cascade bump committed. **Remaining**: the four
   untracked `arch-{gpt2-small,t5-small}.{hnl,safetensors}` files —
   decide commit-vs-leave before the merge (check whether the D-2
   rsync copies untracked files; the dry-run worktree
   `/home/luis/tmp/graphopt-d2-dryrun` shows `packages/mlopt/` picked
   up the two `.safetensors` as untracked, so **untracked files DO
   travel** — commit them or delete them deliberately).
3. `stepe/mlopt-stage-codec` — see recommendation below.
4. The mlopt repin — **already landed** in `095652e` (kit-discovery at
   `v0.1.3`; optional `v0.1.4` bump is trivial post-merge either way).

**Recommendation — land `stepe/mlopt-stage-codec` on mlopt `main`
BEFORE the real D-2 merge** (changed from the old runbook's
"decision needed"):

- The rebase is now cheap: `095652e` already resolved the dep side, so
  the only conflicts are `pyproject.toml`/`uv.lock`, resolved by
  taking main's wholesale. The codec commit becomes file-local again.
- The codec gets proven by mlopt's own suite (`test_stage.py` 30,
  emit/adapter slice, full ~265) **before** the tree moves — landing
  it post-merge inside `packages/mlopt` would prove it only under
  graphopt's suite with shifted paths.
- The D-2 merge rsyncs whatever is on mlopt `main` — landing first
  means D-2 carries final files **once**, mechanically, with no
  follow-up path-shifted cherry-pick.
- The existing dry-run commit `aff8902` in
  `/home/luis/tmp/graphopt-d2-dryrun` predates `095652e` and will need
  regeneration regardless; doing the codec first keeps that
  regeneration to a single re-run.

If D-2 execution is imminent *today* (before the §3.2 rebase+gate can
run), the fallback stands: cherry-pick `5bf44fa`'s file edits onto
`packages/mlopt/...` paths post-merge — but prefer the pre-merge land.

**Unaffected by D-2 — proceed in parallel:** graphopt bench-codec +
repin (§3.1/§4.2 — graphopt *is* the D-2 target, so its repin commit
should land **before** the D-2 merge to keep `uv lock` green through
the merge), kernelopt (§3.3).

**eqsat `const-str→arr` flip — STATUS CHANGED:** flip #1 landed as
`595bb35` on `origin/main`, tagged **`v0.1.13`**. `eqsat.py`'s
`leaf="const-str"` emit may still need the consumer-side flip depending
on which leaf spelling the codec now emits — check
`mlopt_core.term_json`'s canonical form before assuming; RECORD_VERSION
stays 3 regardless unless tail-plan §5's trigger conditions appear.

---

## 6. Verification gates — per remaining step

| step | gate |
|---|---|
| §1 prereqs | all status commands clean; `git ls-remote` shows pushed state; graphopt `packages/mlopt-torch` absent (or D-1e branch queued) |
| §3.1 graphopt codec | `test_public_api.py` 257+; `test_real_model_mint.py` green on holdout pins; codec smoke round-trips; ruff/ty/lint-imports |
| §3.2 mlopt codec | `test_stage.py` 30; emit/adapter slice; full suite ~265; RECORD_VERSION=3; one `mlopt optimize` smoke |
| §3.3 kernelopt codec | ~242 tests; ruff/ty; `uv lock` resolves with zero overrides |
| §4.2 graphopt repin (post-D-1e) | `uv lock` clean; `test_real_model_mint.py` green on the post-D-1e set; `kit-discovery @v0.1.4` + `kit-torch @v0.1.4` resolve |
| §5 mlopt arch-* decision | `git status -uall` clean (committed or removed) before D-2 rsync |
| global | after all landings: `grep -rn 'override-dependencies' ~/GitHub/handl-lang/*/pyproject.toml` → **zero** in real checkouts; `grep -rn 'mlopt_core.ir import'` census unchanged-or-better (shims stay by design) |

## 7. Cleanup after landing

- Delete merged branches: kit-torch `stepe/kit-torch-term-codec`
  (remote+local), kit-onnx `stepe/term-json-repoint` (local — remote
  already gone), source-onnx `stepe-canonical-dialect`, eqsat
  `stepe-eqsat-arr-leaf` (merged+tagged), graphopt
  `stepe/graphopt-bench-codec`, mlopt `stepe/mlopt-stage-codec`,
  kernelopt `stepe-codec-dryrun` (after pushing).
- `/home/luis/tmp/kit-discovery-stepe` — **already gone**.
- Switch kit-onnx checkout back to `main` (`git checkout main`)
  before deleting its merged branch.
- Remove worktrees once their commits are merged:
  `/home/luis/tmp/{mlopt-stepe, graphopt-stepe, kernelopt-stepe,
  kit-torch-joint, graphopt-d2-dryrun}`; keep
  `/home/luis/tmp/graphopt-d1e` until the D-1e removal lands.
  `/home/luis/tmp/kit-torch-work` IS the kit-torch checkout — do not
  remove.
- `/home/luis/tmp/mlopt-stepe`'s `.venv` (~4-5 G) — removal tied to
  disk pressure.
