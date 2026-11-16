# Rust-side law-corpus pin check — post eqsat v0.1.9 → v0.1.10 cascade

Date: 2026-11-16 (checked after Python-side cascade; all consumers on eqsat
`tag = "v0.1.10"` = commit `4e2dc877fc0b2744bc33a1d7ff6d4a7535b9c182`,
verified via `git ls-remote --tags origin` in `handl-lang/eqsat`).

## Verdict: no stale pins found — nothing to bump, nothing committed

### 1. `handl-lang/kit-eqsat` (canonical LAW_FORMAT=2 corpus)

- HEAD `04d8064` == `origin/main` == remote HEAD (`git ls-remote origin`
  confirms `04d80644a527…` on `refs/heads/main`). Working tree clean.
- Corpus is at the canonical **524 records** (`data/laws/` ten packs;
  `prior_art` 171 after the 04d8064 growth commit).
- **No tags exist** in this repo (`git tag -l` empty; `ls-remote --tags`
  empty) — kit-eqsat is not versioned; consumers pin by branch/commit,
  not tag. Nothing to go stale.

### 2. `handl-lang/engine-eqsat`

- HEAD `dd4815e` == `origin/main` — the **arr-leaf fix
  (`{"Arr": …}` leaf records, dd4815e) is pushed**.
- Corpus consumption: **not** a submodule, vendored dir, or fetched
  asset. `Cargo.toml:38` declares
  `handl-kit-eqsat = { git = "https://github.com/handl-lang/kit-eqsat", branch = "main" }`
  as a **dev-dependency** (contract-test fixtures + `emit_laws`
  regeneration path); `laws/` holds only the quarantined
  `reshape_flatten_demo.json`. The `[patch]` dev-link table
  (`Cargo.toml:56`) resolves it to the sibling checkout
  `../kit-eqsat/crates/handl-kit-eqsat`, which is at 524 records.
- `branch = "main"` means every fresh resolve gets 04d8064/524 — there
  is no commit/tag pin that could be stale. `Cargo.lock` carries no
  `source = "git+…kit-eqsat"` entry (the patch makes it a path dep), so
  the lockfile cannot pin an old corpus either.
- `src/lawset.rs` references kit-eqsat only in doc comments.

### 3. Repo-wide grep for stale pins

- `grep -rn 'v0\.1\.9'` across all `*.toml` / `*.handl.toml` /
  `Cargo.lock` under `~/GitHub/handl-lang` (excluding `target/`):
  **zero hits**.
- `grep -rn 'tag =\|rev ='` on all `Cargo.toml`/`*.handl.toml`: only
  unrelated `handl-plugin-lib-tag`/`handl-lib-tag` entries and
  `branch = "main"` deps — **no tag/rev-pinned git deps anywhere in the
  Rust manifests**. Every handl git dep is `branch = "main"`.
- `ci.sh` files: no version/tag/rev pins.
- Python-side `pyproject.toml` pins confirmed all on `tag = "v0.1.10"`
  (eqsat, discoverykit, gamekit, graphopt, kernelopt, kit-carriers, … —
  spot-checked; the cascade is complete).

### 4. `graphopt/packages/mlopt-experiments` pins

- `Cargo.toml:29`: `mlopt-kit = { git = "https://github.com/handl-lang/graphopt", branch = "main" }` — still resolves; `packages/mlopt-kit` exists in-tree at graphopt HEAD `389e3f2` (== origin/main).
- `Cargo.toml:60` `[patch."https://github.com/handl-lang/graphopt"] mlopt-kit = { path = "../mlopt-kit" }` — in-repo checkout wins locally; standalone consumers fall back to git `main`. Correct per design.
- `.handl.toml:34`: same dep + `subdir = "packages/mlopt-kit"`; `[patch]` row `../mlopt-kit`. Resolves.
- `Cargo.lock`: `mlopt-kit` has **no** `source = "git"` line (path-patched, as designed); only three git sources remain — `discovery`, `egraph`, `host-tensor`, all `branch=main` HEADs recorded at last lock refresh. None stale.
- **Known D-2 caveat (unchanged, expected):** these still point at the
  `graphopt` repo/`packages/mlopt-kit` path and will need the `mlopt`
  rename when P3 D-2 lands. For now they resolve — verified.

## Housekeeping note (not a pin issue)

`graphopt/packages/mlopt-experiments/Cargo.lock` has **uncommitted local
modifications** — a lockfile refresh adding new transitive registry deps
(`ar_archive_writer`, `cc`, `find-msvc-tools`, `shlex`, …), presumably
from a `cargo metadata`/build during the cascade. Harmless; left
uncommitted for the parent to decide (commit as `chore: lockfile
refresh` or discard).

## Actions taken

- Read-only verification only: `git log/status/ls-remote`, `grep` sweeps,
  `Cargo.lock`/`Cargo.toml`/`.handl.toml` inspection.
- **No files edited, no commits, no pushes, no builds run.**
