# D-2 GitHub pre-flight — read-only verification

Date: this session. Runbook verified: `d2_execution_runbook.md`.
**Verdict: proceed after anchor refresh + worktree/branch dispositions. No command-order changes required; several runbook facts are stale (listed below).**

## 1. gh auth — PASS

`gh` authenticated as **luisfuturist** on github.com, active, SSH git
protocol, token scopes incl. `admin:org`, `repo`, `delete_repo`.
Can see handl-lang org repos (views below succeeded).

## 2. Repo names — PASS (match runbook assumptions)

| Repo | State |
|---|---|
| `handl-lang/mlopt` | PRIVATE, not archived, default `main` |
| `handl-lang/graphopt` | PUBLIC, not archived, default `main` |
| `handl-lang/mlopt-product` | 404 — name is free for §3.1 |

## 3. Dep-URL inventory — PASS with a major runbook correction

**No `pyproject.toml` anywhere references `github.com/handl-lang/graphopt.git`.**
The §3.5 `sed` list (kernelopt:56, kit-cuda:29, kit-carriers:25,
kit-discovery:29) is **stale** — those pins were re-homed during the
D-1 splits; consumers now pin `eqsat`/`gamekit`/`kit-*`/`discoverykit`/
`kit-torch`/`lib-kernel-ir`, none of which are renamed by D-2.

Distinct `https://github.com/handl-lang/*.git` URLs across all
`*/pyproject.toml` `[tool.uv.sources]` (all rename-safe — none point at
`graphopt` or `mlopt`):

| URL | refs | Notes |
|---|---|---|
| eqsat.git | 24 | subdir pins `packages/mlopt-{core,orchestrator,dtypes}` |
| gamekit.git | 7 | subdir pins `packages/{gamekit,benchkit}` |
| kit-torch.git | 5 | tags v0.1.2/v0.1.4 |
| kit-carriers.git | 4 | |
| discoverykit.git | 4 | subdir `packages/discoverykit` |
| kit-discovery.git | 3 | |
| kit-host-tensor.git | 2 | |
| kit-cuda.git | 2 | |
| kit-onnx.git | 1 | graphopt root only |
| lib-kernel-ir.git | 1 | kernelopt only |

Non-pyproject `graphopt` URL refs that DO need repinning post-rename
(runbook §3.5 cargo/toml list confirmed accurate):
- `graphopt/packages/mlopt-experiments/Cargo.toml:6,29,60` —
  `git = "https://github.com/handl-lang/graphopt"` (no `.git`), `[patch]` section.
- `graphopt/packages/mlopt-experiments/src/bin/conv-bn-fold.rs:55` —
  doc comment `raw.githubusercontent.com/handl-lang/graphopt/main/…`.
- `graphopt/manifest/kit.toml:18,95,108,152` — name/repo/artifact strings.
- `graphopt/packages/mlopt-kit/.handl.toml:25` — comment only.
- `graphopt/ci.sh` — ~10 self-name `graphopt` sites (:13,39,45,50,52,53,
  56,60,67,71) incl. `.ci-target/graphopt` + `../graphopt` symlink logic.
- `.github/workflows/{bench,ci}.yml` — **no** `graphopt` string (clean).
- `~/.config/bupha/bundles/ci-gate.toml:492-493` —
  `dir=…/graphopt`, `name="handl-lang/graphopt"`,
  `secret_env=BUPHA_WH_HANDL_LANG_GRAPHOPT`.
- `mlopt/bupha.toml` — committed manifest, `name="handl-lang/mlopt"`,
  `dir=/home/luis/GitHub/handl-lang/mlopt` (travels into
  `packages/mlopt/`; re-point `dir` post-rename).

## 4. Local state — CHANGED since runbook §0 (anchors stale)

**mlopt** (`095652e` — runbook anchor `24f16b5` is 3+ commits stale):
- `main` == all remotes, **zero unpushed commits on main**.
- Dirty tree RESOLVED: the `deliver_train.py`/`train_delivery_prior.py`/
  `test_deliver_train.py` modifications are now committed (`095652e`,
  `9e2c53e`, `37f5902`). Only **untracked** `arch-*.{hnl,safetensors}`
  remain.
- **NEW linked worktree**: `/home/luis/tmp/mlopt-stepe` on branch
  `stepe/mlopt-stage-codec` — **83 commits, no upstream** (local-only).
- stash empty; single remote origin (SSH); no tags; no nested `.git`.

**graphopt** (`e4c639b` == `origin/main` — runbook anchor `af46e8f` stale):
- `main` clean, pushed. Stash `stash@{0}` still present (unchanged).
- Worktrees now **FOUR**, not two:
  `/graphopt` [main], `/graphopt/project` [project, ahead 1 of
  origin/project], `/home/luis/tmp/graphopt-d2-dryrun` (detached
  `aff8902`), `/home/luis/tmp/graphopt-stepe` [stepe/graphopt-bench-codec].
- Unpushed local-only branches (no upstream): `d1e-kit-torch` **1317**,
  `stepe/graphopt-bench-codec` **1319**, `split-mlopt-discovery` 134,
  `split-mlopt-cuda` 7, `kit-onnx-split` 3, `project` +1.
  → The two `stepe/*`/`d1e-*` branches are large local-only histories;
  decide push-vs-accept-loss BEFORE `mv` (they live in `.git`, survive
  the rename, but the tmp worktrees' gitdir pointers need `worktree
  repair` — runbook §3.3 covers only `project/`).
- Single remote; tags through `v0.1.21`.

**Blockers:** none hard. Soft: 4 worktrees to repair post-`mv` (runbook
names only `project`); unpushed branch commits are preserved-but-local.

## 5. arch-* artifacts — D4 partially resolved

| File | Size | >100MB? |
|---|---|---|
| arch-gpt2-small.safetensors | **622M** | YES — cannot be committed |
| arch-t5-small.safetensors | **588M** | YES — cannot be committed |
| arch-gpt2-small.hnl | 2.6M | no |
| arch-t5-small.hnl | 244K | no |

`.safetensors` must be **gitignored/excluded from the rsync**, not
committed (GitHub hard limit; also LFS not in evidence). `.hnl` files
are committable. `LICENSE`/`LICENSE-APACHE` present at mlopt root.
Runbook §2.1 rsync excludes do NOT currently exclude `arch-*` — add
`--exclude='arch-*.safetensors'` (or move artifacts to D4 destination
first). Also `mlopt/.venv`, `uv.lock` excluded already; `bupha.toml`
and `ci.sh` WILL travel — decide per D6/D10.

## 6. handl_deps.env — FOUND, needs edits

`~/.cache/devin/handl_deps.env` (28,989 bytes). Findings:
- `GIT_CONFIG_COUNT=174` but **175 `GIT_CONFIG_KEY_*` lines exist** —
  count already off-by-one before D-2; fix when editing.
- KEY_168/VALUE_168 (`research-mlopt` insteadOf →
  `file:///…/research-mlopt`, a **nonexistent dir**) — stale, delete
  (runbook correct).
- **No entries exist** for `graphopt`, `mlopt`, `eqsat`, `gamekit`,
  `discoverykit`, `kernelopt`, `kit-torch`, `kit-cuda`,
  `kit-carriers`, `kit-discovery`, `kit-host-tensor`, `kit-onnx`,
  `source-onnx`, `kit-prelude` — i.e. ALL the mlopt-stack repos lack
  insteadOf pins. §3.4's "verify which are absent" → **all absent**.
- Local dir reality: `kit-torch` has **no** `handl-lang/kit-torch`
  checkout (lives at `/home/luis/tmp/kit-torch-work`); all others
  exist under `~/GitHub/handl-lang/`.
- Recommended additions post-rename: `mlopt`→`…/mlopt` (the renamed
  graphopt), `mlopt-product`→`…/mlopt-product`, plus the missing
  stack repos as desired (eqsat, gamekit, discoverykit, kernelopt,
  kit-*, source-onnx, kit-prelude). Bump COUNT accordingly.

## 7. bupha/CI references — listed in §3 above

`ci-gate.toml:492-493` (dir+name+secret_env), `mlopt/bupha.toml`
(committed, travels), `graphopt/ci.sh` ~10 sites. bupha watcher state
(`~/bupha/worktrees/bupha/{queue.redb,history.jsonl}`) mentions
graphopt — runtime state, regenerates.

## 8. parents[N] sites — PASS, runbook list is complete

mlopt (pre-merge paths): exactly the 4 runbook sites —
`src/mlopt/emit.py:2837` (`parents[2]`),
`src/mlopt/adapters/onnx_rust.py:79-80` (comment+`parents[3]`),
`tests/test_arch_rust_source.py:30`, `tests/test_onnx_rust_source.py:32`
(both `parents[2]`). No others in `src/` or `tests/`.
graphopt `bench/`+`scripts/` `parents[` uses are all repo-root-relative
(file-depth unchanged by rename) — none hardcode the dir name; the
`_PROBES.parents[2]` comment at `bench/drivers/probes/kernelopt_triton_probe.py:60`
says "graphopt repo root" (comment-only staleness post-rename).

## Blockers / required runbook corrections

1. **Refresh anchors**: mlopt `24f16b5`→`095652e`; graphopt
   `af46e8f`→`e4c639b`. Re-run §1 freshness block at execution.
2. **§3.5 pyproject sed list is dead** — zero `graphopt.git` pyproject
   pins exist. Repin scope = Cargo.toml/manifest/kit.toml/ci.sh/
   bupha only. Keep the `sed` for safety but expect no matches.
3. **arch-*.safetensors (1.2GB total) cannot be committed** — add
   rsync exclude or gitignore decision to D4 before §2.1.
4. **Worktree count grew**: graphopt has 4 worktrees (added
   `tmp/graphopt-d2-dryrun` detached, `tmp/graphopt-stepe`); mlopt has
   a new `tmp/mlopt-stepe` worktree with **83 unpushed commits**.
   §3.3 must run `git worktree repair` for ALL linked worktrees, not
   just `project/`.
5. **Local-only branch histories are large** (`stepe/*` ~1300 commits,
   `d1e-kit-torch` 1317) — push or explicitly accept before rename;
   they survive `mv` but have no remote backup.
6. **handl_deps.env**: delete stale `research-mlopt` pair AND fix
   pre-existing COUNT=174 vs 175 keys off-by-one; add new
   mlopt/mlopt-product (+ optionally the whole missing stack) entries.
7. mlopt `bupha.toml` + `ci.sh` will travel into `packages/mlopt/` —
   confirm D6/D10 disposition covers re-pointing `dir`/`ci.sh` paths.
