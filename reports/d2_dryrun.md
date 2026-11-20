# D-2 dry-run report — mlopt → packages/mlopt merge rehearsal

**Verdict: GO with conditions.** The §2 merge sequence works end-to-end;
`.venv/bin/mlopt` materializes (D8 = YES). But the rehearsal surfaced
**one hard blocker** (kit-torch has no pushed tag and its eqsat pins
conflict with graphopt's) plus a set of mechanical gate fixes that must
land in the real merge commit. Dry-run commit: `aff8902` on detached
HEAD in `/home/luis/tmp/graphopt-d2-dryrun` (base `e4c639b`, one commit
ahead of the runbook's `af46e8f` anchor).

Snapshot notes:
- mlopt source copied from `/home/luis/GitHub/handl-lang/mlopt` working
  tree at HEAD `9e2c53e` ("chore: mlopt-torch -> kit-torch dep +
  rename (P3 Step D-1e)") **including the untracked `arch-*` artifacts**
  — rsync copies untracked files, which is what D4 wants.
- mlopt `uv.lock` is **stale**: it predates `9e2c53e` (contains no
  `kit-torch` entry at all). mlopt's own `uv sync` would fail
  lock-freshness today — the kit-torch cascade has not re-locked mlopt.
- rsync excluded `.git .venv uv.lock Cargo.lock .gitignore target
  __pycache__` — **added `target/` and `__pycache__` beyond the
  runbook's list** (101 MB of cargo artifacts; root `.gitignore`
  covers the pycache that pytest regenerates).

---

## 1. What worked as written

- `parents[N]` fixes: grep-verified counts matched the runbook exactly
  (emit.py `parents[2]` ×1; onnx_rust.py comment+code ×1;
  test_onnx_rust_source.py / test_arch_rust_source.py `parents[2]` ×1
  each). `arch_rust.py` needed no edit, confirmed.
- `_hnl_run_candidates` rewrite applied verbatim; `_REPO` now resolves
  to the dry-run root (`parents[4]` verified at runtime) and
  `find_hnl_run()` honours `MLOPT_HNL_RUN`.
- Root `pyproject.toml`: `mlopt-meta` rename, description rewrite,
  `requires-python = ">=3.13"`, `"mlopt"` added to dependencies,
  `mlopt = { workspace = true }` + `kit-torch` source added,
  `hf`/`kokoro` extras re-homed — `uv lock` + `uv sync` both succeed.
- Member `packages/mlopt/pyproject.toml`: kept `[tool.ruff]`
  (line-length 100 — hierarchical override works: member files are
  checked with member rules, root's line-length-72/D-rules do NOT leak
  in), kept pytest `testpaths`, hatchling wheel config; **removed the
  member `[tool.uv.sources]` table** (dead config — uv only reads
  sources at the workspace root).
- Docs: `packages/mlopt/AGENTS.md` + `README.md` re-pointed;
  graphopt `AGENTS.md` façade paragraph rewritten (the "`import mlopt`
  fails" claim is now false — `packages/mlopt` IS the mlopt package).
- `packages/mlopt/ci.sh` re-pointed to `../..` and stage paths updated.
- `packages/mlopt/bupha.toml` rewritten (name/dir assume post-rename;
  `cmd = "bash packages/mlopt/ci.sh"`).

## 2. Breakages found + fixes applied (all committed in aff8902)

### B1 — HARD BLOCKER: kit-torch pin unresolvable + eqsat URL conflict

mlopt@`9e2c53e` pins `kit-torch = { git = …/kit-torch.git, tag = "v0.1.0" }`.
`git ls-remote --tags` on handl-lang/kit-torch returns **zero tags** —
the pin fails for mlopt itself too (its uv.lock is stale, see above).
Worse: even pinned by `rev`, `uv lock` fails:

```
× Failed to resolve dependencies for `kit-torch` (v0.1.0)
╰─▶ Requirements contain conflicting URLs for package `mlopt-core`:
    - git+…/eqsat.git@v0.1.10#subdirectory=packages/mlopt-core   (our root)
    - git+…/eqsat.git@v0.1.9#subdirectory=packages/mlopt-core    (kit-torch's own [tool.uv.sources])
```

uv **does** honour `[tool.uv.sources]` inside git/path dependencies'
pyprojects — kit-torch@`e579e714` (origin/main) pins eqsat **v0.1.9** in
its own sources table, which collides with graphopt's v0.1.10 pins.

**Dry-run workaround:** copied `/home/luis/tmp/kit-torch-work` →
`/home/luis/tmp/kit-torch-dryrun`, stripped its `[tool.uv.sources]`
eqsat block, and bound `kit-torch = { path = "/home/luis/tmp/
kit-torch-dryrun" }` (path-dep sources are ignored → our v0.1.10 table
wins). Resolution then succeeded.

**Required before real execution** (pick one):
- kit-torch cascade pushes tag `v0.1.0` **and** bumps its eqsat pins to
  v0.1.10 (or graphopt drops to v0.1.9 — check mlopt-torch/graphopt
  members actually need v0.1.10), OR
- accept a temporary `rev`/`path` pin like the dry-run's and regenerate
  `uv.lock` — but **never commit a `file://` path into the real
  pyproject/lock**; the dry-run's `uv.lock` contains
  `kit-torch @ file:///home/luis/tmp/kit-torch-dryrun` and must be
  re-locked after re-pinning.

### B2 — ty version fork (0.0.84 vs 0.0.104)

Root dev dep `ty>=0.0.84` resolved **0.0.84**; mlopt's own venv runs
**0.0.104**. Under 0.0.84, mlopt's `# ty: ignore[missing-attribute]` /
`[import-not-found]` comments are *unknown rules* → 14 diagnostics
(kokoro adapter monkeypatch attrs, `kokoro.model` source-tree import,
transformers imports before extras sync, plus 4 pre-existing mlopt
errors that exist on mlopt main too). Fix: `uv lock
--upgrade-package ty` → 0.0.104; the ignore comments then resolve and
kokoro/whisper go clean. **Real run: bump the lock (or pin
`ty>=0.0.104` in root dev deps) — mlopt code assumes 0.0.104 rule
names.**

### B3 — New ty diagnostics fixed by annotation (runbook: annotate, don't exclude)

- `packages/mlopt-torch/.../torch_export.py:126,136` — ty 0.0.104 flags
  `**({"device": device} if device else {})` unpacking against
  `Op.make(validate: bool)` (dict-unpack checked against every param).
  Fixed by hoisting `dev_kw: dict[str, Any]` — clean, no ignore.
- `packages/mlopt/tests/test_emit.py:381,395,974` — deliberate
  type violations (string `Const` exemplars, `TimeoutExpired(timeout=
  None)` fake) → `# ty: ignore[invalid-argument-type]` with reasons.
- `packages/mlopt/scripts/whisper_batch_bench.py:338` — `m["notes"]`
  not-iterable after ty narrowed the dict literal → `m: dict[str,
  Any]` annotation.
- `packages/mlopt-torch/.../contraction_policy.py:67` —
  `ty: ignore[unresolved-import]` → `[import-not-found]` (0.0.104
  rename; was a `renamed-rule-in-ignore-comment` warning).
- Remaining: 1 benign `deprecated` warning (contextmanager overload,
  pre-existing). `error-on-warning = false` → `ty check` exits 0.

### B4 — Pre-existing mlopt lint debt (exists on mlopt main @9e2c53e)

`ruff check` on the member found 5 errors **already present upstream**
(the kit-torch rename left unsorted imports + dead import + 2 long
lines in `scripts/whisper_batch_bench.py`). Fixed in dry-run via
`ruff check --fix` + `ruff format` — real mlopt needs the same before
or during merge (its own `ci.sh py` stage is currently red).

### B5 — radon ratchet (expected fail → baseline regen)

`scripts/radon_ratchet.py` failed with **40 new over-threshold
functions** (all `packages/mlopt/**`). `--update` regenerated
`scripts/complexity_baseline.json` (819-line diff) → ratchet green.
This baseline regen is part of the merge commit.

### B6 — vulture

42 findings, all in `packages/mlopt`: unused `game_env` pytest fixture
params (~35), dead stores `want_max/want_all/want_std` (emit.py),
`query_length` (whisper.py), one literal `if False` ternary
(whisper_batch_bench.py:360 — simplified to the live branch).
Fixed via `[tool.vulture] ignore_names` additions + the ternary
cleanup. Note: real graphopt main **already fails vulture** on 2
pre-existing `clean_registry` findings in
`tests/test_torch_backend_registry.py` — unrelated debt, not
merge-caused (verified against the real checkout).

### B7 — bandit

`bandit -c .bandit.yaml -r packages` went red on 9 mlopt findings
(mlopt's own ci.sh never ran bandit). Fixed with targeted `# nosec`
at the exact sites, all deliberate patterns:
- B615 ×3 — HF `from_pretrained(repo_id)` in whisper adapter (×2) +
  compile probe: repo-id refs are the product surface; no revision pin
  by design.
- B108 ×1 — `/tmp/kokoro-src` documented default in kokoro.py.
- B101 ×2 — asserts in whisper adapter / cli (invariants).
- B110 ×3 — best-effort `except: pass` cleanup paths.
Bandit now exits 0 (9 nosec skips, 0 issues). The `[tester] nosec … no
failed test` warnings bandit prints are cosmetic.

### B8 — semgrep / lint-imports / pytest

- `semgrep --config .semgrep.yml packages/mlopt`: 0 findings.
- `lint-imports`: both contracts KEPT — but note `root_packages` is
  still only `mlopt_torch`/`mlopt_dtypes`; the mlopt product's upward
  edges (it legitimately imports gamekit/discoverykit/kit_discovery)
  are unchecked. Open question whether mlopt joins any contract —
  it sits ABOVE the game layer, so it can't join the existing
  forbidden contract as written.
- `pytest packages/mlopt/tests -q`: **263 passed, 26 skipped** in ~10–15 s
  (skips are honest: no hnl-run/source-onnx siblings under
  `/home/luis/tmp`; one fewer skip than the pre-extras run because
  transformers is now installed).
- Root lane spot-check (`test_attrs`, `test_adapters`,
  `test_export.py`): 91 passed — workspace unaffected.
- `.venv/bin/mlopt models`: **works** — all 5 adapters (arch, hnl,
  kokoro, onnx-rust, whisper) report ready.

## 3. D8 answer — YES

`uv sync` at the merged root installs the `mlopt` member through the
`mlopt-meta` dist's workspace dep: `.venv/bin/mlopt` exists (325-byte
entry-point script) and `mlopt models|optimize|export|doctor|stage
--help` all work. No `--all-packages` needed. **Caveat:** this depended
on B1's workaround — with a proper git pin the same mechanism applies.

## 4. Disk-space warning (environmental, not a merge defect)

`/home` was at 100% (430 MB free) during the rehearsal; the dry-run
`.venv` is 6.0 GB and `packages/mlopt/arch-*.safetensors` is **1.2 GB**
of untracked binary. `uv sync` survived on hardlinks from the shared
uv cache, but real execution should ensure ≥10 GB free and should NOT
`git add` the safetensors (see §5/D4).

## 5. Left for the real execution (decisions still open)

- **D4**: `arch-*.{hnl,safetensors}` — I committed the two `.hnl`
  (2.9 MB total) and left the `.safetensors` (1.2 GB) **untracked**.
  Committing them would bloat git permanently; recommend
  `.gitignore`-ing `arch-*.safetensors` or storing them outside git.
- **D7**: I used rsync+squash (one commit `aff8902`, 58 files).
- **D10**: kept member-scoped `packages/mlopt/ci.sh` re-pointed at
  root; `gates.sh` untouched (member tests stay a separate lane per D6).
- **kit-prelude smoke** (`hnl mlopt models`) not run — no `hnl` binary
  in kit-prelude's target/debug on this box; the Python-side
  `MLOPT_PYTHON=$PWD/.venv/bin/python` contract is proven by
  `.venv/bin/mlopt` working.
- Full `pytest -m "not e2e and not slow"` root lane not run (hours);
  spot checks green. Run `MLOPT_GATES_LANE=dev scripts/gates.sh`
  before the real merge commit.
- graphopt `AGENTS.md`/`README` reference sweep beyond the façade
  paragraph not exhaustively done (e.g. `bench/` docs still say
  "graphopt" — cosmetic, post-rename sweep is §3.5 anyway).

## 6. Go/no-go

**GO** once: (a) kit-torch gets a pushed tag AND its eqsat pins are
aligned with the merged repo's choice (or a rev pin is agreed),
(b) `uv.lock` is regenerated against that pin (discard the dry-run
lock — it references `/home/luis/tmp`), (c) the arch-safetensors
destination is decided. Everything else in §2 is rehearsed and green.

Dry-run artifacts left in place: worktree
`/home/luis/tmp/graphopt-d2-dryrun` (commit `aff8902`, detached),
`/home/luis/tmp/kit-torch-dryrun` (kit-torch-work copy minus
`[tool.uv.sources]`). Nothing pushed; real repos untouched.
