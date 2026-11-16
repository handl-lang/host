# llama-22L verify-run — retry after reboot kill (STATUS: BLOCKED by earlyoom, retry loop armed)

Date: 2026-11-16. **No completed eval yet** — every attempt today was
SIGTERM'd by `earlyoom` within ~4–6 min while *other* agents' jobs
(`train_delivery_prior.py`, `run_census.py`/`census_cases.py`, kit-torch
`coverage run -m pytest`, pytest-xdist workers + torch inductor
compile-worker pools) kept system `MemAvailable` oscillating around
earlyoom's SIGTERM threshold (~10% of its ~8.4 GiB view). hnl-run is
always the largest `oom_score` process once it passes ~2 GiB, so it is
the first casualty of every sweep — its own ~3–5 GiB peak is *under*
the ~8.5 GiB budget; the kills are system-level, not self-inflicted.

## Command (reconstructed — identical shape to the llama{4,8} runs)

Per `llama8_verify_run.md`, invocation is **file-arg mode** (not stdin),
`TENSOR_LOAD_DIR=/home/luis/tmp`, RSS sampled 1 Hz via
`/home/luis/tmp/rssmon.sh`:

```sh
cd /home/luis/tmp
TENSOR_LOAD_DIR=/home/luis/tmp \
  /home/luis/tmp/rssmon.sh /home/luis/tmp/<tag>.rss \
    /home/luis/GitHub/handl-lang/graphopt/target/debug/hnl-run \
    /home/luis/tmp/llama22_verify_prog.hnl \
    > /home/luis/tmp/<tag>.json 2> /home/luis/tmp/<tag>.err
```

- Binary: `graphopt/target/debug/hnl-run` (debug build, mtime
  2026-11-16 10:39 — same binary family as the 4L/8L runs; repo HEAD
  `d4e2ea5`, ADR-0179 `069d7db` is in history).
- Program: `/home/luis/tmp/llama22_verify_prog.hnl` (1,209,147 B) —
  the `_ambient_src` form of the `verify-run` stdin payload;
  156 `tensor.load("llama22_e2e.safetensors", …)` sites.
- Sidecar: `/home/luis/tmp/llama22_e2e.safetensors` (4,399,842,691 B,
  F32 storage via `--sidecar-dtype F32`).
- Ref: `/home/luis/tmp/llama22_ref.npy` `[1,128,32000]` f32 (saved as
  f64), `llama22_ref_meta.json` → `"label": "native-f32-memmap-ref"`
  (same native-f32 fallback as llama-8L — the f64 gate
  `2×4.4 GB ≤ budget` declined).
- Compare (once a run completes):
  `python3 /home/luis/tmp/cmp_out.py <out.json> /home/luis/tmp/llama22_ref.npy /home/luis/tmp/llama22_ref_meta.json`
  (PASS threshold max_abs < 1e-3, same as 4L/8L).

## Attempts today (all exit=143 = SIGTERM, `journalctl -u earlyoom` confirms each)

| attempt | start | wall | peak RSS (rssmon) | earlyoom kill VmRSS | outcome |
|---------|-------|------|-------------------|--------------------|---------|
| v4 (manual) | 13:30 | 318 s | 3,682,392 KB ≈ 3.51 GiB | 3,533 MiB @13:36:10 | SIGTERM |
| v5 (manual) | ~14:08 | 240 s | 2,827,200 KB ≈ 2.70 GiB | 2,229 MiB @14:12:07 | SIGTERM |
| r1 | 14:30:36 | 204 s | 2,443,068 KB ≈ 2.33 GiB | — | SIGTERM |
| r2 | 14:38:00 | 265 s | 2,732,908 KB ≈ 2.61 GiB | — | SIGTERM |
| r3 | 14:45:25 | 230 s | 3,082,936 KB ≈ 2.94 GiB | 2,985 MiB @14:49:15 | SIGTERM |
| r4 | 14:55:15 | 241 s | — | — | SIGTERM |
| r5 | 15:02:16 | 248 s | — | — | SIGTERM |
| r6 | 15:10:24 | 370 s | 3,759,332 KB ≈ 3.59 GiB | — | SIGTERM |
| r7 | 15:19:35 | 342 s | 4,811,008 KB ≈ 4.59 GiB | — | SIGTERM |
| r8 | 15:28:17 | running at report time | — | — | — |

Every kill is `earlyoom[647]: sending SIGTERM to process … "hnl-run"`
during a system-wide low-memory sweep that also SIGTERMs/SIGKILLs
desktop helpers and other agents' pytest processes. Peak RSS observed
so far: **4.81 GB** (attempt r7) — consistent with the pre-reboot
v3 peak of 5.29 GB; the eval never reaches its steady-state phase
(weights stream in via `tensor.load` during the forward; the 4L/8L
runs peaked at ~2.0/3.0 GiB *after* completing).

## Retry loop (armed, survives this session)

`/home/luis/tmp/llama22_retry.sh` (nohup pid 2359372) loops up to 12
attempts: waits for `MemAvailable ≥ 3.5 GiB`, runs the exact command
above tagged `llama22_r<N>.{rss,json,err,time}`, stops on exit 0.
Log: `/home/luis/tmp/llama22_retry.log`. To resume later:

```sh
nohup /home/luis/tmp/llama22_retry.sh > /home/luis/tmp/llama22_retry.nohup.log 2>&1 &
# then when any llama22_r<N>.time shows exit=0:
python3 /home/luis/tmp/cmp_out.py /home/luis/tmp/llama22_r<N>.json \
  /home/luis/tmp/llama22_ref.npy /home/luis/tmp/llama22_ref_meta.json
```

## Deviations from the 8L protocol

- **Blocked, not run to completion** — no max_abs/mean_abs/verdict yet.
- Attempt numbering: manual v4/v5 plus scripted r1–r12.
- Output JSON lands in `llama22_r<N>.json` (the v5 script's redirect
  still points at `llama22_hnl_out_v4.json` — cosmetic only).
- Box contention is the sole blocker; ADR-0179 artifacts are intact
  and the binary/program/ref/sidecar are unchanged from the killed
  pre-reboot attempt.

## Recommended next step for the parent

Wait for a quiet window (other agents' pytest/census jobs to drain —
`avail` was 6.5–7 GiB at 15:17 and attempts started with up to
7.0 GiB available still died, so a *sustained* lull is needed, not a
momentary one), then re-arm `llama22_retry.sh`. Alternatively
coordinate a pause of the competing jobs; ~60–90 min contiguous is
the expected debug-build runtime (8L took ~23 min; 22L ≈ 2.5–3× work).
