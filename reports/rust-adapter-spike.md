---
title: Rust adapter spike — feasibility evidence for ADR-0176
type: report
date: 2026-11-15
---

# Rust adapter spike — feasibility evidence for ADR-0176

Date: 2026-11-15.  Question: can the model adapters
(`mlopt/src/mlopt/adapters/{whisper,kokoro,hnl}.py`, plus the
`mlopt-onnx` `OnnxSource`) move from Python to Rust?  Deliverable is
evidence only — no production code.  All scratch work lives in
`/home/luis/tmp/rust-spike` (cargo project) and
`/home/luis/tmp/spike-artifacts/` (exported ONNX); nothing pushed.

## TL;DR

| Source path | Portable to Rust? | Evidence |
|---|---|---|
| ONNX file → graph walk | **Yes, today** | `onnx-ir 0.22.0` parsed a real exported whisper-tiny encoder (146 typed nodes) and a bundled resnet50 (415 nodes) with zero Python. |
| safetensors weights | **Yes, today** | `safetensors 0.4.5` enumerated all 167 tensors of `openai/whisper-tiny` `model.safetensors` (151 MB) — names, dtypes, shapes — and served raw f32 slices. |
| HF hub download | **Yes, today** | `hf-hub 0.4.3` `Api::model(...).get(...)` resolved `config.json`, `model.safetensors`, `preprocessor_config.json` from the local HF cache with `HF_HUB_OFFLINE=1`; `repo.info()` also worked offline. |
| HF transformers *architecture* | **Partially** | The whisper *encoder* forward is expressible as `tensor.*` ops with 1 missing op (`conv1d`, workaround exists). The decoder needs `embedding`/`index_copy_`-class mutation ops that are absent, and the audio frontend needs STFT/mel ops that are entirely absent. Weight tying (`proj_out` absent from the checkpoint) must be encoded in the spec. |
| Candle (HF's Rust ML framework) | **Yes — but it answers a different question** | `candle-transformers 0.8.4` loaded whisper-tiny from the same safetensors via `VarBuilder::from_mmaped_safetensors` and ran the encoder forward on CPU → `(1, 1500, 384)`. Candle *executes*; it does not lift to mlopt IR. It collapses "run the model in Rust", not "optimize the graph in Rust". |

The binding constraint is not parsing or weights — both are solved
Rust crates.  It is the **lift**: porting `mlopt_onnx.liftmap`'s
~90-op ONNX→IR table (or writing a `tensor.*` spec interpreter) into
Rust, plus extending the IR surface for the ops HF architectures
actually use.

## What was done empirically

Scratch cargo project `/home/luis/tmp/rust-spike`
(`safetensors 0.4.5`, `hf-hub 0.4.3`, `onnx-ir 0.22.0`,
`candle-core/nn/transformers 0.8.4`, `prost 0.13`, `serde_json`).
Real artifacts: `openai/whisper-tiny` checkpoint already in
`~/.cache/huggingface` (config.json + generation_config.json +
151 MB `model.safetensors`), and a whisper-tiny encoder exported to
ONNX opset 17 via `torch.onnx.export` (legacy TorchScript path,
`attn_implementation="eager"`) into
`/home/luis/tmp/spike-artifacts/whisper_tiny_encoder.onnx` (33 MB,
189 raw ONNX nodes, 67 initializers, opset `('' , 17)`).

Observed ONNX op census of the export (raw `op_type`):
`Constant 42, Add 35, MatMul 32, Mul 20, Transpose 17, Reshape 16,
LayerNormalization 9, Div 6, Erf 6, Softmax 4, Conv 2` — 11 distinct
ops.  Eager attention decomposes to MatMul+Softmax+Mul+Div; there is
no fused attention op in the export.

### safetensors

`SafeTensors::deserialize` + `SafeTensors::read_metadata` on the real
checkpoint: 167 tensors, all under `model.*`.  Encoder-relevant
shapes confirmed, e.g. `model.encoder.conv1.weight [384, 80, 3]`,
`conv2.weight [384, 384, 3]`, `embed_positions.weight [1500, 384]`
(learned positional buffer — just another leaf), per-layer
`self_attn_{q,k,v,out}_proj` + `fc1`/`fc2` + `layer_norm` weights.
`TensorView::data()` gives raw little-endian bytes; dtype/shape
accessors work.  **No Python anywhere in the path.**

Notable: `model.proj_out.weight` is **absent** from the checkpoint —
HF ties it to `model.decoder.embed_tokens.weight`
(`WhisperForConditionalGeneration.tie_weights`).  A Rust
architecture spec must carry an explicit weight-alias/leaf-share
mechanism or resolve ties at bind time; safetensors alone does not
tell you a leaf is shared.

### hf-hub

`hf_hub::api::sync::Api::new()` → `api.model("openai/whisper-tiny")`
→ `.get(filename)` returned cache paths for all requested files
under `HF_HUB_OFFLINE=1` (the spike set the env var inside the
process).  `repo.info()` returned the snapshot sha offline.  This is
the same cache layout `huggingface_hub` Python uses
(`models--<org>--<name>/snapshots/<sha>/`), so a Rust fetcher is
drop-in compatible with existing caches — no re-download needed for
cached models.

### onnx-ir (the Rust ONNX reader)

`onnx_ir::OnnxGraphBuilder::new().parse_file(path)` produced a typed
`OnnxGraph { nodes: Vec<Node>, inputs, outputs }`:

- whisper-tiny encoder ONNX → **146 nodes**, with `node_type()`
  giving a normalized enum (`Conv` → `Conv1d` Burn-style variant,
  `MatMul` kept distinct from `Linear`).  Op census after IR
  normalization: `Add 15, Constant 23, Conv1d 2, Div 6, Erf 6,
  LayerNormalization 9, Linear 24, MatMul 8, Mul 16, Reshape 16,
  Softmax 4, Transpose 17`.
- bundled `light_resnet50.onnx` (graphopt's onnx test data) → 415
  nodes parsed fine.

`onnx-ir 0.22` is the ONNX-Runtime-org IR crate (also pulled in
`burn-backend`/`burn-onnx` as a dependency — it is the parser half of
Burn's ONNX import).  It does proto decode (protobuf, not prost),
topological-sort validation, type inference, and optional graph
simplification (`OnnxGraphBuilder::simplify`, default on).  It is a
*graph IR*, not an executor — exactly the layer an mlopt lifter
needs.  The re-exported `ModelProto`/`NodeProto`/`GraphProto` types
also allow raw-proto access when the typed IR loses something
(attributes live in a private `attrs` field on nodes — reachable via
methods, but worth verifying per-op during a port).

For contrast, `ort` (the ONNX Runtime Rust binding) *executes* ONNX
via the ORT C library — useful later as a verify oracle, not as the
lifter.  The Python side already has the analogous split:
`OnnxSource` lifts, `OnnxSink`/ORT verifies.

### Python baseline: does `OnnxSource` lift this file today?

Yes — decisive control experiment.  In graphopt's venv:

```python
from mlopt_onnx.source import OnnxSource
ir, leaves = OnnxSource().to_ir(".../whisper_tiny_encoder.onnx")
# -> IR ok; leaves: 67
```

`mlopt_onnx.liftmap` covers ~90 ONNX op types (grep census:
Abs…Xor incl. Conv, Gemm, LayerNormalization, MatMul, Softmax,
Erf, Transpose, Reshape, Slice, Pad, Gather, Einsum, …).  Every op in
the whisper export's census is in the table.  So the *Python* ONNX
path already handles a real HF-exported encoder end-to-end; the Rust
question reduces to porting that table.

### candle

`candle_transformers::models::whisper` exists (`model.rs`,
`audio.rs`, `quantized_model.rs`).  `Whisper::load(&VarBuilder, Config)`
with `Config` deserialized **directly from HF `config.json`** via
serde (field names match: `num_mel_bins`, `max_source_positions`,
`d_model`, `encoder_layers`, …) and weights via
`VarBuilder::from_mmaped_safetensors(&[path], DType::F32, &Device::Cpu)`
(unsafe fn — mmap lifetime contract).  Encoder forward on a
`(1, 80, 3000)` randn mel input returned `(1, 1500, 384) F32` —
matches HF's `last_hidden_state` shape.  candle-transformers ships
~80 model families; coverage is broad but hand-ported per arch
(same "architecture is code" problem as HF transformers, just in
Rust).

## Whisper-encoder-as-data: op coverage vs `host_surface.json`

Checked the 238-op `tensor.*` surface
(`graphopt/packages/mlopt-host-tensor/.../host_surface.json`) against
what the whisper encoder forward needs (conv1d×2 → gelu → transpose →
+pos-embed → 4×{LN→self-attn→residual→LN→fc→gelu→fc→residual} → LN):

**Present:** `tensor.conv2d` (conv1d workaround), `tensor.layer_norm`,
`tensor.linear`, `tensor.matmul`, `tensor.gelu`, `tensor.softmax`,
`tensor.add/sub/mul/div`, `tensor.reshape`, `tensor.transpose`,
`tensor.slice`, `tensor.constant_pad_nd`, `tensor.concat`,
`tensor.expand`, `tensor.flatten`, `tensor.select`, `tensor.gather`,
`tensor.tril`.

**Missing (encoder-blocking):**
- `tensor.conv1d` — only `conv2d` exists.  Workaround: unsqueeze to
  `(b,1,c,l)` → conv2d → squeeze, or im2col+matmul.  Cheap but must be
  spelled in the spec or added to the surface.
- `tensor.permute` / `tensor.movedim` — absent, but `tensor.transpose`
  (with dim list) covers every permute whisper uses.
- `tensor.scaled_dot_product_attention` — absent; eager decomp
  (matmul→softmax→matmul) is what the ONNX export produces anyway, so
  not blocking, just verbose.

**Missing (decoder-blocking):**
- `tensor.embedding` — absent; `tensor.gather`/`index_select` on dim 0
  covers token lookup.
- In-place KV-cache mutation: `index_copy_`, `scatter`, `index_put_`
  all absent.  The adapter's whole `_KVWriteLayer` design exists to
  express this as an opaque mutation; a pure `tensor.*` spec cannot
  mutate a buffer.  Either the IR gains indexed-write ops or the
  decoder step stays outside the lifted fragment (which is what the
  Python adapter already does — it lifts *one* step with caller-owned
  buffers exported as opaque leaves).
- `tensor.searchsorted` exists (needed for timestamp alignment),
  `tensor.tril` exists (causal mask).

**Missing (frontend-blocking — kills "zero-Python" for the full
pipeline):**
- STFT/mel: `tensor.stft`, `rfft`/`fft`/`istft`, `hann`/window
  functions, mel-filterbank matmul — **none exist** in the surface.
  `WhisperFeatureExtractor` (log-mel from audio) cannot be expressed
  as `tensor.*` ops today.  The encoder adapter sidesteps this by
  taking precomputed `input_features`; a Rust zero-Python pipeline
  would need an STFT op family or keep the frontend in another
  language (Rust `rustfft` + a small mel kernel is straightforward,
  but it is *new* surface, not a port).

**Also absent but not whisper-critical:** `interpolate`/`upsample`,
`repeat_interleave` (kokoro needs it for dynamic durations),
`conv_transpose`, `one_hot`, `bucketize`, `tile`, `roll`,
`adaptive_avg_pool`, `glu` variant exists but no `swiglu`, no
rotary/rope helpers (deepseek/qwen families would need these or
manual decomp).

## What cannot move (structural, not crate availability)

1. **HF transformers modeling code is Python semantics, not a
   graph.**  Architecture-as-data must re-encode per-family rules:
   weight tying (whisper `proj_out`↔`embed_tokens` — confirmed absent
   from safetensors), buffer vs parameter distinction
   (`embed_positions` is a *learned* buffer here — leaf, fine;
   sinusoidal caches in other models are computed), attention-impl
   variants (`eager` vs `sdpa` produce different graphs — the spec
   must pin one), and `generation_config` policy (suppress token
   sets — control flow, not tensor ops).
2. **Mutable state.**  KV caches, `index_copy_` slot writes,
   `torch.compile` hooks, in-place buffer adoption — none of this is
   a pure function; the Python adapters already route around it with
   opaque leaves and caller-owned buffers.  A Rust lifter inherits
   the same boundary unless the IR grows stateful ops.
3. **Stochastic/dynamic forward.**  kokoro's `SineGen._f02sine`
   draws `torch.rand` *inside* the forward (the adapter literally
   monkeypatches device creators to survive export) and uses
   `repeat_interleave` with data-dependent lengths.  Not liftable as
   a static IR regardless of language.
4. **`.pth` (pickle) checkpoints** — kokoro ships `.pth`, not
   safetensors; Rust has no safe pickle-torch loader.  safetensors is
   the only clean weight format for a Rust path.
5. **The verify gate.**  `verify=True` compares lowered output
   against a torch reference — the reference executor stays torch
   (Python) or becomes candle/ORT (Rust) as a *second* implementation
   to cross-check, not a replacement for the semantic oracle.

## Per-adapter verdict

- **whisper encoder arm** — most portable.  ONNX export exists and
  lifts cleanly in Python today; a Rust path could be either (a)
  port `liftmap` + emit IR (bounded table, ~11 ops needed for this
  file), or (b) a whisper-encoder `tensor.*` spec + safetensors leaf
  binding.  Both feasible; (a) is more general (any HF model that
  exports to ONNX), (b) is more aligned with architectures-as-data.
- **whisper decoder arm** — blocked on mutation ops / stays a
  caller-owned-buffer fragment; the harvest/snapshot machinery is
  control flow that stays Python or moves to a Rust driver.
- **kokoro** — least portable: `.pth` weights, in-forward RNG,
  dynamic `repeat_interleave`, custom `KModel` loader, monkeypatch
  hacks.  Stays Python; at most the lifted submodule spec could be
  consumed by a Rust *runner*.
- **hnl** — already the most portable in spirit: `hnl_to_term` is a
  stdlib+`mlopt_core.ir` codec (text→IR, no torch).  Porting a parser
  to Rust is mechanical; the codec's honest bounds (shapes not
  values, `tensor.*` surface only) map directly onto a Rust
  frontend.

## Recommended Rust adapter scope (for ADR-0176)

**In scope for Rust:**
1. `safetensors` + `hf-hub` weight/fetch layer — proven, zero Python,
   cache-compatible.
2. ONNX→IR lifter: port `mlopt_onnx.liftmap`'s op table onto
   `onnx-ir`'s typed `Node` enum.  Bounded (~90 ops; whisper encoder
   needs 11).  This is the highest-leverage port — it makes *every*
   ONNX-exportable HF model liftable with zero Python, not just
   whisper.
3. Spec-driven leaf binding: a fragment spec (tensor-op DAG +
   safetensors leaves + weight-alias table for tied weights) consumed
   by a Rust interpreter that emits the same `IR` the Python codecs
   emit.  Encoder-class pure functions only.
4. `hnl` codec port — mechanical, torch-free already.

**Stays Python (or becomes a thin edge):**
- HF config semantics: tied-weight resolution, generation/suppress
  policy, feature-extractor config — *unless* pre-resolved into spec
  data at authoring time (recommended: bake resolved aliases +
  pinned attn impl into the spec).
- Audio frontend (STFT/mel) until the `tensor.*` surface grows an
  FFT/window family — `rustfft` exists if a native path is wanted.
- Decoder KV-cache mutation, generation loop, control flow.
- The torch verify oracle.
- kokoro wholesale.

**Strategic alternative worth naming in the ADR:** candle already
implements "HF model in pure Rust" for ~80 architectures — but as an
*executor with its own IR*, not a lifter.  If the end-state is
"optimize arbitrary models", candle doesn't substitute for the lift;
if the end-state is "run models fast in Rust without Python", candle
+ safetensors + hf-hub already delivers whisper today (verified
above) and makes the whole adapter question moot for inference-only
use cases.  The two goals should be separated in the ADR.

## Repro

```sh
cd /home/luis/tmp/rust-spike && cargo build --release && \
  HF_HUB_OFFLINE=1 ./target/release/rust-spike
# ONNX export (needs torch+transformers; done in /home/luis/tmp/wspike-venv):
#   python export_whisper_onnx.py  -> spike-artifacts/whisper_tiny_encoder.onnx
# Python control:
#   graphopt/.venv/bin/python -c "from mlopt_onnx.source import OnnxSource; \
#     print(len(OnnxSource().to_ir('.../whisper_tiny_encoder.onnx')[1]))"
```

Crate versions resolved: safetensors 0.4.5, hf-hub 0.4.3,
onnx-ir 0.22.0 (+protobuf 3.7.2, burn-* 0.22.1 as its deps),
candle-* 0.8.4.  One caveat: `/tmp` is a 5.7 GB tmpfs that filled
during the candle build (ld bus error) — the project was moved to
`/home/luis/tmp/rust-spike`; keep scratch cargo projects off `/tmp`
on this host.
