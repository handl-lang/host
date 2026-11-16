# Repin cascade — eqsat v0.1.10 + mlopt-torch → graphopt v0.1.21

Date: 2026-… (audit-19 follow-up). Scope: eqsat `mlopt-core` /
`mlopt-orchestrator` / `mlopt-dtypes` `v0.1.9 → v0.1.10` across all
consumers, plus `mlopt-torch` `v0.1.16 → latest tagged`.

## Key mechanic discovered

`uv` **does** propagate `[tool.uv.sources]` from git dependencies —
including single-project repos (kit-carriers etc.), not just uv
workspace members. Every pinned tag's pyproject sources are honored
transitively, so a partial repin fails resolution with
"conflicting URLs for package `mlopt-core`" (`@v0.1.10` vs `@v0.1.9`).
The in-repo comments claiming "uv honours sources only at the
workspace root" are inaccurate for git deps.

Consequence: the cascade required **repin + retag** of every repo
whose pyproject carries eqsat sources, in dependency order, with
consumer pins bumped to the new tags.

## Per-repo results

### eqsat (source repo)
- **Skipped — nothing to do.** Workspace members use
  `workspace = true`; no git self-pins. Tag `v0.1.10` (`4e2dc877`)
  already existed and is what everyone now pins.

### Tags created (all pushed)
| repo | new tag | commit |
|---|---|---|
| gamekit | v0.1.12 | 194ac55 |
| discoverykit | v0.1.19, then v0.1.20 (gamekit pin bump) | 84bdb3e, 0e8a945 |
| kit-carriers | v0.1.3 | b4dc4b8 (lock backfill 0c52509, untagged — lock not propagated) |
| kit-cuda | v0.1.3 | 2be4a79 (lock backfill 7cd5441, untagged) |
| kit-discovery | v0.1.2 | fefc005 (lock backfill fbb61e7, untagged) |
| kit-onnx | v0.1.2 | 0a2bc81 (repin commit; tag on it) |
| kit-host-tensor | v0.1.3 | cb92773 |
| graphopt | v0.1.21 | 389e3f2 |

### Consumer repins (pyproject + uv.lock committed & pushed)
- **graphopt** `389e3f2` — eqsat ×3 → v0.1.10; gamekit/benchkit →
  v0.1.12; discoverykit → v0.1.20; kit-carriers → v0.1.3; kit-cuda →
  v0.1.3; kit-discovery → v0.1.2; kit-onnx → v0.1.2; kit-host-tensor →
  v0.1.3. `uv lock` clean. Tagged **v0.1.21**.
- **kernelopt** `177c0d9` — eqsat ×3 → v0.1.10; mlopt-torch →
  graphopt **v0.1.21**; kit-carriers → v0.1.3; gamekit/benchkit →
  v0.1.12; kit-discovery → v0.1.2; discoverykit → v0.1.20.
  `uv lock` clean.
- **mlopt** `24f16b5` — eqsat ×3 → v0.1.10; mlopt-torch → graphopt
  **v0.1.21**; kit-carriers → v0.1.3; kit-cuda → v0.1.2→v0.1.3;
  kit-discovery → v0.1.2; kit-host-tensor → v0.1.3; gamekit →
  v0.1.12; discoverykit → v0.1.20. `uv lock` clean. **Only
  `pyproject.toml` + `uv.lock` were staged** — the other agent's
  in-flight files (`scripts/train_delivery_prior.py`,
  `src/mlopt/deliver_train.py`, `tests/test_deliver_train.py`,
  untracked `arch-*.hnl/.safetensors`) were left untouched. **No
  `uv sync` was run** (training agent's venv is live).
- **kit-carriers** `b4dc4b8` + `0c52509` — eqsat ×3 → v0.1.10;
  dev-group mlopt-torch direct-URL → graphopt v0.1.21. Tagged v0.1.3.
- **kit-cuda** `2be4a79` + `7cd5441` — same shape. Tagged v0.1.3.
- **kit-discovery** `fefc005` + `fbb61e7` — eqsat ×3 → v0.1.10;
  gamekit → v0.1.12; discoverykit → v0.1.20; kit-carriers → v0.1.3;
  dev mlopt-torch URL → v0.1.21. Tagged v0.1.2.
- **kit-onnx** `0a2bc81` — eqsat ×2 → v0.1.10. Tagged v0.1.2.
- **kit-host-tensor** `cb92773` — eqsat ×1 → v0.1.10. Tagged v0.1.3.
- **gamekit** `194ac55` — root source mlopt-core → v0.1.10
  (covers `packages/benchkit` too — single lock). Tagged v0.1.12.
- **discoverykit** `84bdb3e` + `0e8a945` — eqsat ×2 → v0.1.10;
  gamekit source → v0.1.12. Tagged v0.1.19 then v0.1.20.

### Skipped / not applicable
- **kit-torch** — `git ls-remote --tags` shows **no tags at all**
  (only `main` @ e579e71). Per instructions: skipped. mlopt-torch
  remains the shipped package at graphopt `packages/mlopt-torch`,
  now at tag **v0.1.21** — this satisfies the "v0.1.16 → latest
  tagged" repin for all consumers (kernelopt, mlopt, and the
  dev-env direct-URLs in kit-carriers/kit-cuda/kit-discovery all now
  point at v0.1.21). Note v0.1.20 exists but still carries eqsat
  v0.1.9 sources, so v0.1.21 is the correct target.
- **host-tensor** — no Python eqsat deps (PyO3/Rust repo; pyproject
  has no mlopt pins). Nothing to do.
- **kit-kernel-ir, lib-kernel-ir, lib-triton-qgemv** — no eqsat /
  graphopt git pins. Nothing to do.
- All other `handl-lang/*` repos (the handl language ecosystem) —
  grep across every `pyproject.toml` confirms none reference
  eqsat.git / graphopt.git / the kit-* pins.

## Verification

- `uv lock` clean in all 10 touched repos (mlopt: lock only, no
  sync). Resolutions confirm every `mlopt-core`/`orchestrator`/
  `dtypes` now resolves to eqsat `4e2dc877` (v0.1.10) and
  `mlopt-torch` to graphopt `389e3f22` (v0.1.21).
- No `uv sync` / test runs performed (heavy; locks prove
  resolution only). Parent may want a smoke `uv sync` + import in
  non-mlopt repos later.
- graphopt worktree was clean throughout; no kit-torch Phase-2
  cascade edits were observed in the consumer pyprojects during
  this run — but that agent may still be mid-flight; if it edits
  the same pyprojects it should now rebase on the pushed commits.

## Loose ends for the parent / next agent

1. **kit-torch tagging** — when kit-torch cuts its first tag,
   consumers' `mlopt-torch` pins (kernelopt, mlopt) and the
   dev-group direct-URLs (kit-carriers, kit-cuda, kit-discovery)
   can move from `graphopt:packages/mlopt-torch@v0.1.21` to
   `kit-torch` — that's the Phase-2 cascade's job.
2. **Tag/lock skew** — kit-carriers v0.1.3, kit-cuda v0.1.3 and
   kit-discovery v0.1.2 tags point at commits *before* their
   `uv.lock` backfill commits. Harmless (uv.lock is not propagated
   to consumers) but noted for completeness.
3. **kernelopt** has no consumer pins pointing at it found; its
   repin is committed on main (tag v0.1.0 remains the only tag —
   cut v0.1.1 if anything starts pinning it).
4. mlopt's other in-flight files were deliberately not staged;
   `git status` there still shows the training agent's modifications
   plus untracked `arch-*.hnl`/`.safetensors` artifacts.
