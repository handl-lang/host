---
title: Audit pass 20 — kit-cuda v0.1.2 UAF fix + mlopt poisoned-context skip verified (CUDA unit test passes live; mlopt .venv still runs the buggy 0.1.0 build — env drift, not repo defect); kit-torch D-1e repo landed clean at e579e714, untagged, zero consumers; delivery_v6 corpus 19/21 oracle-positive confirmed against persisted drains (all picks verified, t5 backed by safe-drain summary only); llama22 v3 peak 5.29 GB exit=143 consistent with ADR-0179 memory claims but verify still incomplete; carry-forwards: eqsat repin in flight in graphopt worktree, mlopt-torch pins still v0.1.16
type: report
date: 2026-11-16
---

# Audit pass 20

Scope: commits/artifacts since pass 19 — kit-cuda `6813e3b`+`a917f7e`
(tag `v0.1.2`), mlopt `d4d4d9e`+`31b1fdc` (+ in-flight uncommitted
`deliver_train` oracle-reuse work), graphopt `d4e2ea5` (D-1f split,
tag `v0.1.20`), kit-torch repo `653586a0`+`e579e714` (D-1e Phase 1),
the `/home/luis/tmp/delivery_v6/` corpus + `delivery_v6_corpus.md`,
and the `/home/luis/tmp/llama22_v*.rss` verify-run series.  Pin/tag
sweep re-run across all consumers.

**WIP present and not disturbed:** mlopt worktree carries uncommitted
`deliver_train.py`/`train_delivery_prior.py`/`test_deliver_train.py`
changes (persisted-drain reuse for `train()` — the corpus_v6 answer
key path) plus untracked `arch-{gpt2,t5}-small.{hnl,safetensors}`;
graphopt worktree carries an uncommitted eqsat `v0.1.9→v0.1.10`
pyproject bump (P19-N1 repin in flight, `uv.lock` not yet touched);
kit-torch worktree clean except a stray `.coverage.a16x.*` artifact.

## Per-area verdicts

### 1. kit-cuda v0.1.2 + mlopt poison-skip — **PASS** (one env caveat)

**Recapture correctness (`kit_cuda/runners.py`, `6813e3b`):**

- `_GraphReplayModule.__init__` stores `warmup`/`alias_pred`/
  `mutable_pred` from the `capture()` kwargs and `_recapture()`
  forwards all three to `type(self).capture(...)` — warmup, alias and
  mutable predicates survive recapture correctly.
- `_params_stable()` compares `storage_fingerprint(inner)` element-wise
  with `is` against the capture-time `_param_snap`.  Verified live on
  this box (torch 2.15.0+cu132): `t.untyped_storage()` returns the
  *same* Python wrapper object across calls, so `is`-identity is a
  sound storage-identity check — and the snapshot tuple holds the
  storages alive, so no free-then-realloc ABA.  Same mechanism
  `_DeviceGraphModule._params_match` already shipped (Phase B).
- `_eager_forever` cannot replay stale: `forward` order is
  `_matches → _eager_forever → _params_stable/recapture → copy+replay`;
  a failed recapture sets `_eager_forever` and every later call serves
  `inner(*xs)` — no retry storm, no stale replay.  On successful
  recapture all four graph fields (`_graph`/`_graph_ins`/`_graph_out`/
  `_alias`) plus `_param_snap` are replaced atomically before the
  copy/replay continues with the *new* buffers.
- `_recapture` captures against the caller's `xs`, which already
  passed `_matches` (shape/dtype/device vs the static buffers) — valid
  capture inputs.  `storage_fingerprint` covers
  `parameters()`+`buffers()`+`IRModule._param_map`/`_param_values`
  (`mlopt_torch/device.py:132-160`) — the tables `_align_to`/`_park_params`
  actually rebind.
- **Test evidence:** ran the new tests on this CUDA-capable box —
  `pytest tests/test_runners.py -k "recapture or fallback or
  params_stable or eager_forever"` → **6 passed**, including the
  `requires_cuda`-gated `test_graph_replay_recaptures_on_param_rebind`
  (real capture + rebind + recapture) and a917f7e's arity-mismatch /
  failed-recapture fallback test.  CPU mocks cover the failure paths;
  the CUDA path is genuinely exercised.

**mlopt side (`d4d4d9e`, `31b1fdc`):**

- `_is_context_poison` matches only `device-side assert` /
  `illegal memory access` / `sticky error` (lowercased substring) —
  deliberately narrow; OOM and ordinary lower failures don't trigger.
  Set on poison-signature exceptions from both `lower()` and the bench;
  every later config returns `{"config", "reason": "skipped: CUDA
  context poisoned by ...", "skipped": True}` *before* `_runner_for` —
  the skipped config never reaches `lower()`.  The poisoning config
  itself keeps its honest `lower failed` decline record.
- **JSON safety of persisted drains:** `_drain_records`
  (`deliver_train.py:154-168`) curates to `_DRAIN_KEYS`, which does
  **not** include `skipped` — the persisted record keeps `config` +
  `reason` (a plain str containing the `skipped:` prefix), so drains
  stay JSON-safe but the boolean flag is dropped; downstream can only
  distinguish a skip by parsing the reason string (P20-N3).
- `pytest tests/test_deliver_game.py -k "poison or context"` →
  **3 passed** (marker tests + skip propagation).
- Pin bump is consistent: `pyproject.toml` `kit-cuda tag = "v0.1.2"`,
  `uv.lock` locks `tag=v0.1.2#a917f7e…`; remote tag `v0.1.2` is an
  annotated tag → `a917f7e` = remote HEAD.  ✓

**Caveat — env drift (P20-N1):** mlopt's `.venv` still has
`kit-cuda 0.1.0` installed (site-packages copy dated 05:26, predates
the fix; installed `runners.py` contains zero `_params_stable`
references — grep-verified).  The pin is correct; the environment was
never re-synced, so any GPU deliver run from this venv still executes
the UAF build until `uv sync`.

### 2. kit-torch repo (D-1e Phase 1) — **PASS** (untagged, gates unverified)

- `handl-lang/kit-torch` exists; `origin/main = e579e714` (pushed,
  worktree `mlopt-torch-split` in sync).  Two commits on top of the
  graphopt history: `653586a0` subtree split, `e579e714` follow-up
  (`inv` bound natively in `torch_bindings` — fixes a real
  binding-coverage failure when `kit_carriers` is absent; carrier-
  optional `importorskip` guards; `torch==2.14.*` constraint matching
  graphopt's lock).
- **Rename is clean:** `pyproject.toml` `name = "kit-torch"`,
  `root_packages = ["kit_torch"]`, coverage `source = ["kit_torch"]`;
  `src/kit_torch/` holds 31 modules, `tests/` 68 files.  Tree-wide
  grep: **zero** `import mlopt_torch`/`from mlopt_torch` and zero
  `mlopt-torch` dep references; the single `mlopt_torch` string left
  is `export.py:264`'s deliberate dual-spelling module-prefix tuple
  for pre-split grafted-executor artifacts — documented, correct.
- **No tags** on the remote (`git ls-remote --tags` → 0 refs) and
  **no consumer** references `kit-torch`/`kit_torch` in any repo's
  pyproject/sources (grep across all 11 repos) — Phase-2 repin pending
  by design.  graphopt still ships `packages/mlopt-torch` as a
  workspace member at `v0.1.20`.
- **Gates status: unknown.**  Coverage artifacts exist
  (`.coverage` 12:42, `.coverage.a16x.*` 13:22 — a coverage run
  happened, consistent with a pytest-3080 session at 13:25 whose
  test names match this suite), but no persisted pass/fail record and
  `fail_under = 0` — the floor was never set before the Phase-1 agent
  stopped.  e579e714's message indicates the binding-coverage contract
  was being actively fixed; treat gates as *probably run, unverified*.

### 3. Delivery v6 corpus — **PASS** (19/21 confirmed; two honesty notes)

- `corpus_v6.json` holds **21 cases**; I re-derived the positives from
  the persisted `drain` records: **19 `oracle_pick != None`, every one
  `verified=True` with `med_ms < eager_med_ms`** — speedups range
  1.03× (arch-whisper-base-encoder) to 16.3× (hnl-dispatch-many);
  the report's headline numbers (broadcast-mix 15.6×, dispatch-many
  16.3×, whisper-decoder alias wins) all match the records.  The 2
  negatives (`whisper-tiny-encoder`, `kokoro-textenc-p-t128-v3`) have
  `pick=None` with all-≈eager records — honest.
- **t5:** `oracle_pick=graph` is backed by `t5_safe_drain.
  records_summary` — `graph verified=true, med_ms=5.551 vs
  eager_med_ms=6.193` (~11%), matching the report.  The poisoned
  first drain is *also* persisted verbatim (all five configs'
  `lower failed: device-side assert` records) — good audit trail.
- **Crash attribution:** consistent with the kit-cuda UAF, not a
  region_fuse miscompile — (a) `fuse` *alone* verified clean in
  isolation, so the fused module's numerics are correct eagerly (a
  miscompile would fail the verify gate or produce wrong values, not
  a scatter/gather index-OOB device assert); (b) the assert signature
  (index OOB in `ScatterGatherKernel.cu`) is exactly what replaying
  garbage weights produces; (c) the fix commit states it was measured
  on this case; (d) the CUDA recapture unit test passes.  **Not**
  re-verified end-to-end: no post-fix `fuse+graph` t5 rerun exists,
  and the mlopt venv still runs kit-cuda 0.1.0 anyway (P20-N1).
  Note the UAF is *intermittent*: the 12:51–13:25 `train_v6.log` run
  (old 0.1.0 build) still landed `fuse+graph` picks on
  arch-bert/whisper-tiny-decoder/whisper-tiny-encoder — the bug is
  storage-layout/timing-dependent, which is why it presented as a
  single-case crash (P20-N9).
- **Persistence gap (P20-N4):** every other case persists full
  per-config drain records; t5's *winning* evidence is only
  `records_summary` (min/med/verified scalars, no `rounds` pairs) —
  the paired-round audit trail the v6 protocol exists for is absent
  exactly on the case that needed the workaround.

### 4. llama22 partial run — **PASS** (memory claim consistent; verify incomplete)

- `/home/luis/tmp/llama22_v3.rss`: `peak_rss_kb=5292064` (≈5.05 GiB /
  5.29 GB), `exit=143` = SIGTERM (external timeout kill — **not** 137
  SIGKILL/OOM).  Series: v1 3.05 GB → v2 4.29 GB → v3 5.29 GB, all
  143; `llama22_v3.err` empty, `llama22_hnl_out.json` 0 bytes — the
  verify did not finish.
- ADR-0179's memory arithmetic holds: `llama22_e2e.safetensors` is
  **4,399,842,720 B ≈ 4.4 GB** on disk (F32 sidecar), and a 5.29 GB
  peak is exactly "4.4 GB live weights + working set/transient" —
  versus the pre-ADR ~8.8 GB live set that was memory-walled.  The
  implementation is landed (kernel `9d9e875` `TensorData` F64/F32 +
  `__tensor_bin__`; host-tensor `3af18c0` codec + payload-as-wire
  `tensor.load`; graphopt `069d7db` hnl-run materialize egress;
  ext-eval `eadddba` — pass 19).  **Honest caveat:** "feasible" is
  still unproven end-to-end — three attempts all hit the time limit,
  so the wall has moved from memory to runtime, not disappeared.

### 5. Carry-forward status

- **eqsat v0.1.10 repin (P19-N1):** in flight — graphopt worktree has
  an uncommitted `pyproject.toml` bump of all three eqsat deps to
  `v0.1.10` (`uv.lock` untouched).  Every committed consumer, including
  the brand-new kit-torch, still pins `v0.1.9` → the 468-vs-524 corpus
  skew persists fleet-wide.
- **mlopt-torch consumer pins (P19-N3):** still `v0.1.16` in mlopt and
  kernelopt; graphopt's latest tag is now `v0.1.20` (= `d4e2ea5`, the
  D-1f split — pushed, verified on remote) and kit-torch sits untagged
  at `e579e714` with zero consumers.  The `3e724a8` dtype_ctx fix plus
  D-1d/D-1f/D-1e deltas remain unpinned.
- **Step E / D-2:** still pending — planning docs exist
  (`step_e_ir_census.md` 12:27, `step_e_term_repr_b_plan.md`,
  `d2_merge_prep.md` 12:24) but no landing commits in any repo.
- **P19-N4/N5/N6** (paired-round shuffle/iters/rounds-record gaps):
  `deliver.py`/`deliver_game.py` timing code untouched by `d4d4d9e`
  (which only added the poison skip) → all three carry unchanged.
- **P19-N7** (stale v5 `DELIVERY_PLAYER_WEIGHTS`): `weights.py`
  unchanged → open; the in-flight `deliver_train` oracle-reuse +
  `train_delivery_prior.py --corpus` work is clearly the v6 retrain
  path.
- **P19-N8** (kit-onnx untagged chore): HEAD still `1aa20df`,
  unchanged → carries.
- mlopt `stash@{0}` (old WIP on `32d1819`) still present — noted, not
  audited.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P20-N1 | MEDIUM | mlopt `.venv` (not a repo defect) | **Stale installed dep:** `kit-cuda 0.1.0` still installed though `pyproject.toml`/`uv.lock` pin `v0.1.2`. The UAF fix is committed but not live in the dev env — the 12:51 `train_v6` run and any GPU deliver work execute the buggy build until `uv sync`. |
| P20-N2 | LOW | graphopt `pyproject.toml:146` | graphopt still pins `kit-cuda v0.1.0` — the only remaining consumer on the pre-fix build; its cross-stack tests import `kit_cuda` through this dep. mlopt took `v0.1.2`; graphopt should follow. |
| P20-N3 | LOW | `deliver_train.py:154-168` `_DRAIN_KEYS` | The new `skipped: True` flag is not in the persisted key set — skip records survive only as `config` + `reason` string. JSON-safe, but consumers can't structurally distinguish a context-poison skip from an ordinary decline without parsing the reason prefix. |
| P20-N4 | LOW | `corpus_v6.json` t5 case | The oracle pick's evidence is `records_summary` scalars only — no persisted `rounds` pairs for the safe-order drain, breaking the per-case audit-trail uniformity the v6 protocol was built for. |
| P20-N5 | INFO | `corpus_v6.json` picks | Several verified positives are thin: arch-whisper-base/tiny-encoder `fuse+graph` ≈1.03×, kokoro-textenc-p-t16 1.05×, whisper-base-decoder 1.07× — real by the gate but noise-fragile (the report itself flags the ~2.5%-over-margin ones). |
| P20-N6 | INFO | kit-torch repo | No tags, `fail_under = 0`, gates unverified, zero consumers — Phase-1 landed but Phase-2 (consumer repin + floor) is entirely pending. |
| P20-N7 | INFO | mlopt worktree | Uncommitted `deliver_train` oracle-reuse (`oracle_picks`/`oracle_drains` kwargs), `train_delivery_prior.py` corpus-manifest mode, +36 test lines, and 4 untracked arch artifacts — coherent in-flight v6-retrain work, not audited for correctness. |
| P20-N8 | INFO | graphopt worktree | Uncommitted eqsat `v0.1.9→v0.1.10` pyproject bump (P19-N1 repin starting; `uv.lock` pending). |
| P20-N9 | INFO | kit-cuda UAF blast radius | The bug is intermittent — `train_v6` under the buggy 0.1.0 build still landed `fuse+graph` picks on 3 arch cases. The fix is correct, but "t5 fuse+graph crash" was one manifestation of a probabilistic storage-reuse race; other configs/models could have hit it silently had their drains not poisoned loudly. |

No HIGH findings; one MEDIUM (environmental, one-command fix:
`uv sync` in mlopt).  All pass-19 carry-forwards either landed
(graphopt `v0.1.20`, kit-cuda `v0.1.2`, kit-torch repo) or remain
honestly open (eqsat repin in flight, mlopt-torch pins, Step E, D-2,
stale weights, the three INFO protocol notes).

**Verdict: PASS** — every committed change in scope audits clean and
the headline claims check out against persisted evidence.  The two
things to act on: `uv sync` mlopt before the next GPU run (P20-N1),
and propagate `kit-cuda v0.1.2` to graphopt's pin (P20-N2).
