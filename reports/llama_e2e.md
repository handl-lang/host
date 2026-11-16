# LLaMA architecture end-to-end verification — TinyLlama-1.1B

Date: 2026-11-16. Host: 11.6 GB RAM + earlyoom, `/home` nearly full
(1.8 GB free at time of writing). Verdict: **INFRASTRUCTURE-BLOCKED** —
the Rust lift/bind and the optimizer search/lower path pass, but the
Python-side `export --verify-run` cannot fit in host RAM even for a
1-layer reduction of the spec.

## Spec discovery — PASS

`ls source-onnx/data/architectures/ | grep -i llama` → only
`tinyllama-1.1b.json` (`family: "llama"`,
`TinyLlama/TinyLlama-1.1B-Chat-v1.0`, input `input_ids [1,128] int64`,
hidden 2048, 22 layers, 32 q / 4 kv heads, intermediate 5632, vocab
32000, fp32). Checkpoint `model.safetensors` (~4.1 GB) present in the
local HF cache — no download needed. This is the smallest (only)
viable LLaMA spec.

## Dependency sync — PASS (after pin repair)

`uv sync --extra hf` initially failed on mixed
`discoverykit v0.1.12`/`v0.1.14` pins. Resolved by aligning mlopt's
pins to the consistent set: graphopt `v0.1.15`, eqsat `v0.1.9`,
gamekit `v0.1.10`, discoverykit `v0.1.14`, kit-carriers `v0.1.1`.
`mlopt/pyproject.toml` carries these edits (uncommitted).

## eqsat-run freshness — PASS

`eqsat-run` resolves to
`/home/luis/GitHub/handl-lang/engine-eqsat/target/release/eqsat-run`.
engine-eqsat HEAD `d777a84` has `3443774` ("shape: identity-keyed
infer memo + node budget") as an ancestor — the saturation fix is in
the resolved binary lineage.

## Rust architecture lift + weight binding — PASS (full 22 layers)

After the safetensors memory fix (see below), the full spec assembles
and verifies against the real checkpoint:

```
source-onnx$ ./target/release/examples/arch \
    data/architectures/tinyllama-1.1b.json --checkpoint
→ {"ok": true, "weights_verified": true, ...}   (rc=0, ~194 KB Term JSON,
  completes in seconds — no OOM)
```

Bound-payload safetensors writes were also exercised:

- 1-layer reduction spec → `/home/luis/tmp/llama1_w.safetensors`
  (700,474,636 B ≈ embed 262 MB + lm_head 262 MB + 1 block), rc=0.
- 8-layer reduction spec → `/home/luis/tmp/llama8_w.safetensors`
  (~1.93 GB), produced earlier.
- Full 22-layer `--st-out` (~4.4 GB payload) was **not** written:
  `/home` had only 1.8 GB free (disk 100%); assembly+binding itself
  passed without it.

### Memory fix that made this possible (already committed)

`source-onnx` HEAD `bf71d5e` "arch: stream bound-payload safetensors
writes (OOM fix for large checkpoints)": `WeightStore` keeps only
safetensors headers and reads each tensor's own byte range
(`pread`/`read_exact_at`) per lookup instead of `fs::read`-ing whole
shards; `write_bound_weights` streams instead of accumulating a
second full-size buffer. Before this, every arch/mlopt attempt on the
4.1 GB checkpoint was SIGKILLed by earlyoom at 4.0–4.5 GB RSS.

Note: `source-onnx` also carries **uncommitted, unrelated** work —
`src/arch.rs`/`tests/arch.rs` (+1258 lines: qwen2 + distilbert arch
families) and untracked `data/architectures/{qwen2-0.5b-instruct,
distilbert-base-uncased}.json` — from a parallel effort, not this task.

## mlopt optimize — PASS on 8-layer reduction; full spec BLOCKED

Reduced spec `/home/luis/tmp/tinyllama-8l.arch.json` (same spec with
`llama.layers: 8`):

```
.venv/bin/mlopt optimize 'arch:/home/luis/tmp/tinyllama-8l.arch.json'
model:    arch:/home/luis/tmp/tinyllama-8l.arch.json
backend:  torch
search:   0.1s          (eqsat-run, MLOPT_EQSAT_TIMEOUT=5400)
lower:    3.3s  verify=True
note:     input values: synthetic …
```

This proves the full lift → e-graph search → `lower(verify=True)`
gate for the LLaMA family. It is **not** full-model verification.

Full-spec `mlopt optimize 'arch:tinyllama-1.1b'` was attempted
repeatedly; the Python process binding the ~4.4 GB bound payload was
SIGTERM/SIGKILLed by earlyoom/kernel OOM at ~4.3 GB RSS while other
agent sessions held the host's memory.

## mlopt export --sidecar --verify-run — FAIL (infra-blocked)

Every attempt died in OOM, including reductions and even **without**
`--verify-run`:

| attempt | spec | result |
|---|---|---|
| export --verify-run ×3 | 8-layer | earlyoom SIGKILL ~4.0 GB RSS |
| export --verify-run | 4-layer (~1.5 GB payload) | earlyoom SIGTERM at **7,747 MB RSS** |
| export (no verify-run) | 4-layer | earlyoom SIGKILL at 6,651 MB RSS |
| export --verify-run | 1-layer (700 MB payload) | earlyoom SIGTERM at **6,604 MB RSS** |

Key observation: Python RSS is ~9–11× the safetensors payload and has
a ~6.6 GB floor even at 1 layer (payload floor is embed+lm_head ≈
524 MB). The amplification is in the Python bind path
(`adapters/arch_rust.py` → safetensors/torch/numpy copies → IRModule
params → VerifyReport), not in the Rust lifter. On an 11.6 GB host
already carrying ~3–4 GB of other sessions, no LLaMA-spec export can
fit; `--verify-run` never executed, so **no `.hnl`/sidecar artifact or
`hnl-run` numerical result exists for any LLaMA spec**.

## Stage summary

| stage | result |
|---|---|
| spec discovery | PASS — `tinyllama-1.1b.json`, checkpoint cached |
| `uv sync --extra hf` | PASS (after pin alignment, uncommitted) |
| `eqsat-run` resolution/freshness | PASS — release binary, fix 3443774 in history |
| Rust arch assembly + weight binding (22L) | **PASS** — `ok:true, weights_verified:true`, seconds, low RSS |
| bound-payload safetensors emit | PASS at 1L/8L; 22L not attempted (disk full) |
| `mlopt optimize` search+lower(verify=True) | PASS on 8L reduction (0.1 s / 3.3 s); full spec OOM-killed |
| `mlopt export --sidecar` | FAIL — OOM even at 1L without verify-run |
| `hnl-run --verify-run` | NOT REACHED — infra-blocked |

## Artifacts (preserved, not committed, not deleted)

- `/home/luis/tmp/tinyllama-{1l,4l,8l}.arch.json` — reduced test specs
- `/home/luis/tmp/llama1_w.safetensors` — 700 MB bound payload (1L)
- `/home/luis/tmp/llama8_w.safetensors` — ~1.9 GB bound payload (8L)
- `/home/luis/tmp/llama22_arch.json` — full-spec lift Term JSON
  (`weights_verified: true`)
- `/home/luis/tmp/llama{1,4,8}_export*.log` — attempt logs (mostly empty;
  kills were external)
- Pre-existing, untouched: `mlopt/arch-t5-small.{hnl,safetensors}`,
  `mlopt/arch-gpt2-small.{hnl,safetensors}`

## Uncommitted changes (do not commit per instructions)

- `mlopt/pyproject.toml` — dependency pin alignment + `hf` extra tweaks
- `mlopt/src/mlopt/adapters/{arch_rust,onnx_rust}.py` — memmap/ranged
  safetensors reads on the Python side
- `source-onnx` `bf71d5e` already committed the Rust OOM fix;
  remaining `src/arch.rs`/`tests/arch.rs` modifications + two untracked
  spec JSONs are a **parallel effort** (qwen2/distilbert families)

## What would unblock this

1. Python-side weight binding must stop materializing ~5–10× the
   payload (stream/memmap leaf tensors through `arch_rust.py`, or bind
   leaves lazily at verify time).
2. More free RAM or a quieter host window; disk space on `/home`
   (currently ~1.8 GB free — the 4.4 GB full payload cannot even be
   written).
3. Re-run then: `mlopt optimize 'arch:tinyllama-1.1b'` and
   `mlopt export 'arch:tinyllama-1.1b' --sidecar --verify-run` with
   `MLOPT_EQSAT_TIMEOUT=5400 MLOPT_HNL_RUN_TIMEOUT=1800`.
