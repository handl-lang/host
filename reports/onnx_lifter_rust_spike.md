---
title: ONNX→IR lifter → Rust — feasibility spike
type: report
date: 2026-11-15
---

# ONNX→IR lifter → Rust — feasibility spike

Date: 2026-11-15.  Read-only analysis; no production code changed.
Binds: ADR-0176 (Python only at ecosystem edges), ADR-0178 (P3 codec
consolidation + `mlopt_core.ir` demotion, D5 Term-native adapters).
Companion: `reports/rust-adapter-spike.md` (onnx-ir/safetensors/hf-hub
evidence), `reports/p3_codec_consolidation_spike.md` (codec census),
`adrs/0178-…` (sequencing).

Python lifter under study:
`graphopt/packages/mlopt-onnx/src/mlopt_onnx/{lift.py,liftmap.py,source.py}`.
Scratch artifacts verified present: `~/tmp/spike-artifacts/whisper_tiny_encoder.onnx`
(189 raw nodes, opset 17) and `~/tmp/rust-spike/` (cargo project,
`onnx-ir 0.22.0` resolved; source in `~/.cargo/registry/src/*/onnx-ir-0.22.0/`).

## TL;DR

- **Feasible, medium effort.** `onnx-ir 0.22` covers everything the
  lifter needs *better* than the Python path in one respect: its
  per-op `Config` structs already resolve opset-version semantics
  (e.g. `ReduceAxes` folds the opset-18 axes-input-vs-attr split the
  Python `_axes` helper does by hand).  Initializers arrive as lazy
  `TensorData` constants; shapes arrive as `ArgType::Tensor{dtype,
  rank, static_shape}`.
- **Do not port onto `mlopt_core.ir`** — it is demoting (ADR-0178 D5).
  Emit the kernel-`Term` JSON envelope directly (`hnl_codec`'s
  `{"Op"/"Arr"/"Const"/"Opaque:dag.program"}` shape); the op-name +
  attr-spelling vocabulary is codec-stable and survives the IR
  demotion, so the lifter is forward-compatible *if* it targets the
  wire format, not the Python classes.
- **Sequence after WP0a** (liftmap/opmap generator + drift check).
  The table is currently handwritten and unguarded — the last
  unguarded codec table per the P3 census.  Generating it first gives
  the Rust port a machine-readable, drift-checked spec instead of a
  1,298-line Python file to hand-transcribe.
- **Encoder-class pure functions go zero-Python**: the whisper-tiny
  encoder ONNX (146 typed nodes, 11 distinct ops — all in the table)
  is the proven case; resnet/BERT-class encoders follow.  Decoder
  (KV mutation), audio frontend (no STFT ops), control flow, and
  quantized graphs stay refused — same boundary the Python lifter
  already enforces.

## 1. Liftmap census

`ONNX_LIFTS` has **107 entries** (the "~116" in the task/ADR-0178
census is approximate; `opmap.ONNX_BINDINGS` is 169 — the reverse
direction, out of scope here).  `liftmap.py` is 1,298 lines;
`lift.py` (the `GraphLifter` walk) is 603 lines.

Categorization of the 107:

| class | count | entries | needs |
|---|---|---|---|
| **Mechanical** — `_simple`/`_fold`/`_reducer` factories | **54** | 31 `_UNARY` + 15 `_BINARY` + 3 `_VARIADIC` (Sum/Max/Min) + 5 `_REDUCERS` | attr read + mint only; reducers fold axes-from-input-or-attr + `noop_with_empty_axes`→`clone` |
| **Attr-translation, single mint** | **~30** | Identity, Mod, Clip, LeakyRelu, Elu, Celu, Selu*, HardSigmoid*, Gelu, Softmax, LogSoftmax, IsInf, Cast, CastLike, Concat, Transpose, Flatten, Expand, Tile, Where, Dropout, MatMul, Einsum, ArgMax/ArgMin, ReduceL1/L2, GlobalAveragePool/GlobalMaxPool, BatchNorm, LayerNorm, GroupNorm, InstanceNorm, Conv, MaxPool | ONNX attr spellings → `ATTR_SCHEMA` canonical names; a few read const *inputs* (Clip bounds, Pad value, Trilu k, CumSum axis) |
| **Shape/const-fold/multi-output/decomposition** | **~23** | Reshape (0-extent needs input shape), Unsqueeze/Squeeze (rank), Split (equal-split needs axis extent; per-output `split` projections), TopK (values+`getitem` indices), Gather (rank-dependent select/embedding/index_select + reshape), NonZero (`permute` wrap), Pad (begins/ends reorder + mode map), Slice (4 const inputs, reversal→`flip`, per-axis decomp), Mean/ThresholdedRelu/Gemm/Selu/HardSigmoid (conditional decompositions), Range/ConstantOfShape/OneHot/Arange (const scalars + dtype side channel), CastLike (exemplar dtype) | static shapes, constant-array folding, multi-output binding |

\* Selu/HardSigmoid are single-mint at canonical params, decomposed
otherwise — the conditional logic is the port's care point, not new
machinery.

**Refused/unsupported** (no table entry → `UnsupportedOnnxOpError`;
documented in the module docstring, `liftmap.py:23-48`): `If`/`Loop`/
`Scan` (control flow), `Sequence*`/`Optional*`/`Map*`, `Shape`/`Size`,
all random ops, `Quantize*`/`QLinear*`/`MatMulInteger`, `Scatter*`/
`GatherND`, `Resize`/`Upsample`/`GridSample`/`RoiAlign`/`NMS`,
`RNN`/`LSTM`/`GRU`, fused `Attention` (opset >18), `AveragePool`
(`count_include_pad`), `LpPool`, `ConvTranspose`, `Hardmax`, `Det`,
DFT/window functions, all `ai.onnx.ml` featurizers — ~40+ op families
by name.  Plus the in-table refusals: non-default `domain`,
opset outside **13..21** (`lift.py:72-73,585-591`), non-constant
attr-fold inputs, non-tensor graph outputs, Dropout's mask output.

**Shape-info consumers** (where `g.shape()`/`value_info` is
load-bearing): Reshape-0, Unsqueeze, Squeeze, Split, Gather,
Conv (weight rank → conv1d/conv2d), LayerNorm (`normalized_shape`
from scale shape + axis check), GlobalPool (rank ≥ 3), OneHot
(axis bound), ConstantOfShape, CastLike/Range (dtype channel).
Everything else is shape-free.

## 2. onnx-ir coverage — what it exposes vs what the lifter needs

Verified against `onnx-ir 0.22.0` sources (registry path above):

| lifter need | onnx-ir provides | gap? |
|---|---|---|
| op dispatch | `Node` enum ~150 variants + `node_type()`/`op_type`/`domain`/`domain_opset` (`ir/node.rs:48-75`) | none; `Conv` normalizes to `Conv1d`/`Conv2d` Burn-style variants |
| attributes | per-op `*Config` structs, **opset-resolved** (`node/*.rs`; e.g. `ReduceConfig{axes: ReduceAxes, keepdims, noop_with_empty_axes}` folds the attr-vs-input split) | `Node.attrs` is `pub(crate)` — raw attrs NOT public. Configs looked complete for the reduce family; **audit all 107 per-op configs during port**; fallback below |
| raw attrs/opset | `ModelProto`/`NodeProto`/`GraphProto`/`TensorProto`/`ValueInfoProto`/`AttributeProto`/`AttributeType` re-exported (`lib.rs:111`); `protobuf::Message` re-exported | escape hatch: decode raw proto in parallel, overlay attrs by node order/name |
| opset version | `extract_opset_versions` is internal; **`OnnxGraph` does not retain it** (`pipeline.rs:437-450`) | read `ModelProto.opset_import` before/alongside `parse_*` — trivial |
| initializers→leaves | converted to `Constant` nodes backed by `TensorDataRef`, mmap-lazy (`graph_state.rs:165-210`, `tensor_store.rs`) | loses the initializer-vs-`Constant`-node distinction the Python lifter uses (0-d Constant→`Const` scalar vs initializer→`Param`); recover via the raw proto's initializer name set |
| constant folding | `Argument::value() -> Option<TensorData>`, `is_constant()`, `value_source` (`ir/argument.rs:22-44,369`) | `TensorData` covers numeric dtypes; string/sparse tensors need checking per use |
| shapes | `ArgType::Tensor(TensorType{dtype, rank, static_shape: Option<Vec<Option<usize>>>})` — rank always, dims `Option` | maps cleanly onto Python's `tuple[int\|None,...]`; verify inference parity vs `onnx.shape_inference` on intermediates (Gather output shape, Conv weight rank are the risky reads) |
| graph inputs/outputs + dtypes | `OnnxGraph.inputs/outputs: Vec<Argument>` with dtype+shape — **better than Python** (Python needs the `input_dtypes` side channel because `Var.typ` is shape-only) | none; the dtype side channel becomes first-class |
| subgraphs | `DeferredGraph` builds If/Loop/Scan bodies lazily (`ir/attribute.rs`) | irrelevant — those ops are refused anyway |
| domain | `node.domain`/`domain_opset` fields | none |

**Two caveats that shape the design:**

1. **`OnnxGraphBuilder::simplify` defaults to `true`** — CSE, dead-node
   elimination, pattern rewrites (`pipeline.rs:250-300`).  The Python
   lifter sees the *raw* graph; for parity use `.simplify(false)`.
   (Simplification before lifting is arguably desirable for the
   optimizer, but it is a semantic pre-pass the Python path does not
   have — decide deliberately, don't inherit the default.)
2. **onnx-ir's type inference ≠ `onnx.shape_inference` byte-for-byte**;
   where Python gets `None` dims ONNX-ir may give `static_shape=None`
   or a partial `Vec<Option<usize>>`.  The lifter must treat "rank
   known, extent unknown" uniformly — same contract, different
   provenance.

**Recommended architecture: typed-IR walk, not raw-proto walk.**  The
per-op configs already encode the opset-13..21 normalization the
Python lifter re-implements (`_axes`, `allowzero`, opset-20 Gelu,
`axes`-as-input).  Walking `OnnxGraph.nodes` with a `match` on
`NodeType` is *less* code than mirroring `GraphLifter` over
`NodeProto`s.  Keep the raw `ModelProto` decode only for: opset pin,
initializer-name set (leaf provenance), and per-op attr overlay where
a config proves lossy.

## 3. Port shape

```
ModelProto (re-exported protos)
  ├─ opset_import ──► pin 13..21 (refuse outside, same as Python)
  ├─ graph.initializer names ──► Param-leaf provenance set
  └─ OnnxGraphBuilder::new().simplify(false).parse_*
        └─ OnnxGraph { inputs, outputs, nodes }
             └─ GraphLifter port (~lift.py:399-522 walk)
                  env: HashMap<name, Term-ish>
                  const(): Argument::value() → TensorData → fold
                  shape()/dtype_name(): ArgType + initializer shapes
                  mint(): emit kernel-Term Op record
                        name = mlopt IR op name ("add","matmul",
                        "conv1d",… — codec passes names verbatim)
                        attrs = ATTR_SCHEMA canonical spellings
                  leaves: Param records + TensorData → safetensors
```

- **Attr schemas**: the canonical names/required sets are
  `mlopt_core.attrs.ATTR_SCHEMA` today; under ADR-0178 they become
  generated views of the tensor fragment — `host-tensor/data/
  language/tensor/ast.yaml` `attrs_schema` blocks (238 `tensor.*`
  ops, generated by `emit_tensor_language.py`).  For the Rust port,
  either (a) port the small per-op attr-name table for the ~107
  lifted ops only, or (b) consume `ast.yaml` directly — it's already
  data.  Note the lifted terms use **mlopt IR op names**, not
  `tensor.*` names (hnl_codec passes `t.op` through verbatim,
  `hnl_codec.py:234`); the bindings layer translates later.  So the
  lifter needs the *IR-op* attr spellings (`dim`, `keepdim`,
  `negative_slope`, …), which is `ATTR_SCHEMA`'s vocabulary — a
  distinct, smaller table than the fragment's.
- **Weights → leaves**: `TensorData` → `safetensors` crate serialize
  (proven in the spike) or keep ONNX external-data raw bytes.  Leaf
  names = ONNX value names (Python parity: `Param(name)`).
- **Emit target**: kernel `Term` JSON — `{"Op": {dim, name, args,
  source: {"Foreign":"tensor"}, attrs|lit.__mlopt_attrs__}}`,
  `{"Arr": …}` leaves carrying `{__mlopt_leaf__, name, shape}`,
  `{"Const": …}`, `Opaque dag.program` when sharing exists
  (`hnl_codec.py:215-319`).  `Term::Op.attrs` has landed upstream so
  emit `attrs` directly — no `lit` shim needed in Rust.
- **Refusals**: mirror `UnsupportedOnnxOpError` as a typed Rust error
  carrying `(op_type, node_name, reason)` — the honesty contract is
  the feature; keep it.

## 4. Verification path

Differential harness (the natural shape, reusing existing pieces):

1. **Python oracle**: `OnnxSource().to_ir(path)` → `IR` + leaves;
   serialize root via `hnl_codec.term_to_json` (or
   `mlopt_core.ir.term_to_data` during transition) → canonical JSON;
   leaves → manifest `{name, dtype, shape, sha256(bytes)}` (npz or
   safetensors).
2. **Rust candidate**: same envelope out of the lifter; leaves →
   safetensors; diff structurally.
3. **Corpus**: `~/tmp/spike-artifacts/whisper_tiny_encoder.onnx`
   (146 typed nodes, opset 17 — every op in-table), bundled
   `light_resnet50.onnx` (415 nodes), plus graphopt's ONNX fixtures
   (`tests/` onnx data).  Start with the whisper encoder — its op
   census (Add/Constant/Conv1d/Div/Erf/LayerNorm/Linear/MatMul/Mul/
   Reshape/Softmax/Transpose) exercises attr translation, const
   folding, and multi-input consts without hitting the ~20
   shape-sensitive lifters.
4. **Known diff hazards**: attr spelling-strictness (`0` vs `0.0`
   are different terms, `ir.py:198-207`) — Rust must emit i64/f64
   faithfully; `Const` int-vs-float scalar typing; `None`-dim
   normalization between the two shape inferencers; node *order* in
   serialization (topological in both, but onnx-ir's topo-sort may
   reorder ties — compare as DAGs, not text).
5. **Semantic check beyond structure**: the existing
   `OnnxSink`/onnxruntime verify path can execute a lowered graph;
   a Rust lifter's output lowered through the same sink gives a
   behavioral oracle for the subset that lowers.

## 5. Recommendation & sequencing

**Verdict: feasible — port, but Term-native and after WP0a.**

- **Now (cheap, unblocks everything)**: WP0a — generate
  `liftmap`/`opmap` from a spec + `--check` drift guard
  (ADR-0178 WP0a, "start now").  This converts the port source from
  prose-adjacent Python into a table the Rust port can be generated
  or checked against.
- **Then**: Rust lifter emitting kernel-Term JSON directly — *skip*
  `mlopt_core.ir` entirely (it is a demotion target; building a Rust
  producer for it would be building on sand).  This lands inside
  ADR-0178 D5's direction rather than against it.
- ** effort by component** (S < ~1 wk, M ~1–3 wk, L > 3 wk, part-time):

| component | size | notes |
|---|---|---|
| ONNX parse + opset pin + raw-proto fallback | **S** | protos + `Message` re-exported; ~150 lines |
| typed-graph walk: env, const-fold (`TensorData`→scalars/arrays), leaf materialization | **M** | the `GraphLifter` core; numpy→`TensorData`/ndarray semantics is the fiddly half |
| shape/dtype side channels (`ArgType`→leaf records, `input_dtypes`) | **S** | simpler than Python — dtype is first-class |
| mechanical table slice (54 entries) | **S** | pure data once mint() exists |
| attr-translation slice (~30) | **S–M** | per-op config audit is the real cost |
| shape/const/multi-out slice (~20) + decompositions | **M** | Gather/Split/TopK/Slice/Pad/Gemm/Reshape-0 carry the semantics |
| **full 107-op table** | **M–L** | ~1.5–2.5k lines Rust; encoder subset is a strict subset |
| Term-JSON emission (attrs channel + dag.program) | **S–M** | envelope is documented by `hnl_codec` + `Term` serde |
| leaves → safetensors export | **S** | crate proven |
| differential verify harness | **M** | corpus + canonical diff + hazard list above |
| **encoder-class subset** (whisper's 11 ops + friends) | **S–M** | the zero-Python unlock |

- **What it unlocks**: any ONNX-exportable pure-function model —
  whisper encoder (proven end-to-end today in Python), resnet/
  conv encoders, BERT-class encoders — with **zero Python** in the
  lift path (safetensors + hf-hub already pure-Rust per the spike).
  Still blocked, honestly: decoder KV mutation, STFT/mel frontend,
  control flow, quantized graphs, `ai.onnx.ml` — the same refusals
  the Python lifter makes, now in Rust.
- **What it does NOT do**: replace the Python lifter.  Keep
  `OnnxSource` as the oracle; the Rust lifter is a second producer
  behind the same wire contract, verified differentially — exactly
  the "one codec, two producers" end-state ADR-0178 describes.
- **Alternative named**: emitting `tensor.*` fragment ops directly
  instead of mlopt IR names would skip a bindings hop but invents a
  second op vocabulary the codec doesn't currently carry — reject
  for now; revisit if D5 settles on fragment-native terms.

## Evidence index

- `graphopt/packages/mlopt-onnx/src/mlopt_onnx/liftmap.py` — 107-entry
  `ONNX_LIFTS` (lines 1240-1298), factories at 117-211, refusals doc
  23-48.
- `graphopt/packages/mlopt-onnx/src/mlopt_onnx/lift.py` — `GraphLifter`
  (172-522), opset pin 13..21 (72-73), `lift_model` (569-603).
- `graphopt/packages/mlopt-onnx/src/mlopt_onnx/source.py` — `OnnxSource`
  port wrapper.
- `graphopt/packages/mlopt-discovery/src/mlopt_discovery/hnl_codec.py`
  — Term-JSON envelope, attrs channels (159-207), dag encoding
  (274-319).
- `eqsat/packages/mlopt-core/src/mlopt_core/ir.py` — `Op.make`
  ATTR_SCHEMA validation (237-265), spelling-strict attrs (198-207),
  `term_to_data` (497).
- `host-tensor/data/language/tensor/ast.yaml` — fragment attrs_schema
  (generated; `emit_tensor_language.py`).
- `onnx-ir 0.22.0` sources: `src/ir/{node,graph,argument,attribute}.rs`,
  `src/node/*.rs` (per-op configs), `src/pipeline.rs` (builder,
  simplify default, opset extraction), `src/graph_state.rs`
  (initializer→Constant normalization), `src/lib.rs:111` (proto
  re-exports).
- Prior spikes: `reports/rust-adapter-spike.md` (onnx-ir parse of the
  whisper encoder + resnet50, safetensors/hf-hub),
  `reports/p3_codec_consolidation_spike.md` (§1c/§3 codec census,
  liftmap named the last unguarded table).
