# LLaMA e2e retry — TinyLlama-1.1B after the Python-side memory fix

Date: 2026-11-16 (retry of `llama_e2e.md`). Host: 11.6 GB RAM +
earlyoom (hair-trigger: swap ~0 free, SIGTERM at mem-avail <10%),
`/home` ~6.5 GB free during the run. mlopt HEAD `a569845` ("stream
safetensors sidecar writes + dtype-native sidecar entries"),
source-onnx HEAD `0210522` (`cargo build --release --example arch`
rebuilt — includes the bf71d5e streaming fix plus later qwen2 work).

**Verdict: the blocker is gone.** The full 22-layer TinyLlama lift →
bind → eqsat search → `lower(verify=True)` → `.hnl` export → 4.4 GB
F32 safetensors sidecar all **PASS**. The remaining wall is
`--verify-run`: `hnl-run` itself needs more RAM than this host can
give it at llama scale (see root cause below), and on the one size
where it did complete (1-layer, standalone), the numeric compare
against the f64 torch reference exceeds the gate's 1e-9 bound —
flagged for investigation.

## Dependency sync — PASS with the predicted workaround

`uv sync --extra hf` **failed** exactly as warned: mlopt main pins
`kit-discovery @ v0.1.0` and the GitHub repo has no `v0.1.0` tag yet
(D-1d in flight). Workaround applied:

```
uv pip install --no-deps --python .venv/bin/python \
    /home/luis/GitHub/handl-lang/kit-discovery   # @1d6429f
```

(`.venv/bin/uv` does not exist; system `uv pip` + `--python` was the
working form.) `import mlopt, kit_discovery` OK afterwards.
Uncommitted local install — `uv sync` will keep failing until the
`v0.1.0` tag lands or the pin moves.

## Results per stage

| stage | spec | result |
|---|---|---|
| `mlopt optimize` (lift→search→lower verify=True) | **22-layer full** `arch:tinyllama-1.1b` | **PASS** — search 0.6 s, lower 2.6 s verify=True |
| `mlopt export --sidecar --sidecar-dtype F32` | **22-layer full** | **PASS** — `llama22_e2e.hnl` (1,208,576 B) + `llama22_e2e.safetensors` (**4,399,842,691 B**) |
| `mlopt export --sidecar --verify-run` | 8-layer | export+sidecar **PASS** (1,933,580,669 B F32); verify-run **FAIL** — `hnl-run` earlyoom-SIGTERM at 4,034 MB RSS (Python parent ~2.3 GB resident) |
| `mlopt export --sidecar --verify-run` | 1-layer | export+sidecar **PASS** (700,449,747 B F32); verify-run **FAIL** — `hnl-run` earlyoom-SIGTERM at 5,065 MB RSS |
| `hnl-run` eval, standalone (split verify) | 8-layer | **FAIL** — earlyoom-SIGTERM at 6,347 MB RSS |
| `hnl-run` eval, standalone | 4-layer | **FAIL** — earlyoom-SIGTERM at 6,295 MB RSS |
| `hnl-run` eval, standalone | 1-layer | **PASS** — ~3.95 GB peak RSS, ~230 s, 78 MB JSON result |
| numeric compare vs torch f64 ref | 1-layer | **exceeds gate bound** — max_abs=4.53e-07 (see below) |

All runs used `TMPDIR=/home/luis/tmp/mlopt-tmp` (disk, not the
RAM-backed `/tmp` tmpfs) so the transient bound-payload safetensors
(~payload-size) did not consume memory.

## The verify-run wall — root cause (new finding)

`hnl-run` OOMs are not the old Python-side amplification — they are
inside `hnl-run` itself, and the reason is in host-tensor:

- `tensor.load` (`host-tensor/src/ops/codec.rs:509` `load_safetensors`)
  does `std::fs::read` of the **entire safetensors file per call** —
  58 calls in the 8-layer program each read the full 1.9 GB sidecar
  (transient, but the allocator retains the arenas → RSS stays high).
- Every bound weight is then held for the whole `do`-block as **f64**
  (F32 sidecar widens `f32→f64` on ingest — exact): 1L ≈ 1.4 GB,
  4L ≈ 2.5 GB, 8L ≈ 3.9 GB, 22L ≈ 8.8 GB of weights alone.
- Measured peaks: 1L ≈ 5.1 GB (killed inside export; ~3.95 GB
  observed standalone before completion — true peak likely ~5 GB),
  4L/8L > 6.3 GB (killed standalone). 22L would need ≈ 13 GB —
  impossible on this host regardless of the Python side.

So `--verify-run` can only pass on this host for specs whose
f64-weights + one full sidecar read fit under ~5 GB — i.e. roughly
the 1-layer reduction, and even that only when the Python parent is
not also resident (the CLI always runs them together → the 1L CLI
attempt still died at 5,065 MB).

## The 1-layer numeric result — flag for investigation

Split verify (same `emit_program` + same sidecar + same probes the
gate uses; torch reference computed in-process, hnl-run run
standalone afterwards):

- host `tensor.*` eval vs **f64** torch reference (`label=f64`,
  `shape=[1,128,32000]`): **max_abs = 4.53e-07**, mean 8.9e-09,
  median 3.7e-09, p99 7.2e-08.
- At the gate's f64 tolerance (atol=rtol=1e-9): **2,368,512 / 4,096,000
  mismatches → the real `verify-run` would print FAILED** had
  `hnl-run` survived.
- At the f32-scaled bound (512·eps = 3.05e-05): 0 mismatches.

The diff is reassociation-scale, not a semantic break (first values
agree to ~9 digits; argmax diff 4.5e-7 on a logit of ~2.17).  Since
`tensor.load` F32→f64 ingest is exact and the F32 sidecar round-trip
of f32-native weights is lossless, the divergence is inside host
tensor op kernels (likely an f32-accumulating kernel somewhere in
the matmul/norm/softmax path) — worth a look before trusting
`--verify-run` PASSes at the f64 bound on deeper graphs.  Caveat:
this is a measurement through the split path, not the CLI gate
itself (which never reached the compare).

## Artifacts

Kept (small):
- `/home/luis/tmp/llama22_e2e.hnl` — full 22-layer export artifact (1.2 MB)
- `/home/luis/tmp/llama8_e2e.hnl` — 8-layer export artifact (485 KB)
- `/home/luis/tmp/llama1_e2e.hnl` — 1-layer export artifact (91 KB)
- `/home/luis/tmp/llama{1,4,8}_verify_prog.hnl` — the exact do-block
  programs the verify-run gate emits (split-verify evidence)
- `/home/luis/tmp/llama{1,4,8}_ref.npy` + `llama{1,4,8}_ref_meta.json`
  — torch references (1L=f64, 4L/8L=native-f32-fallback… 8L native,
  4L f64)
- `/home/luis/tmp/llama1_hnl_out.json` (78 MB) — the standalone
  hnl-run result compared above
- logs: `llama22_optimize_retry.log`, `llama22_export_retry.log`,
  `llama8_e2e_retry.log`, `llama1_e2e_retry.log`,
  `llama{1,4,8}_split_verify.log`
- scripts: `/home/luis/tmp/llama{1,4,8}_split_verify.py`

Deleted for disk (all regenerable, commands in this report):
- `llama22_e2e.safetensors` (4.4 GB — the 22L sidecar; rewrite via
  the export command above), `llama8_e2e.safetensors` (1.9 GB),
  `llama1_e2e.safetensors` (700 MB), `llama4_e2e.safetensors`
  (1.2 GB), `llama1_w.safetensors` (700 MB, prior bound payload),
  `llama1_e2e_f64.*` (prior F64-sidecar experiment)

## What remains blocked / next steps

1. `hnl-run` memory: `tensor.load` should range-read like the Rust
   side does (`pread` per tensor) instead of `fs::read` per call,
   and/or the do-block prelude should bind weights lazily.  Until
   then `--verify-run` is capped at ~1-layer specs on an 11 GB host.
2. The 1L numeric divergence (4.5e-7 vs f64 ref) needs a verdict:
   find the f32-precision op or relax the gate for sidecar-F32
   programs (the emitted program *is* f32-weighted by user choice).
3. `uv sync --extra hf` stays broken until `kit-discovery` tags
   `v0.1.0` or mlopt's pin is repointed (D-1d).
