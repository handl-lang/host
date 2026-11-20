# Delivery prior v7 — training result: INFRA-BLOCKED (earlyoom ×2)

**Verdict: no numbers, no bake.** `src/mlopt/weights.py` stays
`DELIVERY_PLAYER_WEIGHTS = {}` — not because the trained arm lost
(v6's honest negative), but because the training process never
survived long enough to produce a held-out table.  Two attempts, two
earlyoom SIGTERMs on an 11.6 GB host shared with other agents'
pytest/coverage runs.  Per the run constraints ("if earlyoom kills
it twice, stop and report infra-blocked") the experiment stops here.

Artifacts (all under `/home/luis/tmp/delivery_v7/`, not the repo):

- `train_v7.log` — attempt A, killed 19:07:22 during held-out eval
- `train_v7b.log` — attempt B, killed 19:25:29 during eval-case lift
- `corpus_v7.json` — the corpus + persisted oracle answer key
  (see `delivery_v7_corpus.md`)
- No `train_result_v7.json` — the driver only writes `--json` at the
  end; both kills landed before that point.  **The driver has no
  checkpoint/resume** — `train()`'s learned weights die with the
  process.

## Protocol attempted (v6-replicated)

```
MLOPT_EQSAT_TIMEOUT=900 nice -n 10 .venv/bin/python \
  scripts/train_delivery_prior.py \
  --corpus /home/luis/tmp/delivery_v7/corpus_v7.json \
  --skip-name <unmeasurable cases> --eval-name <held-out> \
  --json /home/luis/tmp/delivery_v7/train_result_v7.json
```

Corpus mode reuses the persisted drains — no `oracle_drain` re-ran;
imitation cloned from the corpus answer key; REINFORCE episodes
measure through the real referee.  All other defaults
(`EPOCHS=8`, `IMITATE_EPOCHS=4`, `BUDGET=8`, `BENCH_ITERS=50`,
`SEED=0`).

Stack was ready for once: commit `095652e` fixed `build_case`'s
`mlopt_torch`→`kit_torch` import and the installed kit-cuda/kit-torch/
kit-carriers/eqsat cascade is unified — **no `run_census.py` alias
wrapper was needed** (the v7 corpus report's caveat 1 is resolved).

## Attempt A — eval = 5 heavy + 4 hnl (9 held-out)

Train 12 hnl cases; held-out `hnl-dispatch-many-l`,
`hnl-select-sort`, `hnl-gemm-bound`, `hnl-where-mix`,
`arch-whisper-small-encoder`, `whisper-turbo-decoder`,
`whisper-small-encoder`, `arch-whisper-tiny-full`,
`kokoro-textenc-p-t64-v1`; skipped `kokoro-textenc-t64` (corpus
`ok=False`, inductor `GuardOnDataDependentSymNode` build-fail).

All 21 lifts completed by 19:04 (HF cache warm).  `train()` ran
silently and finished; eval cases lifted; `held_out` began ~19:04.
**earlyoom SIGTERM at 19:07:22, pid 2645390, VmRSS 3.4 GB**
(journalctl `_COMM=earlyoom`).  Memory avail had been oscillating
0.9–3.6 GB all run; other agents' pytest-xdist workers were live.

## Attempt B — eval trimmed to 6 (4 hnl + 2 heavy)

To cut peak RSS: dropped `whisper-small-encoder`,
`arch-whisper-tiny-full`, `kokoro-textenc-p-t64-v1` from the run
entirely (`--skip-name`; all three are near-certain confirm-eager
negatives — lowest information value).  Held-out now
`hnl-dispatch-many-l`, `hnl-select-sort`, `hnl-gemm-bound`,
`hnl-where-mix`, `arch-whisper-small-encoder`,
`whisper-turbo-decoder`; train still the 12 hnl cases.

12 train lifts + silent `train()` completed; eval lifts reached
`whisper-turbo-decoder` (809M params) at ~19:25.  **earlyoom
SIGTERM at 19:25:29, pid 2673915, VmRSS 3.2 GB** — killed inside
the decoder's lift (post weight-load, during export/search), before
even the `n_enodes` line printed.  Swap was already at 0 free;
avail ~1.3–2 GB with other tenants resident.

Note: the pgrep-based liveness checks during attempt B self-matched
the polling command string; the process actually died at 19:25, not
19:44 — verified via log mtime and the earlyoom journal.

## Why this is infra-blocked, not a negative

- Both deaths are earlyoom SIGTERMs under host memory pressure
  (mem ≤10%, swap ≤15% of 4 GB), not measurement failures and not
  CUDA-context poisoning — no drain records, no cells, no rewards
  were produced or lost to poison; the `held_out` per-cell
  resilience machinery never got to run.
- The binding constraint is `whisper-turbo-decoder` (~809M params,
  fp32 module + export/search working set ≈ 3+ GB RSS alone) on a
  box with ~3 GB available while other agents run.  Even the
  *lift* of that one case OOM'd attempt B.
- Attempt A proved the 12-case hnl train stream + `train()` fits;
  the failure moved to the eval-case pin set (9 modules) and then
  to a single heavy lift.  Smaller splits won't fix it while the
  turbo decoder is in scope.

## What would unblock it (for the next attempt)

1. **Quieter box / off-peak window** — the census drains ran fine
   at 14:30–15:16 when the box was less contended.
2. **Drop `whisper-turbo-decoder` from held-out** (eval = 5 hnl +
   `arch-whisper-small-encoder`) — loses the only positive real
   case but should fit; held-out headroom then rests on the hnl
   stratum (dispatch-many-l 32.5×, select-sort 15.4×).
3. **Checkpointing** — `train_delivery_prior.py` has no
   resume; adding a `--resume`/weights-dump after `train()` would
   let a killed run keep its player (small patch, mirrors the
   `--json` tail).
4. Alternatively re-run only `held_out` if a weights literal ever
   lands — the arms table needs the trained player, so this doesn't
   help retroactively here.

## Decision

**Do not bake.**  `DELIVERY_PLAYER_WEIGHTS` remains `{}`; the
`linear*` arms stay honestly cold.  No commit — no source changes.
The v6 negative stands unrefuted and unconfirmed on v7 data; the
v7 corpus's larger oracle ratios (up to 32.5×) remain an untested
opportunity, gated on RAM, not on evidence.
