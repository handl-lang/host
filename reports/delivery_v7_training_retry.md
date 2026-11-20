# Delivery prior v7 — training retry (lighter split + checkpointing): INFRA-BLOCKED (earlyoom ×3)

**Verdict: no held-out arms table, no bake.** `src/mlopt/weights.py`
stays `DELIVERY_PLAYER_WEIGHTS = {}` — not because the trained arm
lost, but because the process never survived the `held_out` phase.
Three attempts, three earlyoom kills on the same 11.6 GB host, all
inside ~90 s of each other at the same log position.  Per the run
constraints ("max ~3 resume attempts then stop and report
infra-blocked") the experiment stops here — **but the new
checkpointing worked exactly as designed**: the expensive train
phase is banked and a future quiet-window resume only pays the
eval lifts + held-out cells.

## What changed vs the first v7 attempt

- Repo at `79546f1` ("deliver_train: checkpoint/resume + per-cell
  held-out progress"), now HEAD `6c10981` (a follow-up deliver-stats
  commit that landed during the session; `79546f1` is an ancestor).
  `git pull --ff-only` clean; working tree clean except pre-existing
  untracked `arch-*.hnl/.safetensors` scratch files (not touched).
- Stack verified via `importlib.metadata` direct_url: kit-cuda
  requested_revision **v0.1.5** (commit e361eb9 — the fixed build),
  kit-torch v0.1.4, kit-carriers v0.1.5, mlopt-core/orchestrator
  eqsat v0.1.12, kit-discovery v0.1.4.
- Lighter split (below) removed `whisper-turbo-decoder` — the
  binding ~3 GB lift constraint from the first report.
- `--ckpt` checkpointing: post-imitation and post-train phases each
  persist; `--resume` skips completed phases.

## Split used

Corpus `/home/luis/tmp/delivery_v7/corpus_v7.json` (22 entries),
corpus mode.  Command (attempt 1; resumes identical plus
`--resume <ckpt>`):

```
MLOPT_EQSAT_TIMEOUT=900 nice -n 10 .venv/bin/python \
  scripts/train_delivery_prior.py \
  --corpus /home/luis/tmp/delivery_v7/corpus_v7.json \
  --eval-name hnl-dispatch-many-l --eval-name hnl-select-sort \
  --eval-name hnl-gemm-bound --eval-name hnl-where-mix \
  --eval-name arch-whisper-small-encoder \
  --skip-name whisper-turbo-decoder --skip-name whisper-small-encoder \
  --skip-name arch-whisper-tiny-full \
  --skip-name kokoro-textenc-t64 --skip-name kokoro-textenc-p-t64-v1 \
  --ckpt /home/luis/tmp/delivery_v7/train_v7c.checkpoint.json \
  --json  /home/luis/tmp/delivery_v7/train_result_v7c.json
```

- **Train (12):** all `hnl-*` except the four held-out —
  broadcast-mix-s/l, outer-mix, reduce-fold, dispatch-many-m,
  elem-chain-l, attn-wide, conv-tail2, gemm-pointwise, linear-tail,
  softmax-blocks, norm-dispatch.
- **Held-out (5):** `hnl-dispatch-many-l`, `hnl-select-sort`,
  `hnl-gemm-bound`, `hnl-where-mix`, `arch-whisper-small-encoder`.
- **Skipped (5):** `whisper-turbo-decoder` (the OOM binding case),
  `whisper-small-encoder`, `arch-whisper-tiny-full`,
  `kokoro-textenc-t64` (corpus `ok=False`), `kokoro-textenc-p-t64-v1`.

## Timeline — three kills, one banked phase

All logs/artifacts under `/home/luis/tmp/delivery_v7/`.

**Attempt 1** (`train_v7c.log`, started ~19:52): all 17 case lifts +
`train()` completed by ~19:53 — imitation 88/88 moves cloned,
train block means `[5.488, 4.103, 4.086, 5.51, 5.035, 4.256]`,
post-train checkpoint written 19:53.  Held-out `trained` cells 1–5/5
all completed.  **earlyoom SIGKILL 19:57:36** (journal
`_COMM=earlyoom`), process VmRSS ~1.3 GB — host-wide pressure
(avail ~0.5–0.9 GB, swap 0 free; earlyoom was simultaneously
killing desktop daemons).

**Attempt 2 — resume 1** (`train_v7c_r1.log`, ~19:59): post-train
resume correctly skipped train ("resuming … train phase complete"),
re-lifted the 5 eval cases, re-ran all 5 `trained` cells.  **earlyoom
SIGTERM 20:03:15**, VmRSS 1253 MiB — same log position (first
`trained-greedy` cell), amid another host-wide kill wave.

**Attempt 3 — resume 2** (`train_v7c_r2.log`, ~20:04): identical
progress; memory had recovered to ~2.9 GB avail before launch.
**earlyoom SIGKILL 20:07:59**, VmRSS 1343 MiB — third kill at the
same point.  Attempt budget exhausted → stop.

No `train_result_v7c.json` was ever written (the driver writes
`--json` only at the end).  **No CUDA poison signatures** appeared;
no drain records were lost to context poisoning — every death is a
host-memory kill, not a measurement failure.

## Checkpoint state (the salvageable asset)

`/home/luis/tmp/delivery_v7/train_v7c.checkpoint.json` (62 KB,
phase=`trained`) contains the full learned player: 27 weight keys
(`bias`, `st:*` features), `totals`, `imitation` stats, `picks`,
`drains`, `train_cases`, `epochs=8`, `imitate_epochs=4`.  A resume
in a quiet window skips straight to eval-case lifts + `held_out`
— roughly 10–15 min of work instead of the full pipeline.

## Arms table

**None produced.**  Per-arm means, errors and done-counts live only
in the process that reaches the end of `held_out`; three processes
died inside it.  The five `trained` cells ran to completion twice
but their per-cell rewards were never persisted — honest negative:
no numbers exist to report, and none are fabricated here.

## Decision

**Do not bake.**  `DELIVERY_PLAYER_WEIGHTS` remains `{}`; the
`linear*` arms stay honestly cold.  No commit — no source changes
(`git status` clean apart from pre-existing untracked `arch-*`
scratch files).

## What unblocks it

1. **Quiet-window resume** — re-run the exact command above with
   `--resume /home/luis/tmp/delivery_v7/train_v7c.checkpoint.json`
   (keep `--ckpt`/`--json` paths).  Only held-out remains; the
   trained weights are banked.  Best odds: off-peak, when no other
   agents' pytest/coverage runs are live (the census drains
   succeeded 14:30–15:16 under lighter load).
2. If earlyoom keeps winning even then, the remaining lever is
   splitting `held_out` itself (per-arm processes) — the ckpt makes
   each retry cheap, but that is a driver change, not attempted
   here.
3. The v6 honest negative stands unrefuted; the v7 corpus's larger
   oracle ratios remain an untested opportunity, gated on host RAM.
