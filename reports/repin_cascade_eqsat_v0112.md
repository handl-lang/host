# Repin cascade — eqsat v0.1.10 → v0.1.12

Date: follow-up to `repin_cascade_eqsat_v0110.md`. Scope: eqsat
`mlopt-core` / `mlopt-orchestrator` / `mlopt-dtypes`
`v0.1.10 → v0.1.12` across all consumers, plus the kit-torch
intermediate tag (v0.1.1 → **v0.1.2**, first tag carrying
v0.1.12 sources).

Targets: `v0.1.11` = `a23bcb3` (layout-law expansive budgets,
I-003 fix); `v0.1.12` = `d67d1fc` (Step E term repr B —
`mlopt_core.term` + `term_json`, `ir.py` shim; backward
compatible). eqsat HEAD == v0.1.12.

Same mechanic as the v0.1.10 cascade: uv propagates
`[tool.uv.sources]` from git deps transitively, so every repo
carrying eqsat sources needed repin + retag in dependency order,
with consumer pins bumped to the new tags.

## Per-repo results

### Done — repin + `uv lock` + commit + push + tag

| repo | commit | new tag | changes |
|---|---|---|---|
| kit-torch (`/home/luis/tmp/kit-torch-work` → github handl-lang/kit-torch) | `03207247` | **v0.1.2** | eqsat ×3 → v0.1.12. Clean tree, no unpushed work (only untracked `build/`). |
| gamekit | `00263df` | **v0.1.14** | root `mlopt-core` source → v0.1.12 (covers `packages/benchkit`, single lock). |
| discoverykit | `4464acf` | **v0.1.23** | eqsat ×2 → v0.1.12; gamekit → v0.1.14. |
| kit-onnx | `084897d` | **v0.1.3** | eqsat ×2 (core+dtypes) → v0.1.12. |
| kit-host-tensor | `1f5912a` | **v0.1.4** | eqsat ×1 (core) → v0.1.12. |
| kit-carriers | `34c1339` | **v0.1.5** | eqsat ×3 → v0.1.12; dev-group `kit-torch @ …@v0.1.1` → **v0.1.2**. |
| kit-cuda | `e361eb9` | **v0.1.5** | eqsat ×3 → v0.1.12; dev-group `kit-torch @ …@v0.1.1` → **v0.1.2**. |

All `uv lock` runs resolved cleanly to eqsat `d67d1fc8`
(v0.1.12); kit-torch resolved to `03207247` (v0.1.2). Only
`pyproject.toml` + `uv.lock` were committed everywhere. No
`uv sync` was run in any repo.

### Deferred / blocked

- **kit-discovery** — SKIPPED per the active-work rule: 3
  unpushed cascade commits on main (`a0ba534` D-1e rename,
  `ec3c8eb` ruff isort, `7d507c5` kit-torch v0.1.1 /
  kit-carriers v0.1.4 / gamekit v0.1.13 / discoverykit v0.1.22
  dep bump). Its pyproject still carries eqsat **v0.1.10**
  sources; remote tag v0.1.2 is the newest and also carries
  v0.1.10. It needs: push (or the owning agent finishes),
  eqsat ×3 → v0.1.12, and a new tag (v0.1.3) before any consumer
  can lock against it.
- **graphopt** — BLOCKED, verified empirically. Tree was clean
  and not ahead, and `packages/mlopt-torch` still exists (Phase-2
  D-1e removal hasn't landed here). I applied the full repin —
  eqsat ×3 → v0.1.12, gamekit/benchkit → v0.1.14,
  discoverykit → v0.1.23, kit-carriers → v0.1.5,
  kit-cuda → v0.1.5, kit-onnx → v0.1.3,
  kit-host-tensor → v0.1.4 — leaving `kit-discovery` at v0.1.2
  (no v0.1.12-carrying tag exists). `uv lock` failed exactly as
  the cascade mechanic predicts:

  ```
  × Failed to resolve dependencies for `kit-discovery` (v0.1.0)
  ╰─▶ Requirements contain conflicting URLs for package
      `discoverykit` … v0.1.20 vs v0.1.23
  ```

  (kit-discovery@v0.1.2's own sources pin discoverykit v0.1.20 /
  eqsat v0.1.10.) Changes **reverted**; worktree is clean. Once
  kit-discovery is repinned + tagged, graphopt needs the same
  edit plus `kit-discovery` → the new tag, then commit/lock/tag
  (next graphopt tag would be v0.1.22).
- **mlopt** — SKIPPED: ahead 1 (`9e2c53e` kit-torch dep+rename,
  unpushed) AND `pyproject.toml` dirty — Phase-2 cascade in
  flight. No `uv sync` anywhere near it.
- **kernelopt** — SKIPPED: ahead 1 (`5a1679b`, unpushed) AND
  `pyproject.toml` dirty.
- **kit-discovery consumers note** — kit-discovery's unpushed
  pyproject already pins kit-torch-era deps (kit-carriers
  v0.1.4, gamekit v0.1.13, discoverykit v0.1.22); when its owner
  repins eqsat they should also bump kit-carriers → v0.1.5,
  gamekit → v0.1.14, discoverykit → v0.1.23 (the tags I just
  pushed supersede the ones its lock was built against — those
  older tags still carry eqsat v0.1.10 sources and will conflict
  the same way).

### Not applicable

- **eqsat** — workspace members use `workspace = true`; no git
  self-pins. v0.1.11/v0.1.12 tags already exist and are pushed.
- **host-tensor** — PyO3/Rust repo, no Python eqsat deps.
- All other `handl-lang/*` repos — the earlier sweep
  (`grep 'eqsat|v0.1.10'` over every `pyproject.toml`) covered
  the full set; kit-torch's only checkout is
  `/home/luis/tmp/kit-torch-work` (done above).

## Verification

- `uv lock` clean in all 7 touched repos; resolutions confirm
  eqsat `d67d1fc8` everywhere and kit-torch `03207247`.
- graphopt repin attempt failed at `uv lock` (kit-discovery
  conflict) and was fully reverted — `git status` clean.
- No `uv sync` / test runs (locks prove resolution only).

## Loose ends for the parent / next agent

1. **Order to finish the fleet**: kit-discovery (owner agent:
   push pending work, repin eqsat → v0.1.12, retag v0.1.3, and
   refresh its intermediate pins to gamekit v0.1.14 /
   discoverykit v0.1.23 / kit-carriers v0.1.5) → then graphopt
   (edit listed above) → then mlopt + kernelopt (eqsat ×3 →
   v0.1.12, kit-torch → v0.1.2, kit-carriers → v0.1.5,
   kit-cuda → v0.1.5, kit-discovery → new tag, kit-onnx → v0.1.3,
   kit-host-tensor → v0.1.4, gamekit/benchkit → v0.1.14,
   discoverykit → v0.1.23 — exact pin set per each repo's
   pyproject).
2. **Tag/lock skew** — same as last cascade: tags point at the
   repin commits which include the lock, so no skew this time.
3. mlopt/kernelopt dirty pyprojects belong to the Phase-2
   kit-torch cascade agent — coordinate before touching.
