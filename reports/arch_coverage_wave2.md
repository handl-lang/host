# Architecture coverage wave 2 — qwen2 + distilbert

Two new `mlopt.arch/v1` families landed in `handl-lang/source-onnx`:
`qwen2` (decoder-only causal LM) and `distilbert` (encoder-only
transformer). Both are first-class families — own spec section, own
validation, own emitter dispatch — not aliases of `llama`/`bert`.

## Specs

| spec | family | checkpoint declared | cached? |
|---|---|---|---|
| `data/architectures/qwen2-0.5b-instruct.json` | `qwen2` | `Qwen/Qwen2.5-0.5B-Instruct/model.safetensors` | **no** — only the AWQ int4 sibling is cached |
| `data/architectures/distilbert-base-uncased.json` | `distilbert` | `distilbert/distilbert-base-uncased/model.safetensors` | **no** |

Dims verified against cached/raw `config.json` files (Qwen2 values
cross-checked against the cached `Qwen2.5-0.5B-Instruct-AWQ` sibling —
same architecture; DistilBERT header keys verified against the remote
safetensors header, cache-only rule preserved — nothing downloaded).

## source-onnx status

- `src/arch.rs`: `Qwen2Spec`/`DistilbertSpec` sections, family
  dispatch + validation, `qwen2_forward` (reuses the llama-style
  decoder path with Qwen2 naming, `attention_bias`, tied
  `lm_head.weight → model.embed_tokens.weight` alias, GQA, RoPE,
  RMSNorm, SwiGLU) and `distilbert_forward`/`distilbert_layer`
  (word+position embeddings, **no** token-type embeddings, post-LN
  blocks: `sa_layer_norm` / `output_layer_norm`, biased q/k/v/out_lin,
  GELU FFN; MLM/task heads deliberately out of slice).
- `src/weights.rs` + `examples/arch.rs`: bound-payload emission is now
  **streamed** (`write_bound_weights`) — peak memory is one tensor,
  not the whole payload; the accumulating `bind_weights` path
  OOM-killed on multi-GB checkpoints. `tests/stream_parity.rs` proves
  streamed output is byte-identical to `bind_weights` on
  whisper-tiny-full / t5-small / gpt2-small.
- Assembly census: qwen2 = 291 leaves / 217 matmuls; distilbert =
  100 leaves / 48 matmuls. Op vocabulary unchanged — no unsupported
  ops were needed.

### Tests (`tests/arch.rs`, 55 total, all pass)

- Parse + assemble + op census for both families.
- Variants: `emit_logits=false`, `attention_bias=false` (qwen2),
  validation refusals for both.
- **Fabricated safetensors binding** for both (vocab/layers shrunk to
  4096/4 in-fixture for debug-mode speed — committed specs unchanged).
- Real-checkpoint status tests: qwen2 AWQ sibling refuses float bind
  by name; dense checkpoint and distilbert checkpoint report
  cache-miss honestly.

Gates: `cargo test` (all binaries), `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`,
`RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` — all clean.
`cargo build --release --example arch` rebuilt (mlopt prefers the
release binary; the stale one refused `qwen2` as an unknown field).

## mlopt status (adapter unchanged — `arch_rust.py` is spec-generic)

Real specs (checkpoint not in HF cache — honest declines):

- `mlopt doctor arch:qwen2-0.5b-instruct` → **available**;
  `probe.weights: not in HF cache`.
- `mlopt doctor arch:distilbert-base-uncased` → **available**;
  `probe.weights: not in HF cache`.
- `mlopt optimize arch:<spec> --iters 1` → named decline:
  `arch: checkpoint not found: <repo>/model.safetensors under
  ~/.cache/huggingface/hub — download it or pass an explicit weights
  path`. No download attempted (cache-only contract).

End-to-end on shrunk fixture specs (`/home/luis/tmp/arch-wave2/`,
vocab 256 / 2 layers / f32 fabricated safetensors — not committed):

| step | qwen2-tiny | distilbert-tiny |
|---|---|---|
| `optimize --iters 1` (verify=True) | pass | pass |
| `export --check` (hnl-run) | pass | pass |
| `export --verify-run` | **PASS** max_abs=1.17e-15 / 4096 vals | **PASS** max_abs=4.16e-17 / 1024 vals |

Inputs are seeded synthetic probes (specs declare shapes, not data) —
same contract as every arch adapter.

## Caveats / follow-ups

- No cached float checkpoint exists for either family, so real-weight
  binding is proven only via fabricated safetensors + the env-gated
  status tests. `hf download` of either checkpoint would light up the
  real-bind path with no code change.
- Qwen2's only cached sibling is AWQ int4 — binding it would need a
  dequantization story, deliberately not faked.
- mlopt needed **no adapter changes**; nothing committed there.
