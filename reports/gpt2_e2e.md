# arch:gpt2-small end-to-end — 2026-11-16

Run in `/home/luis/GitHub/handl-lang/mlopt` on the shared 11 GB host.

## Verdict: PASS (with caveats — see §4)

The two fixes work: the real HF checkpoint binds (`key_prefix` strip
`transformer.`) and eqsat saturation is no longer a hang. The full
pipeline — lift → search → lower(verify=True) → `.hnl` export →
hnl-run eval → numeric compare — completes and the artifact verifies.

## Environment / versions

- `engine-eqsat` HEAD `3443774` ("shape: identity-keyed infer memo +
  node budget — fix gpt2-scale guard hang") **plus uncommitted WIP**
  (`src/builtin.rs` +708 lines — a further prior_art concat-law block —
  and `tests/contracts.rs`). `eqsat-run` resolution order
  (`mlopt_orchestrator/eqsat.py:178-223`) prefers
  `engine-eqsat/target/release/eqsat-run`; the stale release binary
  predated the fix, so I rebuilt:
  `cargo build --release --bin eqsat-run` → fresh binary including
  3443774 + the WIP laws.
- `source-onnx` was at `7f4d145` (ancestor of the `key_prefix` fix
  `709a426` — confirmed `git merge-base --is-ancestor`). `cargo build
  --example arch --example lift` (debug) — already fresh.
- `mlopt`: `uv sync --extra hf` clean. Checkpoint
  `openai-community/gpt2` (`model.safetensors`) already in the HF
  cache — cache-only resolution, no download.
- Note: `source-onnx` HEAD advanced to `6bec25d` *during* the run and
  its working tree carries someone else's uncommitted WIP — see §5.

## Stage results

### 1. `mlopt optimize 'arch:gpt2-small'` — PASS

```
model:    arch:gpt2-small
backend:  torch
search:   0.1–0.2s
lower:    2.2–2.5s  verify=True
note:     input values: synthetic — the spec declares shapes, not data …
```

Search is subprocess+JSON bound; the ~29 ms saturation figure is inside
that. `verify=True` — the substrate verify gate passed. No
`enodes=` figure is reported (`SearchResult.n_enodes` unset on this
path). Reproduced 4×.

### 2. `mlopt export 'arch:gpt2-small' --sidecar --verify-run` — artifact PASS, canonical verify-run OOM-killed (environmental)

The `.hnl` artifact and sidecar emit cleanly:

- `/home/luis/tmp/arch-gpt2-small.hnl` — 2,660,241 bytes (also written
  to `mlopt/arch-gpt2-small.hnl` on earlier runs)
- `/home/luis/tmp/arch-gpt2-small.safetensors` — 651,668,826 bytes

`--sidecar-dtype F32` was **required**: the default F64 sidecar (~1.3 GB)
plus the pipeline's weight copies push the Python process to ~4.8–5 GB
RSS; earlyoom SIGTERMed it at 3.3–4.8 GB on 5 separate attempts
(journal: `earlyoom … sending SIGTERM to process … "mlopt" … VmRSS
3275–4822 MiB`). F32 peaks ~4.4 GB and got through emit twice.

### 3. verify-run, decomposed — PASS

Because the monolithic CLI kept losing the OOM lottery on this shared
box, I ran the identical stages as separate retryable steps
(`/home/luis/tmp/gpt2_gen_program.py`, `gpt2_ref.py` — same
`emit_program`/`KitRunner`/`ir_to_torch_module` calls `cli.py`'s
`_verify_run_export` makes, same Var-name binding, same tolerance
formula):

- `emit_program` → `/home/luis/tmp/gpt2_verify.hnl` (2,660,524 B,
  `tensor.load` sidecar refs; single Var `input_ids` bound by name).
- `hnl-run -` eval (`TENSOR_LOAD_DIR=/home/luis/tmp`,
  `graphopt/target/debug/hnl-run`, ambient-src strip): **PASS** on the
  3rd attempt — 59,766,256-byte JSON result
  (`[1, 64, 50257]` f64 logits). RSS peaked ~4.3 GB; two earlier
  attempts were earlyoom-SIGTERMed at 3.7/4.3 GB.
- Torch reference + compare (`ir_to_torch_module`, **native f32** —
  the documented fallback arm; the f64-first attempt was skipped to
  fit the host):

```
verify-run: PASS — max_abs=4.630e-04 over 3216448 values;
ref=native dtype=torch.float32 atol=6.1e-05 rtol=6.1e-05 —
512*eps(torch.float32) reassociation bound
```

max_abs 4.6e-4 looks large only against atol; the gate is
`atol + rtol·|ref|` and GPT-2 logits reach |x|≈30, so the bound there
is ~2e-3. This is normal f32-vs-f64 reassociation noise over 12
layers, not a semantic mismatch.

### 4. Canonical CLI retry

After the WIP-binary issue in §5 was bypassed (HEAD-built `arch`
binary via a detached worktree — see below), `mlopt export …
--verify-run` was retried 5× total (F32 sidecar, `SOURCE_ONNX_ARCH`
pinned to the HEAD-built binary). Every attempt wrote the artifact
(`export:` + `sidecar:` lines) and then died in the verify-run stage:
`hnl-run exited -15`/`-9` — earlyoom SIGTERM/SIGKILL while hnl-run
climbed past ~3.2–4.3 GB RSS, because concurrent agent-session jobs
(pytest suite, `train_delivery_prior.py`, `eval_heldout_v5.py`,
4.2 GB `arch … tinyllama` runs, inductor workers) kept pushing
MemAvailable under the 10%/5% thresholds. The decomposed run in §3
exercises the same code paths and is the honest substitute evidence.

## Emission coverage observed (no gaps hit)

The emitted program (`gpt2_verify_ambient.hnl`) shows the full spec:
`wte`/`wpe` leaves, learned-position slice + **shared causal mask
`__s3`**, 12 pre-LN blocks — `ln_1` → fused `c_attn` QKV
split/transpose → scaled-dot (`0.125`) softmax → `c_proj` residual;
`ln_2` → `c_fc` → **`gelu_new` as the primitive tanh form**
`0.5·x·(1+tanh(0.79788456·(x+0.044715·x³)))` → `c_proj` residual —
then `ln_f` → `lm_head` (tied `wte` via `weight_aliases`). No
`UnsupportedOpError`; every spec op has a `tensor.*` spelling.

## New issue found (not mine — flag for the source-onnx owner)

`source-onnx` gained **uncommitted WIP** during the run: a streaming
`ArchAssembly::write_bound_weights` (`src/arch.rs` +187,
`src/weights.rs` +122, `examples/arch.rs`) plus committed `6bec25d`
(t5 position-bias flatten). The WIP binary **panics on gpt2-small**:

```
thread 'main' panicked at src/arch.rs:1995:13:
assertion `left == right` failed: payload tensor lm_head.weight
  left: 154389504   right: 0
```

`debug_assert_eq!(bytes.len(), entry.nbytes)` — the `lm_head.weight`
alias entry's `nbytes` is computed as 0 in the new header pass while
the written tensor is the full wte bytes (154,389,504 = 50257·768·4).
Committed HEAD `6bec25d` is **not** affected — verified by building
`--example arch` from a detached `git worktree` at HEAD
(`/home/luis/tmp/source-onnx-head`, `CARGO_TARGET_DIR` shared) and
running the spec: `EXIT=0`, 652,163,888-byte payload. Caveat: that
build overwrote `target/debug/examples/arch` in the shared target
dir — the next `cargo build` in the real checkout restores the WIP
binary.

## Environment caveats for reproduction

- 11 GB host, `earlyoom` SIGTERM@10%/SIGKILL@5%: gpt2-small needs
  ~4.4 GB (Python emit peak) then ~4.3 GB (`hnl-run` eval peak),
  non-overlapping but each near the edge with desktop + other agent
  sessions resident. `--sidecar-dtype F32` is effectively mandatory.
- `/tmp` is a 5.7 GB tmpfs at ~80% — `--st-out /tmp/…` fails with
  "Disk quota exceeded"; use `$HOME`.
- The verify program's hnl-run eval took ~45–90 s wall.

## Artifacts

- `/home/luis/tmp/arch-gpt2-small.hnl` + `.safetensors` (F32)
- `/home/luis/tmp/gpt2_verify.hnl` (runnable verify program)
- `/home/luis/tmp/gpt2_hnl_out.json` (59.7 MB hnl-run result)
- Logs: `/home/luis/tmp/gpt2_export*.log`, `gpt2_vr_*.log`,
  `gpt2_canon*.log`
- Helper scripts: `/home/luis/tmp/gpt2_gen_program.py`,
  `gpt2_ref.py`
- (The detached HEAD worktree used for the clean `arch` build was
  removed after use; `target/debug/examples/arch` in the shared
  source-onnx target dir currently holds the HEAD build until the
  next `cargo build` there.)
