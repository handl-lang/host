# llama-8L verify-run — ADR-0179 binary tensor wire + F32 storage

Date: 2026-11-16. Binary: `graphopt/target/debug/hnl-run` built at HEAD
`069d7db` ("ADR-0179: materialize __tensor_bin__ at hnl-run stdout; wire.py
bin decode"). Invocation: **file-arg mode** (`hnl-run <file.hnl>`, not stdin),
`TENSOR_LOAD_DIR=/home/luis/tmp`, RSS sampled 1 Hz via `/home/luis/tmp/rssmon.sh`.

Programs/refs were staged earlier by the split-verify scripts
(`/home/luis/tmp/llama{4,8}_split_verify.py`): the `.hnl` program is the exact
stdin payload `verify-run` would emit (`emit_program(term, run_feed,
sidecar=…, sidecar_dtype="F32")` + `_ambient_src`), and the safetensors
sidecars are the F32 payloads (`llama4_e2e.safetensors` 1.23 GB,
`llama8_e2e.safetensors` 1.93 GB).

## Results

| case | sidecar | ref | wall | peak RSS | verdict | max_abs | mean_abs |
|------|---------|-----|------|----------|---------|---------|----------|
| llama-4L | 1.23 GB F32 | f64 torch | ~16 min | **2,154,020 KB ≈ 2.05 GiB** | **PASS** | 3.07e-05 | 3.76e-07 |
| llama-8L | 1.93 GB F32 | f32 torch (native) | ~23 min | **3,129,448 KB ≈ 2.99 GiB** | **PASS** | 2.10e-04 | 5.84e-06 |

Both exited 0; outputs are `[1, 128, 32000]` logits, `dtype: f64` on the
wire (`{"__tensor__": {"data": …}}` — `__tensor_bin__` materialized back to
array spelling at stdout per ADR-0179).

Comparison script: `/home/luis/tmp/cmp_out.py` (elementwise |hnl − ref| over
the flattened 4,096,000 logits).

## Notes / caveats

- **The memory wall is gone.** The pre-ADR-0179 llama-8L attempt was
  earlyoom-killed at ~5.2 GB RSS under load; the same program now peaks at
  ~3.0 GiB — roughly the expected halving from F32 storage plus the binary
  wire keeping large tensors out of the JSON/Value path during eval.
  `tensor.load` on the ≥4096-elem weight tensors works (58 `tensor.load`
  sites in the 8L program, all resolved through the sidecar).
- **llama-8L ref is f32, not f64.** The split script's f64 gate
  (`2 × 1.93 GB ≤ 3 GiB`) declined, so `llama8_ref.npy` is the *native f32*
  torch forward (saved as f64). The 2.1e-4 max_abs therefore includes f32
  reference noise on top of the hnl side's own rounding — rel ≈ 1.6e-5 vs
  `ref_absmax` 13.1, consistent with f32 accumulation over 8 layers.
  llama-4L *did* get an f64 ref and shows 3.1e-5 max_abs, the cleaner
  number.
- **Debug build.** These are debug-profile runs (~16/23 min wall); a
  release `hnl-run` would be much faster but was not rebuilt for this
  pass. RSS figures are debug-build numbers.
- Box was loaded (load avg ~18–42, other agents' pytest running);
  ~3–4.5 GiB stayed available throughout — no earlyoom events, no retry
  needed.

## Artifacts

- `/home/luis/tmp/llama4_verify_prog.hnl`, `llama4_hnl_out_v2.json`,
  `llama4_v2.rss` (`peak_rss_kb=2154020 exit=0`)
- `/home/luis/tmp/llama8_verify_prog.hnl`, `llama8_hnl_out_v4.json`,
  `llama8_v4.rss` (`peak_rss_kb=3129448 exit=0`)
- Refs: `llama4_ref.npy` + `llama4_ref_meta.json` (f64),
  `llama8_ref.npy` + `llama8_ref_meta.json` (native f32)
- No repo files modified; report only.
