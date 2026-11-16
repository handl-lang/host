# Delivery prior — v3/v4/v5 negative results

**Verdict: `DELIVERY_PLAYER_WEIGHTS` stays `{}`.** Three training waves
produced trained priors at or below the cold/playbook arms on held-out
cases. This is a measured negative, not a bug — the machinery works
(oracle argmin, verify-gated scoring, imitation+RL all function); the
signal isn't there.

## What was tried

| wave | corpus | result |
|---|---|---|
| v3 | small torch/kokoro cases | negative |
| v4 | + `arch:` sources (tiny encoder) | negative; eqsat 300s ceiling found → `MLOPT_EQSAT_TIMEOUT` added |
| v5 | 9 cases (whisper encoders/decoders, kokoro f0n/textenc-p, arch bert/t5/whisper) | negative; see below |

## v5 findings (the informative one)

- **Only 2/9 feasible cases have any winning config** (kokoro f0n→`graph`,
  t5-small→`compile`); whisper encoders and decoder-step workloads are
  eager-argmin by construction → imitation collapses to stop-bias.
- **Bench noise ~3%** on the contended 11GB host exceeds the reward
  differences the prior trains on; the one apparent win (playbook 1.082)
  failed to reproduce across seeds.
- `WIN_MARGIN=1.02` sits *below* the noise floor — margin-gated rewards
  can't distinguish signal from noise on this hardware.

## Real bugs found and fixed along the way

- `st:n_enodes` feature was permanently 0.0 (`SearchResult` has no
  `n_enodes`; now counts Op nodes on the `res.term` DAG) — mlopt `da1dc25`.
- `MLOPT_EQSAT_TIMEOUT` didn't reach the trainer's `build_case` — fixed
  same commit.
- `onnx_rust` safetensors loader `read_bytes` → `np.memmap` (multi-GB
  payloads) — same commit.

## v6+ recommendations (from the v5 agent)

1. Train on **win-positive cases only** (f0n/t5-like), or
2. **Distill the playbook** directly instead of RL on noisy rewards, or
3. Fix `lowered_by_config` unbounded retention (memory hazard) + per-config
   bench caching before attempting bigger cases.

Artifacts: `/home/luis/delivery_prior_v{3,4,5}_train.json`,
`/home/luis/delivery_v5_*.log`, eval drivers `/home/luis/*v5*.py`.
