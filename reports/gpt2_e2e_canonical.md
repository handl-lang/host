# GPT-2-small canonical monolithic verify-run — PASS

Date: 2026-11-16 ~07:20–07:26 local. Clean end-to-end confirmation of the
monolithic `arch:gpt2-small` export + `--verify-run` path (the decomposed
run had already passed; this re-ran the canonical single-shot command).

## Command

```sh
cd /home/luis/GitHub/handl-lang/mlopt
MLOPT_EQSAT_TIMEOUT=3600 MLOPT_HNL_RUN_TIMEOUT=1800 \
  .venv/bin/mlopt export 'arch:gpt2-small' --sidecar --sidecar-dtype F32 --verify-run
```

## Result — PASS

```
model:    arch:gpt2-small
backend:  torch
search:   0.2s
lower:    0.3s  verify=True
note:     input values: synthetic — the spec declares shapes, not data;
          pass --adapter-opt inputs='{"name": [dims]}' or feed=<json>
note:     eqsat search timeout: 3600s via MLOPT_EQSAT_TIMEOUT
export:   arch-gpt2-small.hnl
sidecar:  arch-gpt2-small.safetensors (651668826 bytes)
verify-run: PASS — max_abs=1.794e-12 over 3216448 values;
            ref=f64 dtype=torch.float64 atol=1.0e-09 rtol=1.0e-09
            — f64 reference vs dense-f64 host — tight bound
```

- **max_abs = 1.794e-12**, tolerance atol=1e-9 / rtol=1e-9 → comfortably inside.
- 3,216,448 compared values (gpt2-small forward, synthetic probe inputs).
- Artifacts (untracked, left in place per instructions):
  - `mlopt/arch-gpt2-small.hnl` (2,660,241 B)
  - `mlopt/arch-gpt2-small.safetensors` (651,668,826 B, F32 sidecar)
- Full log: `/tmp/gpt2_canonical_run.log` (tmpfs — ephemeral).

## Prereq state at run time

- `source-onnx` @ `bf71d5e` (streaming bound-payload writes committed);
  `cargo build --example arch` rebuilt clean. Worktree has unrelated
  uncommitted qwen2/distilbert emitter additions (`src/arch.rs`,
  `tests/arch.rs`) — do not touch the gpt2 path.
- `engine-eqsat` `cargo build --release` — already current
  (`eqsat.py` resolution prefers `target/release/eqsat-run`).
- `uv sync --extra hf` **failed**: `handl-lang/kit-discovery` git dep is
  unfetchable over https (private repo, `Repository not found`). Worked
  around with `uv pip install --no-deps /home/luis/GitHub/handl-lang/kit-discovery`
  into `mlopt/.venv` — sufficient because the run only needs
  `kit_discovery.hnl_codec` / `hnl_import` (stdlib + `mlopt_core` only).
  **Follow-up:** mlopt's `uv sync` is currently broken until the
  kit-discovery source is fixed (ssh URL or published tag).

## Infra notes (memory)

- Host: 11 GB RAM + 4 GB swap, earlyoom active. A concurrent
  `coverage run -m pytest tests/ -q -m not slow` in `kit-discovery`
  (PID 1664388, ~1.8 GB RSS + idle inductor workers) was running
  throughout — the run still completed without OOM pressure
  (available stayed ≥ ~3.5 GB; peak `hnl-run` RSS ~2.0 GB).
- `/tmp` (tmpfs, 5.7 GB) was ~80% full of stale safetensors scratch from
  prior sessions; cleared ~3.8 GB of regenerable files
  (`mlopt-export-smoke/`, `t5-*`, `wtf/wtd/wt2`, `whisper-tiny-full-bound`,
  `bert.safetensors`). Kept `/tmp/gpt2-bound.safetensors` (decomposed-run
  artifact). First launch attempt also died instantly on
  `ModuleNotFoundError: kit_discovery` (see prereq note), not on memory.

## Verdict

Canonical monolithic gpt2-small export + dense-f64 verify-run: **PASS**.
Pipeline confirmed end-to-end on committed HEAD tooling.
