---
title: Audit pass 21 — Step E plan B landed clean (eqsat v0.1.12 term/term_json split, ir.py shim verified name-complete); I-003 layout-law expansive tags consistent across kit-eqsat/engine-eqsat/eqsat with parity tests run; kit-torch cascade advanced (v0.1.0+v0.1.1 pushed, eqsat-pin conflict resolved) but mlopt venv carries a file:// kit-torch install and a lockfile with zero kit-torch entries — env drift again; D-2 B1 blocker resolved, lint debt still open on mlopt main; llama22 still time-walled at ~5.3 GB peak
type: report
date: 2026-11-16
---

# Audit pass 21

Scope: commits/artifacts since pass 20 — eqsat `a23bcb3`+`d67d1fc`
(tags `v0.1.11`, `v0.1.12`), kit-eqsat `b2eb3d8`, engine-eqsat
`818c9ed`, kit-torch `653586a0`→`fa646941` (tags `v0.1.0`/`v0.1.1`),
mlopt `8fceab1`+`37f5902`+`9e2c53e` (+ uncommitted repin), kernelopt
`5a1679b` (+ uncommitted repin), `host/project/reports/d2_dryrun.md`
+ `layout_law_budgets_fix.md`, `/home/luis/tmp/delivery_v7/`, and the
`llama22_r10–r12` verify attempts.

**WIP present and not disturbed:** mlopt worktree has uncommitted
`pyproject.toml` (kit-torch `v0.1.0→v0.1.1` + kit-carriers/cuda/gamekit/
discoverykit/discoverykit bumps) and `scripts/train_delivery_prior.py`
(`mlopt_torch`→`kit_torch` import fix), plus the 4 untracked arch
artifacts; kernelopt worktree has the same uncommitted repin pattern;
graphopt clean (stash `stash@{0}` still present, unchanged).

## Per-area verdicts

### 1. Step E plan B — eqsat `d67d1fc` / tag `v0.1.12` — **PASS**

**Shim completeness (the 757-import-site risk):** `ir.py` does **not**
use `from mlopt_core.term import *` — it re-exports explicitly, which
sidesteps the `__all__`/underscore trap entirely. Verified two ways:

- Extracted every top-level name defined by `v0.1.10:ir.py` (27 names
  incl. privates) and probed all of them as attributes of the new
  `mlopt_core.ir` in the eqsat venv: **zero missing** —
  `_OP_REGISTRY`, `_OP_REPR_BUDGET`, `GenDef`, `_attr_key`,
  `_dag_postorder`, `_dag_shared`, `_hashable_attr`, `_sexp`,
  `_tree_sizes` all re-exported via `X as X` (ir.py:24-47).
- `term.py`'s `__all__` is the old `ir.py` public surface **minus**
  `term_to_data`/`term_from_data` — and those two stayed in `ir.py`
  itself (the lemma-store codec, `ir.py:265` total, `__all__` still
  lists them). Exactly the split the commit message claims.
- `eqsat.py`'s private codec quartet now delegates to
  `mlopt_core.term_json` (`term_to_json`/`terms_to_json`/
  `term_from_json`, `leaf="const-str"` profile preserving the legacy
  `__mlopt_leaf__:` wire spelling; decode accepts both spellings).
  `term_json.py` is a clean 682-line module with its own `__all__`.

**Consumer spot-checks:** `kit_discovery.hnl_codec` imports clean in
the kit-discovery venv (mlopt-core `v0.1.10` installed — the shim is
compat, and consumers don't see v0.1.12 until repin anyway);
`mlopt.emit` imports clean in the mlopt venv. All committed consumers
pin `v0.1.10` — verified across mlopt, kit-discovery, graphopt,
kernelopt, kit-onnx, kit-carriers, kit-cuda pyprojects.

Minor: the task brief said "1885 green"; the commit message says
**1883 fast-lane tests pass + 2 pre-existing kit-eqsat drift
failures** (verified on clean HEAD per the message). Not re-run.

### 2. I-003 fix — expansive tags on 77 layout laws — **PASS**

Tag consistency verified independently at all three layers:

- **kit-eqsat `b2eb3d8`** (pushed, remote main = `b2eb3d8`): parsed
  `data/laws/layout.json` — **77/77 records carry
  `tags=["expansive","layout"]`**, nothing else changed (+77 lines).
- **engine-eqsat `818c9ed`** (pushed, remote main = `818c9ed`):
  `builtin.rs:8322` `layout()` maps `.with_tag("expansive")` over the
  pack once — the per-constructor `with_tag("layout")` sites stay
  readable. `tests/contracts.rs:668` does compare kit records to
  `builtin::layout()` **verbatim including tags**.
- **eqsat `a23bcb3` = tag `v0.1.11`** (pushed): `laws/layout.py:66`
  `R` helper default `(LAYOUT,) → (LAYOUT, EXPANSIVE)`; regenerated
  `eqsat_laws.json` = 524 records, all 77 layout records
  `(expansive, layout)` — consistent with the kit pack.

**Was the parity test run?** Yes — `layout_law_budgets_fix.md`
"Verification run" records: kit-eqsat `cargo test --workspace` pass
(incl. corpus_drift), engine-eqsat `cargo test` pass ("contracts test
needed the builtin.rs tag parity commit; confirmed failing before,
green after"), eqsat `test_law_bank_corpus_parity.py` +
`test_eqsat_laws_corpus.py` 8 passed, plus a 354-test layout/law slice.
Wire-level A/B on the captured joint spec: **82–132 s → 10.1 s**,
byte-identical extracted term/cost/fired set — the fix is sound, not
just fast.

**Caveat (by design, noted in the fix report itself):** LAW_FORMAT=2
records carry no budget field — `rule_budgets` is derived
Python-side from `rules.tagged(EXPANSIVE)` (`optimize.py:906-913`).
All consumers still pin eqsat `v0.1.10`, so the Python half is inert
until the pending repin past `a23bcb3`. The corpus tag alone does
nothing.

### 3. kit-torch cascade — **PASS** (incomplete by design; env drift found)

- **kit-torch repo** (`/home/luis/tmp/kit-torch-work`, remote
  `handl-lang/kit-torch`): tags **`v0.1.0`→`f0c1bb3a`, `v0.1.1`→
  `fa646941` both pushed**. `fa646941` repinned its own eqsat sources
  `v0.1.9→v0.1.10` — **the D-2 B1 URL-conflict half is resolved**:
  `git show v0.1.1:pyproject.toml` confirms eqsat `v0.1.10` pins
  matching graphopt/mlopt.
- **Consumers:** mlopt `9e2c53e` committed `kit-torch v0.1.0`; the
  worktree carries an **uncommitted** bump to `v0.1.1` +
  kit-carriers `v0.1.4`, kit-cuda `v0.1.4`, gamekit/benchkit
  `v0.1.13`, kit-discovery `v0.1.3`, discoverykit `v0.1.22`,
  kit-host-tensor `v0.1.3`. kernelopt `5a1679b` committed the rename;
  same uncommitted `v0.1.1`+bumps pattern in its worktree.
- **graphopt** still ships `packages/mlopt-torch` as a workspace
  member at HEAD `e4c639b` — cascade incomplete, status noted not
  flagged.
- **NEW env drift (P21-N1, MEDIUM):** mlopt's `.venv` has
  `kit_torch-0.1.0.dist-info` installed from
  `file:///home/luis/tmp/kit-torch-work` (a local-path install, the
  same trick the v7 census used) — **not** the git `v0.1.1` pin — and
  `uv.lock` contains **zero `kit-torch` entries** (stale; `uv sync`
  will fail freshness until re-locked). Stale `mlopt_torch
  0.1.0.dev0` is also still installed beside it. Same class as
  P20-N1: pins correct-ish, environment behind.

### 4. D-2 dry-run — **GO confirmed; B1 resolved; two items still open**

- `d2_dryrun.md` verdict GO-with-conditions stands; the hard blocker
  (kit-torch untagged + eqsat `v0.1.9` vs `v0.1.10` URL conflict) is
  **now resolved** by `v0.1.1`→`fa646941` (§3). Remaining pre-merge
  items per the report: regenerate `uv.lock` against the real pin
  (never commit the dry-run's `file://` lock), mlopt lint-debt fixes,
  arch-safetensors decision.
- **Lint debt still open (P21-N2, LOW):** `ruff check src scripts`
  in mlopt → **6 errors**: the 5 dry-run B4 items persist
  (`whisper_batch_bench.py` F401+E501×2,
  `train_delivery_prior.py`/`optimize.py`/`stage.py` I001) **plus the
  rename added 3 new I001 unsorted-import errors** (`kit_torch`
  import placement in `optimize.py:162`, `stage.py:393`,
  `train_delivery_prior.py:159`). mlopt's own `ci.sh py` stage is
  still red; the fixes exist only in the dry-run commit `aff8902`.
- **arch safetensors (P21-N3, LOW):** `arch-gpt2-small.safetensors`
  (652 MB) + `arch-t5-small.safetensors` (616 MB) remain **untracked
  and NOT gitignored** (`git check-ignore` fails; no `.gitignore`
  entry). The dry-run's recommendation to gitignore
  `arch-*.safetensors` has not been applied — one stray `git add -A`
  commits 1.27 GB permanently.

### 5. Carry-forwards

- **P20-N3 — FIXED:** `37f5902` adds `"skipped"` to `_DRAIN_KEYS`
  (`deliver_train.py`) — the flag now survives into persisted drains.
- **I-002/I-003 filed:** both exist as myco issues
  (`host/project/issues/issue-0002-*.md` CUDA-context-poison,
  `issue-0003-*.md` joint-pass perf). I-003's fix landed (§2);
  I-002 stays open — v7's stack notes confirm kit-cuda was bumped
  venv-only to `v0.1.3` for the census.
- **eqsat repin pending:** every committed consumer pins `v0.1.10`;
  `v0.1.11` (I-003 tags) and `v0.1.12` (term repr B) are unpinned
  fleet-wide. The I-003 fix is inert until this lands.
- **llama22 still unverified:** three new attempts `r10`–`r12`
  (15:58–16:20) — all `exit=143` (SIGTERM timeout), `wall_s` 318–322,
  `peak_rss_kb` ≈5.33–5.39 GB, empty `.json` outputs. Memory wall
  holds (~5.3 GB ≈ 4.4 GB weights + working set, consistent with
  ADR-0179); the wall has moved to **runtime**, not disappeared.
- **delivery v7:** `corpus_v7.json` = 22 cases / **15 oracle-positive**
  (71%; 88% of the hnl stratum, up to 32× eager/best). Report
  `delivery_v7_corpus.md` is honest about stack skew: census ran with
  a `sys.modules` alias unifying `mlopt_torch`/`kit_torch` and a
  venv-only kit-cuda `v0.1.3`/eqsat `v0.1.10` bump. **Training is
  blocked on the kit-torch rename cascade** — `train_delivery_prior.
  py` still imported `mlopt_torch` until the uncommitted worktree fix.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P21-N1 | MEDIUM | mlopt `.venv` (env, not repo) | `kit_torch 0.1.0` installed from `file:///home/luis/tmp/kit-torch-work`; `uv.lock` has **zero** kit-torch entries (stale lock — `uv sync` fails freshness); stale `mlopt_torch 0.1.0.dev0` coexists. Same drift class as P20-N1 — fix: commit the repin, `uv lock && uv sync`. |
| P21-N2 | LOW | mlopt `src`/`scripts` | 6 ruff errors on main+worktree: 5 pre-existing dry-run B4 items **plus 3 new I001** from the kit_torch rename (`optimize.py:162`, `stage.py:393`, `train_delivery_prior.py:159`). ci.sh py stage still red; fixes exist only in dry-run `aff8902`. |
| P21-N3 | LOW | mlopt worktree | `arch-*.safetensors` (1.27 GB total) untracked and not gitignored — dry-run recommendation unapplied; one `git add -A` commits it permanently. |
| P21-N4 | LOW | fleet-wide | eqsat `v0.1.11`/`v0.1.12` unpinned — I-003 expansive-tag fix and term-repr-B shim both inert for consumers until repin (all at `v0.1.10`). mlopt/kernelopt `v0.1.1` kit-torch bumps also uncommitted. |
| P21-N5 | INFO | llama22 | r10–r12 all exit=143 at ~320 s wall, ~5.3 GB peak — runtime wall persists; verify still incomplete. |
| P21-N6 | INFO | kit-torch/graphopt | Cascade mid-flight: kit-torch tagged through `v0.1.1`, graphopt still ships `packages/mlopt-torch`; consumer pins converge on `v0.1.1` but uncommitted. |
| P21-N7 | INFO | eqsat commit msg | "1883 fast-lane + 2 pre-existing kit-eqsat drift failures" — the 2 failures are kit-eqsat-side drift, worth a look when the repin lands (they may be the tag-parity tests this pass just fixed). |

No HIGH findings. One MEDIUM (environmental — `uv lock && uv sync`
in mlopt after committing the in-flight repin). Every claimed landing
verified against the repos: the ir.py shim is name-complete (checked
against the full v0.1.10 top-level name list, not just `__all__`),
the three-way expansive-tag fix is consistent and its parity tests
are recorded green, kit-torch `v0.1.1` resolves the D-2 blocker, and
the honest-status items (llama22, v7 corpus, graphopt tail) all check
out as reported.

**Verdict: PASS** — act on: commit mlopt/kernelopt repins + `uv
lock`/`uv sync` (P21-N1/N4), apply the mlopt lint fixes + safetensors
gitignore before D-2 executes (P21-N2/N3).
