# Delivery-prior v6 case corpus — census + oracle drains

**Result: 21/21 cases built, 19/21 oracle-positive (90%)** — vs v5's
2/9 (22%).  The autopsy §4 pre-screen worked: every kept case is
launch-bound (`eager_med_ms` mostly < 12 ms), fusible, or alias-declared.

Artifacts (all under `/home/luis/tmp/delivery_v6/`, not the repo):

- `cases.json` / `cases_cuda.json` / `cases_retry.json` /
  `cases_whisperarch.json` — candidate manifests
- `hnl/*.hnl` — 8 synthetic `.hnl` case sources (synthetic leaves via
  `leaves=` opt; no feed)
- `census.jsonl` + `census.log` — pass 1, CPU-resident (hnl/arch have
  no `device` load opt; pending={compile} or {fuse,compile})
- `census_cuda.jsonl` + `census_cuda.log` — pass 2, `--device cuda`
  input-placement override, `bench_iters=10`
- `drains.jsonl` + `drains.log` — full oracle drains,
  `bench_iters=50` (5 paired rounds × 10 iters/side), `--drain`
- `drains_retry.jsonl`, `drains_whisperarch.jsonl` — re-drains after a
  CUDA-context poisoning (below)
- `drains_t5_safe.json` — the t5 sidecar (poison-safe drain order)
- `corpus_v6.json` — consolidated corpus + per-config drain records
- `t5_isolate.py`, `t5_drain_safe.py` — the fuse+graph isolation drivers

Driver: `mlopt/scripts/census_cases.py` (new, committed) — manifest →
`build_case` → `board_of` census facts → optional `oracle_drain`;
appends JSONL per case so interrupted runs keep measurements.

## Corpus table (cuda drains, bench_iters=50)

| case | ref | eager_med_ms | pending | fusible | alias | oracle pick |
|---|---|---|---|---|---|---|
| hnl-elem-chain-s | hnl:elem_chain_s.hnl [128²] | 0.205 | 3 | – | – | compile+graph |
| hnl-elem-chain-m | hnl:elem_chain_m.hnl [256²] | 0.643 | 3 | – | – | graph |
| hnl-dispatch-many | hnl:dispatch_many.hnl [64²], 51 ops | 0.297 | 5 | ✓ | – | fuse+graph |
| hnl-pointwise-tail | hnl:pointwise_tail.hnl matmul+transcendental tail | 0.162 | 3 | – | – | compile+graph |
| hnl-matmul-chain | hnl:matmul_chain.hnl 5×matmul [128²] | 0.076 | 3 | – | – | compile+graph |
| hnl-attn-small | hnl:attn_small.hnl softmax+LN block | 0.094 | 3 | – | – | compile+graph |
| hnl-broadcast-mix | hnl:broadcast_mix.hnl [1024,1]×[1,1024] | 1.822 | 5 | ✓ | – | fuse+graph |
| hnl-conv-tail | hnl:conv_tail.hnl conv2d+pool+LN | 0.175 | 3 | – | – | graph |
| whisper-tiny-decoder | whisper:openai/whisper-tiny | 2.706 | 4 | – | ✓ | **graph-alias** |
| whisper-base-decoder | whisper:openai/whisper-base | 3.807 | 4 | – | ✓ | **graph-alias** |
| whisper-small-decoder | whisper:openai/whisper-small | 8.849 | 7 | ✓ | ✓ | **fuse+graph-alias** |
| whisper-tiny-encoder | whisper:openai/whisper-tiny arm=encoder | 29.818 | 3 | – | – | None (eager) |
| kokoro-f0n | kokoro arm=f0n | 7.656 | 5 | ✓ | – | graph |
| kokoro-textenc-p-t16 | kokoro arm=textenc-p text_len=16 | 2.309 | 3 | – | – | compile |
| kokoro-textenc-p-t128-v3 | kokoro arm=textenc-p text_len=128 voice_idx=3 | 5.553 | 3 | – | – | None (eager) |
| arch-bert-minilm-l6 | arch:bert-minilm-l6 | 3.028 | 5 | ✓ | – | fuse+graph |
| arch-gpt2-small | arch:gpt2-small | 11.456 | 5 | ✓ | – | fuse+graph |
| arch-t5-small | arch:t5-small | 6.228 | 5 | ✓ | – | graph* |
| arch-whisper-tiny-decoder | arch:whisper-tiny-decoder | 5.258 | 5 | ✓ | – | compile+graph |
| arch-whisper-tiny-encoder | arch:whisper-tiny-encoder | 34.535 | 5 | ✓ | – | fuse+graph |
| arch-whisper-base-encoder | arch:whisper-base-encoder | 71.320 | 5 | ✓ | – | fuse+graph |

\* t5's `oracle_drain` is poisoned by a real bug — see below; the pick
comes from the safe-order re-drain (`graph`, med 5.55 vs paired eager
~6.19 ms, ~11%).

## What the drains show

- **The whisper-decoder alias win is real and now legal.**  bd386b7's
  `alias_inputs`/`mutable_inputs` meta makes `graph-alias` /
  `fuse+graph-alias` eligible: tiny 2.82→2.08 ms, base 3.79→3.54 ms,
  small 8.90→8.20 ms (all verified).  Exactly the −0.9 ms/tok-class
  win the autopsy predicted, now reachable.
- **hnl: cases are the strongest positive stratum** — all 8 pick a
  runner config, mostly `graph`/`compile+graph`/`fuse+graph` at
  2–15× eager.  `fuse+graph` on broadcast-mix: 1.82→0.117 ms (15×).
- **arch: specs on cuda are far faster than the v5 CPU numbers**
  (gpt2-small 176→11.5 ms eager, t5 87→6.2 ms, bert 26.8→3.0 ms) —
  the device-placement override matters; v5's arch cases were
  CPU-resident (`pending=2`, no graph configs eligible).
- **Negatives are honest**: whisper-tiny *encoder* (29.8 ms,
  GEMM-bound — every config within noise of eager) and
  kokoro-textenc-p-t128-v3 (all configs ≈ eager).  These are the
  "confirm eager" stratum the autopsy wanted kept, not trained mass.

## Bugs / caveats found (honest reporting)

1. **`fuse+graph` crashes arch:t5-small on cuda** —
   `RegionFusionRunner → CudaGraphRunner` lower dies with a
   scatter/gather device-side assert (index OOB,
   `ScatterGatherKernel.cu:257`).  Isolated per-config: `compile`,
   `fuse`, `graph`, `compile+graph` all pass; only `fuse+graph`
   crashes.  Because the assert poisons the CUDA context, the stock
   `oracle_drain` order (`fuse+graph` first) then fails every
   subsequent config — t5's first two drains record all-`lower
   failed` and a bogus `pick=None`.  Re-drained with `fuse+graph`
   stepped last → `pick=graph`.  **This is a real fusion+capture bug
   worth a myco issue** — and a demonstration that drain order can
   silently corrupt an oracle label.
2. **Verify declines on fused kokoro/arch paths**: kokoro-f0n
   `fuse+graph`+`compile+graph` and arch-whisper-tiny-decoder
   `fuse+graph` all `VerifyDeclinedError` — the region-fuse
   rtol=1e-4 problem from the autopsy, now visible per-config in the
   persisted records.
3. **Thin-margin positives**: arch-whisper-tiny/base-encoder
   `fuse+graph` picks clear WIN_MARGIN by only ~2.5% — real by the
   gate but noise-fragile; treat as marginal, not strong positives.
4. **hnl/arch device placement is a post-build input move**, not an
   adapter feature — `torch_eval` moves params to the input device
   per call, so it works, but it is not the shipped `device=` opt
   path (hnl/arch adapters don't have one).  If the corpus graduates
   to `train_delivery_prior.py --ref`, either add a `device` opt to
   those adapters or keep the `--device` override convention.
5. **Noise observed live**: arch-t5 `compile+graph` measured
   min_ms 5.52 in one process and 11.3–11.7 in another (its paired
   eager baseline moved 6.2→11.9 with it — the paired-round protocol
   absorbed the contention correctly, which is the point of it).
6. hnl `tensor.ones`/`full`/`from_list` bindings mint `Param` leaves
   bound to *seeded synthetic* values — the "ones" in my chains are
   actually randn.  Timing-relevant? No (same shapes/ops); noted for
   honesty.  `dispatch_many.hnl` v1 diverged to inf (unbounded
   mul-chain) and correctly verify-declined — rewritten with scalar
   damping (`tensor.mul(a, 0.5)` — Const scalars do import).
7. **Env note**: whisper/kokoro adapters need the `kokoro` extra —
   the mlopt `.venv` had been re-synced without it; installed
   `transformers==5.23.0 loguru attrs huggingface_hub` via
   `uv pip install` (venv-only change, no repo/lockfile edits).
   transformers 5.x worked fine for all lifts.

## Corpus recommendation

Train on all 21 (positives teach config choice; the 2 negatives are
the confirm-eager stratum).  If imitation-positive-only weighting is
used, drop `whisper-tiny-encoder` + `kokoro-textenc-p-t128-v3` from
the imitation stream per `imitate_positive_only` — they stay in the
REINFORCE stream.

Not done (time/GPU): no `train_delivery_prior.py` run — this task
was corpus construction; the drains ARE the persisted answer key
(`corpus_v6.json`).
