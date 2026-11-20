# joint-safe-preset merge prep — kit-torch

Date: 2026-… (session date) · Worktree: `/home/luis/tmp/kit-torch-joint` · Branch: `joint-safe-preset`

## Verdict: READY — no rebase or force-push was needed

- Branch head `fb44685ae71085c576399b222b7557a22a4b8e83` ("composer: joint-safe default — drop pointwise layout laws from cross_pairs")
  already sits **directly on top of** `origin/main` `03207247f8d7e9359b5d7738917a1b2bc38b83e7` (v0.1.2, eqsat v0.1.12 repin).
  `git merge-base --is-ancestor origin/main HEAD` → true. The repin commit touched only `pyproject.toml`/`uv.lock`,
  and the branch was evidently cut after it landed — the expected conflict surface never materialised.
- `git rebase origin/main` was therefore a no-op (skipped; nothing to replay).
- `origin/joint-safe-preset` == local `HEAD` (`fb44685a`) — remote already up to date, **no force-push required**.
  A `git push --force-with-lease origin joint-safe-preset` would be a no-op.
- Branch diff vs main: `src/kit_torch/composer.py` (+46/−7 region) and `tests/test_compositional_pairs.py` (+41) — 2 files, 80 insertions, 7 deletions.

## Verification

`uv sync` — clean (51 resolved / 48 audited, no changes).

Tests (worktree `.venv`, Python 3.13):

```
.venv/bin/python -m pytest tests/test_compositional_pairs.py tests/test_compositional.py tests/test_compositional_cache.py -q
54 passed in 98.28s
```

Lint (branch-touched files):

```
.venv/bin/ruff check src/kit_torch/composer.py tests/test_compositional_pairs.py
All checks passed!
.venv/bin/ruff format --check src/kit_torch/composer.py tests/test_compositional_pairs.py
composer.py formatted; test_compositional_pairs.py "would be reformatted"
```

Pre-existing issues (NOT introduced by this branch — verified against `origin/main` blobs):

- `tests/test_compositional_pairs.py` fails `ruff format --check` on main too (long `Optimizer(...).optimize(model, x, strategy=Compositional(), ...)` lines pre-date the branch). kit-torch's AGENTS verification gate is `ruff format --check src` only — tests are out of scope. No action needed for merge.
- `tests/test_compositional_cache.py:247` has a pre-existing `ruff check` F841 (`entry` assigned, unused) — file untouched by this branch.

## Merge commands (ready — DO NOT RUN yet; Phase-2 cascade agent still active on main)

Tag check: `git tag -l 'v0.1.*'` locally and `git ls-remote --tags origin` show only `v0.1.0`, `v0.1.1`, `v0.1.2` — **`v0.1.3` is free** (no local or remote tag).

```sh
cd /home/luis/GitHub/handl-lang/kit-torch   # or any checkout of main
git fetch origin
git checkout main
git pull --ff-only origin/main               # ensure main == 03207247 (or newer)
git merge --ff-only joint-safe-preset        # fb44685a is a direct child of 03207247 → ff-only succeeds
git tag v0.1.3                               # annotated or lightweight per repo convention
git push origin main v0.1.3
```

Caveat: if the Phase-2 cascade agent lands commits on main before this merge, `--ff-only` will fail — rebase `joint-safe-preset` onto the new main first (conflict surface: `composer.py`, `test_compositional_pairs.py`).

## Consumer repinning

- `handl-lang/kit-carriers` `pyproject.toml:24` — dev-dep `kit-torch @ git+https://github.com/handl-lang/kit-torch.git@v0.1.2` (uv.lock pins `03207247`).
- `handl-lang/kit-cuda` `pyproject.toml:28` — same `@v0.1.2` pin (uv.lock same commit).
- The preset change is **additive perf-only** (default `cross_pairs` drops the pointwise layout family; explicit `rules=RuleSet` still opts into the full WITH_LAYOUT − SUBSUMED surface). No API break, extraction byte-identical on the six specs per the commit message.
- **Repin not required for correctness** — consumers stay valid on v0.1.2. They only pick up the perf fix if bumped to `@v0.1.3`. Recommend tagging v0.1.3 and letting consumers repin opportunistically (or in the Phase-2 cascade's own repin pass) rather than forcing a coordinated bump.
