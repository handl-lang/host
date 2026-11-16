# t5-small `export --verify-run` — T-101 emit-side fix check

Date: 2026-11-16 ~04:05 -0300.  Verification only — nothing committed.

## Verdict: FAIL — but *past* the T-101 cast; new named failure point

Command:

```sh
cd handl-lang/mlopt
MLOPT_EQSAT_TIMEOUT=5400 MLOPT_HNL_RUN_TIMEOUT=1800 \
  .venv/bin/mlopt export 'arch:t5-small' --sidecar --verify-run
```

Result (full log: `/home/luis/tmp/t5_verify_run3.log`):

```
verify-run: hnl-run rejected the program: hnl-run exited 1:
  eval error: tensor.index_select: index must be rank-1,
  context: " at 227401..228062 in call"
model:    arch:t5-small
backend:  torch
search:   0.1s
lower:    0.4s  verify=True
export:   arch-t5-small.hnl
sidecar:  arch-t5-small.safetensors (615525932 bytes)
```

## What this means for T-101

- **Lift → search → lower → `verify=True` → emit all pass.**  The
  `maximum(scaled, 1.0)` clamp in source-onnx `edb8a75` means the
  dead-branch `tensor.to(..., "int64")` inside the T5
  relative-position-bucket arithmetic now evaluates on finite values —
  hnl-run eval proceeded *through* the bucket `log`/`div`/`to` chain
  (it is inside the failing call's index argument, i.e. it was
  evaluated before the rank check fired).
- **New blocker, one stage later:** `--verify-run`'s hnl-run eval of
  the emitted `.hnl` fails at
  `tensor.index_select: index must be rank-1`.  The failing call is the
  **encoder block-0 `SelfAttention.relative_attention_bias` gather** in
  `mlopt/arch-t5-small.hnl` (~offset 226948; error span 227401..228062
  is inside it):

  ```
  tensor.index_select(
    encoder_block_0_layer_0_SelfAttention_relative_attention_bias_weight,
    0, <bucket-index tensor of shape [64,64]>)
  ```

  The bucket index is rank-2 (`[q_len, k_len]` = `[64,64]`); the host
  `tensor.index_select` requires a rank-1 index.  `emit.py`'s
  `_embedding`/`_index_select` (`src/mlopt/emit.py:1088-1125`) document
  this exact case: "a rank>1 index would decline honestly at eval" —
  the arch emitter mints the relative-bias gather over the 2-D bucket
  matrix without flatten+reshape, so the honest decline now surfaces.
  Torch's reference path accepts the rank-N gather, which is why the
  `lower(verify=True)` gate passed while the `.hnl` artifact cannot be
  evaluated.  (The decoder relative-bias `index_select` at ~235441 has
  the same shape and would fail next.)

  This is **not** the T-101 dead-branch-cast issue — `tensor.to` no
  longer sees non-finite input here.  It is a separate emit/eval gap:
  either the t5 arch emitter should flatten the bucket index
  (`reshape [64*64]` → `index_select` → `reshape [64,64,heads]`) or
  host-tensor needs a rank-N `index_select`/`embedding` spelling.

## Environment / binary notes

- `source-onnx`: rebuilt `cargo build --example arch` at 03:35 —
  `target/debug/examples/arch` includes `edb8a75`.
- `hnl-run`: mlopt selects `graphopt/target/debug/hnl-run` first
  (`emit.py:_hnl_run_candidates`); rebuilt via
  `cargo build -p mlopt-kit --bin hnl-run` — already fresh, includes
  host-tensor `ecc57d9` (narrowing casts) + `54d1f7e` (erf port) via the
  graphopt `[patch]` to `../host-tensor`.
- `eqsat-run`: **a concurrent session is actively committing to
  engine-eqsat and rebuilding the binary.**  Run 2 (03:39, timeout 1200)
  hit `eqsat-run exceeded 1200.0s` on the pre-`3443774` binary.  Commit
  `3443774` "shape: identity-keyed infer memo + node budget — fix
  gpt2-scale guard hang" (04:00) landed mid-run; the release binary was
  relinked at 04:02 and run 3's search finished in **0.1 s**.  The fast
  search is attributable to that engine-eqsat fix, not to T-101 — flag
  if a clean-room rerun is wanted once the sibling session settles.
- First attempt (03:37) died with `eqsat-run emitted non-JSON (exit
  -15)` — SIGTERM with no earlyoom/kernel kill in the journal; likely
  transient host contention (earlyoom was killing chrome around then).
  Retry ran clean.
- Memory stayed ~2.5-4 GB available throughout; no OOM involvement in
  the final run.

## Artifacts left behind (untracked, not committed)

- `mlopt/arch-t5-small.hnl` (248 KB)
- `mlopt/arch-t5-small.safetensors` (615 MB sidecar)
- `mlopt/scripts/train_delivery_prior.py` shows a pre-existing `M`
  modification — **not mine**, untouched.
