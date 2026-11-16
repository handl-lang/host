# Delivery-prior autopsy — measurement problem or reality problem?

**Verdict: BOTH, but predominantly a REALITY problem in the current
case corpus — with a genuine MEASUREMENT problem that guarantees the
game cannot learn the small wins that do exist.**

Three training waves (v3/v4/v5) left `DELIVERY_PLAYER_WEIGHTS = {}`.
The machinery works (oracle argmin, verify-gated scoring,
imitate+REINFORCE all function — `deliver_train.py`, `deliver_game.py`).
The signal isn't there *in this corpus*, and the referee couldn't
resolve it anyway.

## 1. What the cases look like, and what the oracle found

v5 corpus (`/home/luis/delivery_v5_train.log`, picks in
`/home/luis/delivery_prior_v5_train.json`): 9 cases —
whisper-tiny decoder-step, whisper-base/small encoders, kokoro f0n,
kokoro textenc-p, arch bert-minilm-l6, arch t5-small,
arch whisper-tiny-encoder, arch whisper-tiny-full.  All `device=cuda`,
fp32/fp16-class workloads on a shared RTX-2050-class 11 GB box.

Oracle picks (one scratch drain per case, `deliver_train.oracle_pick`,
`deliver_train.py:139-163`): **only 2/9 positive** —
kokoro `f0n` → `graph`, `arch:bert-minilm-l6` → `compile`.  The other
7 drain to `None` (eager argmin past `WIN_MARGIN=1.02`).

Eligible config pool per case is small: `graph-alias`/`fuse+graph-alias`
require `meta["alias_inputs"|"mutable_inputs"]`, and **no adapter
declares either** (`grep alias_inputs src/mlopt` → only
`deliver.py`/`deliver_game.py` readers).  So pending is at most
`{fuse+graph, graph, fuse, compile+graph, compile}` and typically 3–5.

Prior measurement reports explain *why* the negatives are real, not
noise (`graphopt/docs/`):

- **Whisper encoders are GEMM-bound**: delivered IRModule ~59 ms,
  `delivered+CudaGraphRunner` ~59–63 ms — "no gain"
  (`whisper_encoder_bench.md:57,65`).  Launch overhead is hidden
  under ms-scale GEMMs; no runner config can pay >2%.
- **Whisper decode steps are kernel-bound**: CUDA-graph replay ≈ plain
  module (5.23 vs 5.37 ms); the *copy-path* capture is actively worse
  (6.08–6.52 ms — 0.88 ms of input copies, `decode_step_fusion.md` §1–3).
- **region_fuse has a measured 1.43× win** (kokoro decoder 116.8→81.4
  ms, `kokoro_region_fuse.md`) **but fails the verify gate**:
  fused-vs-delivered `max_rel ≈ 1.4e-3` vs stock `rtol=1e-4` → the
  config declines under `verify=True` and can never score.  The
  stochastic kokoro arms can't verify at all.
- **Where wins DO exist** (all previously measured): dispatch-bound
  modules — carrier fp32 3.73→1.15 ms under graph
  (`results_wholemodel_gap.md` re-run table), fused32 0.951→0.454 ms;
  pointwise-tail fusion — compile recovers ~9% (fp64) / ~35% (fp32)
  on MLP-chain tails; kokoro f0n→graph (v5 oracle).  These are exactly
  the two v5 picks.

## 2. The noise floor vs the reward signal

- `_bench` (`deliver.py:249-293`): warmup=3, then `bench_iters`=50
  **contiguous** timed calls → `(min, median)`.  `BENCH_ITERS=50`
  (`deliver_train.py:75`).
- Each config is measured **once**, in one contiguous block, and eager
  is measured **once at board construction** (`deliver_game.py:247-254`)
  *before* any config runs.  There is no interleaving, no repeat
  rounds, no re-measurement of eager.  On a contended shared GPU,
  slow drift across the episode systematically biases every config
  measured after eager.
- Observed noise is worse than the "~3%" headline: the same probe
  reports "absolute numbers swing ±30% run to run" on dispatch-bound
  arms (`decode_step_fusion.md:13-15`), and an uncaptured fp32 carrier
  drifted 2.917→3.726 ms (**+28%**) between runs
  (`results_wholemodel_gap.md:198-200`).  Dispatch-bound cases — the
  ones most likely to have real wins — are exactly the noisiest.
- Direct evidence of reward noise: held-out evals produced
  `playbook=1.082` (`delivery_v5_eval_c7.log`) and
  `shipped-greedy=1.031/1.034` (`_c7s1`, `_c1`) that **failed to
  reproduce across seeds** — all other arms 1.000 everywhere.
- `WIN_MARGIN=1.02` (`deliver_game.py:80`) sits *below* the noise
  floor, so sub-margin real wins score 1.0 — conservative and honest
  for shipping, but it flattens the reward landscape: on 7/9 cases
  every action returns ~1.0 and REINFORCE has no gradient.  The v5
  "oracle answer key" itself is a single noisy drain cached per case
  (`deliver_train.py:209-215`) — half the imitation labels are
  stop-bias manufactured by noise, not by measurement.

## 3. Where wins plausibly exist — and where they can't

| family | mechanism that could pay | verdict |
|---|---|---|
| GEMM-bound encoders/large decoders (whisper enc, t5, bert-large) | none — kernels dominate, launch overhead hidden | **no headroom** (measured: graph ≈ eager) |
| Decode-step workloads w/ persistent K/V (whisper decoder, kokoro decoder-nc) | `graph-alias` (−0.9 ms/tok measured, `decode_step_fusion.md` §3) | **headroom exists but unreachable**: no adapter declares `alias_inputs`/`mutable_inputs` in `meta`, so `graph-alias` is never eligible (`deliver_game.py:421-436`) |
| Small launch-bound modules (kokoro f0n, small MLPs, many-small-op hnl terms) | `graph` copy-path, `compile` | **real wins** (f0n→graph in v5; carrier 3.2× in probes) — but smallest absolute ms → noisiest |
| Pointwise-tail / transcendental-heavy IRModules | `compile`, `region_fuse` | compile pays ~9–35%; region_fuse pays 1.33–1.43× **but fails rtol=1e-4 verify** → declined |
| Stochastic arms (kokoro decoder/decoder-nc/full) | — | can't pass `verify=True`; only `--no-verify` scores them, unverified |

## 4. v6 design (concrete)

**Case selection — pre-screen before paying for a drain.**
The cheapest predictor of "can any config win" is already on the
board state: `eager_med_ms`, `n_enodes`, `n_params`, `fusible`.  Build
cases with `bench_iters=10` census first (`census_cases_v5.py`
pattern — it already prints `pending`/`fusible`/`eager_med_ms`), keep
only cases where a config *can* plausibly clear margin:

- `eager_med_ms < ~20 ms` (launch-bound regime) OR `fusible=True`
  with ≥2 eligible regions OR a declared alias spec;
- explicitly include `hnl:` refs with synthetic leaves
  (`adapters/hnl.py` — arbitrary op mixes, seeded shapes) to mint
  dispatch-bound and elementwise-chain cases the HF adapters can't
  produce; vary `kokoro text_len`/`voice_idx` for shape diversity;
  add small `arch:` specs (vit/gpt2/t5 variants) beyond the 3 used;
  **declare `alias_inputs`/`mutable_inputs` in the whisper decoder
  adapter meta** (positions are known: `meta["input_layout"]` —
  ck/cv constant → alias, sk/sv write-back → mutable) so the
  measured −0.9 ms/tok win becomes a legal move.

Target ≥50% oracle-positive rate; measure it in the census pass
*before* training — v5's 2/9 is a corpus defect, not a training defect.

**Timing protocol.**
- Interleaved paired rounds: per config, alternate eager/config
  measurement (e.g. 5 rounds × 20 iters, order randomized per round),
  score on the per-round **min** aggregated across rounds — min is the
  robust statistic under contention spikes; the current
  median-of-50-contiguous-iters doesn't defend against drift and eager
  is never re-measured.
- Re-measure eager inside each evaluation round (or at least once
  mid-episode) so drift can't systematically favor late configs.
- Raise `bench_iters` for sub-5 ms cases — bench cost is trivial
  there; the expensive part is lower+verify, not timing.
- Persist every oracle drain: per-config `min_ms/med_ms/verified/
  reason` to JSON alongside `oracle_picks` — currently the answer key
  is a bare name and the measurements are discarded (this alone
  would have made the v5 autopsy trivial).
- Fix `ctx.lowered_by_config` unbounded retention (`deliver_game.py:536`)
  and consider caching lowered modules per (case, config) across
  epochs — the v5 report's recommendation 3; lower+verify dominates
  episode cost.

**Reward shaping.**
- Keep the committed-speedup reward but add a small per-move cost
  (`reward = committed_speedup − λ·steps`, λ≈0.005): today a player
  that drains all 5 configs scores identically to one that stops
  immediately — the prior can't learn *efficient* stopping, only
  argmax.
- Train imitation only on oracle-positive cases (or weight them);
  the 7 eager-argmin cases teach a degenerate all-stop policy —
  keep them as a held-out "eager-is-correct" stratum instead of
  training mass.
- Alternative worth pricing first: **distill the playbook** — the
  playbook already encodes CONFIGS preference order + early stop; a
  prior that only learns stop-timing may not beat it.  The learnable
  residual is config-kind × shape-feature → win-probability, which
  requires the positive-case corpus above.

**Honest cheaper win available now:** the corpus *does* contain
valuable negative data — 7 cases where eager is genuinely optimal
under every legal config is exactly the "confirm eager" signal the
product ships.  A v6 that reports "prior = playbook-equivalent on
GEMM-bound cases, graph on launch-bound cases" is a real artifact
even if weights stay small; what's missing is corpus diversity, not
machinery.

## 5. Bottom line

- **Reality problem (primary):** in the v5 corpus, verified faster
  configs exist on only 2/9 cases, and prior probe reports show the
  other 7 are eager-argmin *by construction* (GEMM-bound or
  verify-blocked).  No timing fix creates headroom that isn't there.
- **Measurement problem (secondary but blocking):** ~3–28% bench noise
  vs `WIN_MARGIN=1.02`, single-shot eager baseline, no interleaving,
  and a noisy cached oracle label mean even the wins that exist
  (f0n→graph, bert→compile, and the unreachable graph-alias decode
  win) can't be learned reliably — v4's cold-RL curve
  `[1.034, 1.061, 1.0, 1.023, 1.018, 1.037]` vs train `[1.0×6]`
  shows the noise floor masquerading as signal.
- Cheapest real fix order: (1) alias-spec meta on whisper decoder +
  hnl:/small-arch dispatch-bound cases → raise oracle-positive rate;
  (2) paired interleaved timing + persisted drain records;
  (3) per-move-cost reward + positive-case-weighted imitation;
  (4) only then decide whether RL beats distilled-playbook.
