# Delivery prior v6 — training result: NEGATIVE, weights stay cold

**Verdict: do not bake.** `src/mlopt/weights.py` stays
`DELIVERY_PLAYER_WEIGHTS = {}`.  Two independent corpus splits both
show the trained arm at or below the cold/shipped baselines on
held-out cases — run A slightly below, run B far below.  The v6
protocol machinery (persisted oracle key, corpus mode, poison-
resilient held-out) is committed and works; the *prior* it produced
does not generalise.

Artifacts (all under `/home/luis/tmp/delivery_v6/`, not the repo):

- `train_v6b.log` — run A, first attempt: **earlyoom SIGKILL** during
  `train()` (system-wide mem avail hit 0.9%; 11.6 GB host)
- `train_v6c.log` + `train_result_v6.json` — run A, completed
- `train_v6d.log` — run B, first attempt: **earlyoom SIGKILL** again
  (same signature, mid-`train()`)
- `train_v6e.log` + `train_result_v6b.json` — run B, completed
- `corpus_v6.json` — the census corpus + persisted oracle answer key
  (see `delivery_v6_corpus.md`)

Code: mlopt commit `8fceab1` — `--corpus`/`--eval-name`/`--skip-name`
corpus mode, `train(oracle_picks=, oracle_drains=)` persisted-key path,
per-cell poison-resilient `held_out`.

## Protocol actually run

```
.venv/bin/python scripts/train_delivery_prior.py \
  --corpus /home/luis/tmp/delivery_v6/corpus_v6.json \
  --skip-name arch-t5-small-cuda \
  --eval-name <5 or 7 names> \
  --json <out>.json        # MLOPT_EQSAT_TIMEOUT=900, all other defaults
```

- `arch-t5-small-cuda` **skipped deliberately**: its `fuse+graph`
  config triggers a scatter/gather device-side assert that poisons
  the process-wide CUDA context (census §bugs).  Live RL/eval
  episodes could retrigger it and silently corrupt every later
  measurement — the persisted label is safe, the live case is not.
- The persisted drains made the oracle pass **free**: no
  `oracle_drain` re-ran; imitation cloned 112/112 (run A) and 60/60
  (run B) demonstrated moves from the corpus answer key.  REINFORCE
  episodes still measured every config through the real referee.
- Split sizes are 15/5 and 13/7, not the nominal ~16/5: **RAM, not
  statistics, set the ceiling.**  Every `DeliveryCase` pins a full
  lifted module; the five heaviest cases (gpt2, whisper-base-encoder,
  kokoro-t128, both whisper decoders) had to live in held-out so the
  15-case train stream stayed under earlyoom.  Run B traded two hnl
  cases into held-out (13 train) for stratum coverage.

## Run A — held-out = real models only (5 cases)

Train 15 (8 hnl + 3 arch + 2 kokoro + 2 whisper-tiny), eval
`arch-gpt2-small`, `arch-whisper-base-encoder`,
`kokoro-textenc-p-t128-v3`, `whisper-base-decoder`,
`whisper-small-decoder`.  Imitation 112/112; train block means
`[2.252, 3.253, 2.871, 2.196, 3.982, 2.193]`.

| arm | reward (5/5 cells) |
|---|---|
| trained | 0.977 |
| trained-greedy | 0.996 |
| shipped (=cold, weights={}) | 1.006 |
| shipped-greedy | **1.014** |
| cold | 0.988 |
| playbook | 0.994 |
| random | 0.989 |

Context that makes this less surprising than it looks: the oracle
*ceiling* on these five cases is only ~1.07 mean reward
(`eager_med/config_med − 2·MOVE_COST`; per-case ceilings 1.223 /
1.017 / 0.990 / 1.060 / 1.075 from the persisted drains).  Real-model
delivery headroom after `MOVE_COST=0.005`/step is thin — even perfect
play nets ~1.07.  The trained arm didn't even reach that.

## Run B — held-out adds the hnl stratum (7 cases)

Same recipe; eval adds `hnl-conv-tail-cuda` +
`hnl-matmul-chain-cuda` (train drops to 13).  Imitation 60/60; train
block means `[1.0, 1.0, 1.032, 1.017, 1.004, 1.227, 1.0]` — markedly
lower than run A's, itself a signal the RL stream wasn't finding the
big hnl wins consistently.

| arm | reward (7/7 cells) |
|---|---|
| trained | 0.999 |
| trained-greedy | 1.022 |
| shipped (=cold) | 1.242 |
| shipped-greedy | 1.304 |
| cold | 1.257 |
| **playbook** | **1.333** |
| random | 1.015 |

The hnl cases restored real headroom (baselines jump to ~1.3) — and
the trained arm scored ~1.0, i.e. it learned to *stop*, not to pick.
Its baked table is near-zero (`|w| ≤ 0.31`, e.g.
`a:kind:graph-alias 0.3092`, `a:kind:graph -0.3237`) — imitation
cloned the expert lines but the REINFORCE stream never grew the
weights into a policy that beats a cold linear arm exploring
freely.

## Why the negative is believable, not a fluke

- Two disjoint splits, same direction; run B's gap is large
  (trained-greedy 1.022 vs playbook 1.333).
- Run A's first attempt died to a **real** CUDA-context poison in
  held-out eval (illegal memory access at the next case's eager
  bench) — the per-cell `held_out` added in `8fceab1` is what let
  run A/B complete at all; before it, one bad config zeroed the
  whole table.
- The corpus's big oracle wins are concentrated in synthetic hnl
  micro-cases (2–15×); on real whisper/arch/kokoro arms the wins are
  1.0–1.23× raw and ~1.0 after move costs.  A prior trained mostly
  on the synthetic stratum has little real-model signal to learn,
  and what it learns (stop-early conservatism) doesn't transfer.

## Follow-ups worth capturing (not done here)

- **myco issue**: `fuse+graph` on `arch:t5-small` (region_fuse →
  cuda_graph lower) device-side assert — already flagged in
  `delivery_v6_corpus.md`; the drain-order corruption it caused is
  now guarded by `deliver_game._is_context_poison`, but the fusion
  bug itself is unfixed.
- The same assert class recurred **nondeterministically** in held-out
  eval on the first run-A attempt (illegal memory access, different
  case set) — worth an issue: which config/case, reproducibility.
- Corpus v7 should weight real-model cases heavier (the hnl stratum
  is cheap to generate but teaches the wrong prior) and/or price
  `MOVE_COST` into case selection — cases whose oracle reward ceiling
  is < ~1.05 are untrainable mass.
- `earlyoom` on this 11.6 GB host is the binding constraint on
  corpus size; a streamed/subset training mode (lift → train slice →
  free → next) would allow the full 21-case corpus.
