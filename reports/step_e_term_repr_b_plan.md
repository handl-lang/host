---
title: Step E-1 design — Term representation option B ("thin Python view over a kernel-Term envelope") for mlopt adapters
type: report
date: 2026-11-17
status: implementation-ready plan (read-only analysis; binds mlopt_step_e_plan.md E-1/E-2/E-3, ADR-0178 D5)
inputs: mlopt_step_e_plan.md, ADR-0178, p3_codec_consolidation_spike.md, live code census (mlopt adapters, kit-discovery hnl_codec/hnl_import, eqsat mlopt-orchestrator eqsat.py, eqsat mlopt-core ir.py/attrs.py, engine-eqsat dag.rs/eqsat-run.rs/pat.rs, kernel engine term.rs, egraph term_bridge.rs, source-onnx term.rs)
---

# Step E-1 — Option B implementation plan

## 0. What the investigation found (the facts the design rests on)

### 0a. What adapters construct today

All five product adapters sit behind `mlopt.models.ModelSpec` +
`mlopt_core.ports.Source` (`to_ir(model, x) -> (IR, leaves)`), and
`IR`/`Op`/`Var`/`Param`/`Const`/`TensorType` are `mlopt_core.ir`
classes (`eqsat/packages/mlopt-core/src/mlopt_core/ir.py`, 956 lines):

- `onnx_rust.py:67` — `from mlopt_core.ir import IR, Op, Param, TensorType, Var`;
  `_lift` (line 571) runs the source-onnx `lift` subprocess, then
  `kit_discovery.hnl_codec.term_from_json(doc["root"])` (line 591)
  decodes the `dag.program`/tree Term-JSON into **ir terms**, `_census`
  (line 367) walks `Var`/`Param`/`Op` leaves, builds `IR(root=term,
  inputs=…, params=…)` (line 683). Safetensors payload → `param_values`.
- `arch_rust.py:60` — same path via `term_from_json` (line 461),
  reusing `onnx_rust._census`/`_sanitize_param_names`/`_load_safetensors`.
- `hnl.py:47` — `hnl_to_term` (`kit_discovery.hnl_import`, 1269 lines,
  30 `Op.make`/`Var(`/`Param(`/`TensorType` sites) produces ir terms;
  `_census` (line 84) + `IR(...)` (line 341) same shape.
- `whisper.py`/`kokoro.py` — zero direct ir imports; they emit
  `ModelSpec`s whose `source` port is `mlopt_torch`'s `TorchSource`
  (`torch_export` → ir `IR`). The ir surface is in mlopt-torch, not the
  adapter files.
- `Optimizer.search`/`lower` (`mlopt_orchestrator.optimize`) consume the
  `IR` opaquely-but-structurally: `source.to_ir` → `ir.root` walked by
  laws/cost/typing/egraph or encoded by `eqsat.py`; `lower` rebuilds
  `IR(root=result.term, inputs=result.ir.inputs, …)` (optimize.py:1176)
  and calls `sink.lower(ir, params)` — TorchSink/`IRModule` walks
  `Op`/`Var`/`Param`/`Const` via `torch_bindings`. `emit.py` (2971 ln)
  walks the same four classes + `typing.shape_of`.

So "Term-native" under any Python-side option means: the objects the
adapters decode into must be the same type the whole substrate
consumes. There is no way to flip only the five adapters without the
substrate's term type moving with them.

### 0b. The TWO existing ir→kernel-Term encoders — exact divergence map

**Encoder 1: `kit_discovery.hnl_codec`** (`kit-discovery/src/
kit_discovery/hnl_codec.py`, 479 ln — already split to its own repo):

- Tree: `{"Op": {"dim": {"Const":{"value":1}}, "name", "args": [<tree
  nodes>], "source": {"Foreign":"tensor"}, "lit": {"__mlopt_attrs__":
  {k: attr_to_data(v) …}}}}`; leaves `{"Arr": {"dim": …, "value":
  {"__mlopt_leaf__": "var"|"param", "name", "shape": […]|null}}}`;
  consts `{"Const": {"value": …}}` (`_enc`, `_leaf_json`, lines
  215–245).
- DAG: `{"Opaque": {"__tag__": "dag.program", "nodes": [...],
  "root": n}}` — post-order table, `Op.args` are **indices**, but
  **`Op.dim` is emitted as a nested `{"Const":{"value":1}}` record,
  NOT an index** (`_enc_dag`, lines 274–319). `_dec_dag` (390–444)
  ignores `dim` entirely.
- Decode accepts `attrs` field *or* legacy `lit.__mlopt_attrs__`,
  never both (lines 168–207). Non-finite floats →
  `{"__nonfinite__": "nan"|"inf"|"-inf"}` recursively.
- Dedup/sharing detection is `id()`-based (`_has_sharing`,
  `_enc_dag.index`) — relies on `Op.make` hash-consing.

**Encoder 2: `mlopt_orchestrator/eqsat.py`** (`_kterm`/`_kdag`/`_pyterm`/
`_pyterm_dag`, lines 429–718 — ~330 lines of parallel codec):

- Tree (`_kterm`): same `Op` record but `source: "Native"` (string,
  not `{"Foreign":…}`), attrs under **`attrs`** (the ADR-0173 field,
  not `lit`), `dim` nested record.
- DAG (`_kdag`, lines 505–575): `Op.dim` is a **node-table index**
  (`dim_index()` — `Const(1)` emitted once, shared); `Op.args` indices;
  `source: "Native"`.
- **Leaves are a DIFFERENT spelling**: `{"Const": {"value":
  "__mlopt_leaf__:" + json({"k","n","s"})}}` — a prefix-tagged JSON
  string inside a `Const` (`_leaf_payload`, line 429), NOT the
  `Arr`-record `__mlopt_leaf__` dict hnl_codec emits.
- `_pyterm`/`_pyterm_dag` decode back to ir terms; `_pyterm` has **no
  `"Arr"` branch** — an `Arr` record inside a dag table or tree raises
  `EqsatError("undecodable term record")` (line 718). Conversely
  `hnl_codec._dec_leaf` on a `__mlopt_leaf__:`-prefixed `Const` string
  returns `Const(str)` — the Var/Param typing is silently lost.

**The two codecs are NOT interoperable in either direction for leaves,
and their `dag.program` node schemas differ in `dim` (nested record vs
index).** They never meet today only because every consumer re-encodes:
adapter JSON → hnl_codec → ir → `_kdag` → eqsat-run → `_pyterm` → ir →
sink. `source` also differs semantically: `{"Foreign":"tensor"}` vs
`"Native"`.

### 0c. What the Rust side accepts (the actual contract)

- **Kernel `Term` serde** (`engine/crates/…/src/term.rs:26-230`):
  externally-tagged enum — `Arr{dim,value}`, `Const{value}`,
  `Op{dim,name,args,source,lit?,attrs?}`, `Seq`, `First`, `Choice`,
  `Loop`, `App`, `Iter`, `Child`, `Handle`, `Rewrite`, `Src`, `Tgt`,
  `RecordProj`, `RecordWith`, `Tag`, `Cobind`, `Opaque{data}`.
  `Op.dim` defaults to `Const(1)`; `attrs` is `Option<Value>`
  (ADR-0173, `skip_serializing_if=None`); `ArrSource` = `"Native"` /
  `{"Foreign": name}` / `"Judgment"` / `"DynamicForeign"`.
- **`eqsat-run`** (`engine-eqsat/src/bin/eqsat-run.rs`): stdin spec
  `{name, terms[], root, laws[], options{}, cost, check_terms[]}`;
  `terms` entries go through `eqsat::saturate_json` which accepts
  kernel-Term tree JSON **or** a top-level `dag.program` record;
  `check_terms` is kernel-Term tree JSON only.
- **`engine-eqsat/src/dag.rs`** — the dag decoder is STRICTER than
  hnl_codec's emitter:
  - `Op.dim` **must be a node-table index** (`node_ref` requires
    `as_u64`, lines 121–135, 193–198) — hnl_codec's nested-record dim
    would be a named `BadTerm` refusal.
  - Node tags: `Op` (index args/dim), `Const`, `Arr` — leaves decode
    through `serde_json::from_value::<Term>` + `from_kernel`.
  - **CRITICAL GAP**: an `Arr` leaf record becomes an `arr` *op enode*
    via `from_kernel` (`term_bridge.rs:54-58` — `Term::op("arr",
    [dim], [("value", attr)])`). On the return path,
    `dag::op_record` (lines 366–450) maps enode attrs back to kernel
    fields for `source`/`foreign`/`lit`/`__attrs__` only — a `"value"`
    attr hits the `other =>` arm and returns
    **`Error::BadTerm("dag output: enode attr \"value\" has no kernel
    Term spelling")`** (lines 411–415). So `Arr`-record leaves are
    input-accepted but **extraction-output-fatal** in engine-eqsat
    today. This is why the eqsat wire uses `Const`-string leaves
    (`Term::Leaf` → `leaf_record` round-trips verbatim, lines
    452–457, 493).
  - `dag::term_to_wire` (lines 462–522) emits tree-or-dag by
    occurrence/depth bounds; dag `Op` records carry `dim` as an index.
- **Law matching pins `source`** (`pat.rs:28-31, 235-255, 517-519,
  547-551`): patterns default `PatSource::Native` → attr
  `source:"native"`; attr matching is **exact-keyset**. A term
  arriving as `{"Foreign":"tensor"}` produces enode attrs
  `source:"foreign", foreign:"tensor"` which will NOT match
  native-source patterns. **The eqsat wire MUST emit `"Native"`** —
  this is load-bearing, not cosmetic.
- **Dep-graph constraint (why two encoders exist)**:
  `mlopt-orchestrator/pyproject.toml` deps = `["mlopt-core"]` ONLY;
  `kit-discovery` deps include `mlopt-orchestrator` — orchestrator
  *cannot* import `kit_discovery.hnl_codec` (cycle). The codec must
  live at or below `mlopt-core` in the dep graph.

### 0d. ir.py's API surface the view must preserve

`Op.make(op, *args, validate=True, **attrs)` — hash-consed via
`Op._INTERN` WeakValueDictionary (line 283), `validate_attrs`
(`attrs.py:374`) enforces `ATTR_SCHEMA`/`ATTR_REQUIRED`; spelling-strict
eq/hash via `_attr_key` repr-keys (lines 164–185, 226–234). `Const`
spelling-strict (`repr`-keyed eq/hash, 141–149). `TensorType.shape`
allows the `None` wildcard despite the annotation (52–57). `IR` =
`{root, inputs, input_names, params}` + `roots` property (918–956).
`op_def`/`generator`/`_OP_REGISTRY`/`GenDef` (698–792, ~40 registered
ops). `op_repr`/`op_repr_dag` bounded reprs (286–463).
`attr_to_data`/`attr_from_data` (`__tuple__`/`__list__`, 491–514).
`term_to_data`/`term_from_data` — the **lemma-store codec** (`{"op",
"args","attrs"}`/`{"var","param","const","mvar","__tag__":"dag.program"}`)
used by egraph certs/rulecache/trajectories and `vocab/laws.yaml` —
a *different* wire with metavar leaves; it is not a term codec and must
not be conflated with the kernel envelope (its `dag.program` is a third
dialect: `{"__tag__":…}` top-level, `{"op":…}` node records).

---

## 1. The design — what option B concretely is

**B = the ir *type contract* re-based onto the kernel-Term envelope,
living in `mlopt-core`, with ONE codec beside it.** Not "adapters emit
raw JSON dicts" (that is option C — every consumer would need a parse
layer) and not literal `hnl_engine::Term` objects (that is option A —
no PyO3 binding exists).

Concretely, two new modules in `eqsat/packages/mlopt-core`:

- **`mlopt_core/term.py`** — the view *types*: `TensorType`, `Var`,
  `Param`, `Const`, `Op` (+`Op.make` interning + `validate_attrs`),
  `IR`, `GenDef`/`op_def`/`generator`, `op_repr`/`op_repr_dag`,
  `attr_to_data`/`attr_from_data`. API-identical to today's `ir.py`
  class half — this is what makes the migration cheap: every consumer
  (`isinstance(t, Op)`, `t.op`, `t.args`, `t.attrs`, `t.name`,
  `t.typ.shape`, `Op.make(...)`) works unchanged.
- **`mlopt_core/term_json.py`** — the ONE envelope codec:
  `term_to_json(term, *, dag=None, leaf=…, budget=…)` /
  `term_from_json(data, *, allow_dag=True)` emitting/accepting the
  kernel `{"Op"/"Arr"/"Const"/"Opaque"}` tree + `dag.program` record.
  This is `hnl_codec` generalized: it absorbs `_wire_value`/
  `_unwire_value` (`__nonfinite__`), `_attrs_from_node` (`attrs`
  canonical, `lit.__mlopt_attrs__` legacy-decode), the leaf record,
  and a `wire_node_budget` parameter (the `_kdag` `state=[emitted,
  budget]` accounting moves here as a named error).

`ir.py` then becomes a **compat shim**: `from mlopt_core.term import
*` plus the lemma-store codec (`term_to_data`/`term_from_data`,
`_DAG_TAG`, `_leaf_to_data/_leaf_from_data`) kept in place — it is a
pattern/lemma wire, not a term wire. `kit_discovery.hnl_codec`
becomes a re-export shim over `mlopt_core.term_json` (its
`HnlCodecError` aliased or re-exported). `eqsat.py` deletes its codec
quartet (§4).

**Why mlopt-core and not kit-discovery or a new kit-terms repo:**
mlopt-orchestrator deps only on mlopt-core, kit-discovery deps on
orchestrator — the codec MUST live in mlopt-core or lower or
orchestrator can't reach it without a cycle. mlopt, kit-discovery, and
orchestrator all already dep on mlopt-core, so zero new dep edges. A
`kit-term` package would add a dep edge to every consumer for no
ownership win (the term repr is substrate, not discovery). Rejected;
record as considered.

**"Adapters construct envelopes directly":** under B the envelope *is*
the object graph — `term_from_json` returns `term.Op`/`Var`/`Param`/
`Const` view objects whose `term_to_json` is byte-faithful to the
kernel envelope. onnx_rust/arch_rust swap one import
(`kit_discovery.hnl_codec.term_from_json` → `mlopt_core.term_json.
term_from_json`) and everything else — `_sanitize_param_names`
(which walks the raw JSON *before* decode, unchanged), `_census`,
`IR(...)`, `param_values` — is untouched. There is no "ir
intermediate" because the view IS the pipeline's term type; the
kernel-Term-ness lives in the codec being total and canonical, not
in a second in-memory representation.

---

## 2. API sketch (code-level)

### `mlopt_core/term.py` — the view types

```python
# eqsat/packages/mlopt-core/src/mlopt_core/term.py
"""The kernel-Term view (ADR-0178 D5 option B).

API-identical to the historical ``mlopt_core.ir`` class contract;
the difference is canonical: serialization is ONLY the kernel
Term-JSON envelope (``mlopt_core.term_json``) — there is no second
term wire.  ``ir.py`` re-exports this module during transition.
"""
from __future__ import annotations
# TensorType, Var, Param, Const, Op(+make/_INTERN/validate_attrs),
# IR(+roots), GenDef/op_def/generator/_OP_REGISTRY,
# attr_to_data/attr_from_data, op_repr/op_repr_dag/_tree_sizes/_dag_shared
# — moved verbatim from ir.py (they are already the right contract).
```

No new methods needed on the types themselves; the codec is a sibling
module (keeps `term.py` torch-free and serde-free — same discipline
as today).

### `mlopt_core/term_json.py` — the one codec

```python
"""mlopt term ↔ kernel Term-JSON — THE term wire codec (ADR-0173/0178).

Envelope constants pinned HERE (single home):
  _DIM1        = {"Const": {"value": 1}}          # tree-position dim
  _SOURCE      = "Native"    # canonical ArrSource on the search wire
  _LEAF_RECORD = {"__mlopt_leaf__": kind, "name":…, "shape":…}  # Arr leaf
  _NONFINITE_TAG = "__nonfinite__"
  _DAG_TAG     = "dag.program"
  _LEAF_PREFIX = "__mlopt_leaf__:"   # legacy Const-string leaf (decode-only)

term_to_json(term, *, dag=None, leaf="arr", budget=None) -> dict
term_from_json(data, *, allow_dag=True, leaf="auto") -> term
TermJsonError(ValueError)   # was HnlCodecError
```

Encode rules (the consolidated spelling — see §5 for the two
transitional profiles):

- `Op` → `{"Op": {"dim": _DIM1, "name": t.op, "args": […],
  "source": "Native", "attrs": {k: _wire_value(attr_to_data(v))}}}`
  — **`attrs` field canonical** (kernel `Term::Op.attrs`); `lit.
  __mlopt_attrs__` emitted never, decoded as legacy (hnl_codec's
  existing dual-accept rule, `_attrs_from_node`).
- `Var`/`Param` → `{"Arr": {"dim": _DIM1, "value": {"__mlopt_leaf__":
  "var"|"param", "name":…, "shape": […]|null}}}` — the structured
  leaf record (hnl_codec/source-onnx spelling).
- `Const` → `{"Const": {"value": _wire_value(v)}}`.
- Sharing → `{"Opaque": {"__tag__": "dag.program", "nodes": […],
  "root": n}}` — **engine-eqsat's node schema**: post-order table,
  `Op.dim` AND `Op.args` are node indices (`Const(1)` emitted once,
  shared — the `_kdag.dim_slot` pattern), leaf records complete.
  Dedup key: the term object itself (content-hashable) with `id()`
  fast-path — works with or without `Op.make` interning and merges
  structurally-equal subtrees (a strict improvement over `id()`-only).
- `budget` → the `wire_node_budget` accounting moves here: a shared
  `[emitted, budget]` counter across a term list, raising
  `TermJsonError` naming the bound (replaces `_kdag`'s `state` arg and
  `_kterm`'s tree-expansion counter; `_kterm`'s tree walker is test-only
  and dies with it).
- Decode is tolerant where honest: `dim` accepts index (canonical)
  or nested record (legacy hnl_codec dag spelling — ignored, mlopt
  terms have no dim); `source` accepts any `ArrSource` spelling,
  ignored (mlopt terms are native-sourced — see §6 risk R2); `lit.
  __mlopt_attrs__` accepted; `Const` strings starting
  `__mlopt_leaf__:` decode to Var/Param (legacy eqsat spelling —
  decode-only, never emitted); other `Opaque` tags → named
  `TermJsonError`.

### `ir.py` — the shim

```python
# mlopt_core/ir.py — transitional (E-6 decides final form)
from mlopt_core.term import *          # the whole class contract
from mlopt_core.term import __all__ as _TERM_ALL
# term_to_data / term_from_data / _DAG_TAG / _leaf_to_data /
# _leaf_from_data stay — the LEMMA-STORE codec (mvar leaves, {"op":…}
# nodes), explicitly documented as not-a-term-codec.
```

Every existing `from mlopt_core.ir import Op, …` keeps working — the
E-6 question ("view vs delete") is deferred exactly as ADR-0178
allows; consumers migrate to `mlopt_core.term` lazily.

### `kit_discovery/hnl_codec.py` — the shim

```python
from mlopt_core.term_json import (
    TermJsonError as HnlCodecError, term_to_json, term_from_json,
)
__all__ = ["HnlCodecError", "term_from_json", "term_to_json"]
```

(Or keep the module as a thin wrapper if the `dag=`/`allow_dag=`
kwarg spellings need aliasing — they don't; signatures match.)

---

## 3. Module layout summary

| module | repo | role |
|---|---|---|
| `mlopt_core/term.py` | eqsat `packages/mlopt-core` | view types (the former ir.py class half) |
| `mlopt_core/term_json.py` | eqsat `packages/mlopt-core` | THE kernel-Term-JSON codec (former hnl_codec + _kdag unified) |
| `mlopt_core/ir.py` | eqsat `packages/mlopt-core` | compat shim + lemma-store codec (`term_to_data`/`term_from_data`) until E-6 |
| `kit_discovery/hnl_codec.py` | kit-discovery | re-export shim (compat for mlopt adapters, stage.py, differential driver) |
| `mlopt_orchestrator/eqsat.py` | eqsat `packages/mlopt-orchestrator` | codec quartet deleted; calls `mlopt_core.term_json` |

No new package, no new repo, no new dep edges.

---

## 4. E-3 codec consolidation — how hnl_codec and _kdag unify under B

**One encoder survives: `mlopt_core.term_json`.** Under B the
unification is forced by the dep graph (orchestrator can't see
kit-discovery) and made cheap by the view (both codecs already
encode/decode the same classes).

`eqsat.py` deletions (lines ~349–718): `_LEAF_PREFIX`, `_NONFINITE_*`,
`_DIM1`, `_wire_value`, `_unwire_value`, `_leaf_payload`, `_shape_of`,
`_kterm`, `_terms_wire`, `_DAG_TAG`, `_kdag`, `_leaf_from_payload`,
`_op_from_node`, `_pyterm_dag`, `_pyterm` — replaced by:

```python
# EqsatEngine.add_term:  self._terms.append(term)          # view terms
# EqsatEngine.run:       terms_json = [
#     term_json.term_to_json(t, dag=True, budget=self._wire_node_budget)
#     for t in self._terms
# ]
# result decode:         term = term_json.term_from_json(report["term"])
```

The `budget` semantics are preserved exactly (named error over
`wire_node_budget` distinct nodes). `_pyterm_synops` (line 364) — the
synop-coverage check on decoded terms — keeps working on view terms
(it walks `Op`s).

**The leaf-spelling problem (the one real design decision):**

| spelling | producer today | engine-eqsat status |
|---|---|---|
| `{"Arr": {"value": {"__mlopt_leaf__": {kind,name,shape}}}}` | hnl_codec, source-onnx | input-OK, **output-FATAL** (`arr` enode `value` attr, dag.rs:411) |
| `{"Const": {"value": "__mlopt_leaf__:{json}"}}` | eqsat `_kdag` | fully round-trips (`Term::Leaf`) |

Two honest resolutions:

- **(R1 — recommended long-term) Fix engine-eqsat**: teach
  `dag::op_record` the `arr`↔`Term::Arr` inverse (op `"arr"`,
  `args.len()==1`, `"value"` attr → emit `{"Arr":{dim,value}}` —
  `to_kernel` already has the forward arm at term_bridge.rs:193).
  ~20 lines in `engine-eqsat/src/dag.rs` + tests. Then ONE leaf
  spelling (the structured `Arr` record — self-describing, no
  string-embedded JSON, extensible) works everywhere. Cost: an
  engine-eqsat change + repin; sequence it as E-3a.
- **(R2 — zero-Rust-change transitional) Codec profiles**:
  `term_to_json(…, leaf="arr"|"const-str")` and matching decode.
  `profile="eqsat"` emits Const-string leaves + `source:"Native"` +
  dim-index dag (byte-compatible with today's `_kdag` output);
  `profile="canonical"` emits Arr-record leaves + dim-index dag.
  hnl_codec/stage/source-onnx consumers decode `leaf="auto"` (accept
  both). When R1 lands, the eqsat profile collapses to canonical and
  the profile parameter is deleted. Named, documented, drift-guarded
  — not a silent fork.

Recommend: ship R2 first (unblocks E-2/E-3 with no Rust dependency),
land R1 in engine-eqsat as its own step, then collapse. Either way
the *decoder* must accept both spellings permanently (old records).

`term_to_data`/`term_from_data` are NOT part of this consolidation —
they are the lemma-store/pattern codec (mvar leaves, `{"op":…}`
records, its own `dag.program` dialect). ADR-0178 already demotes
them; they stay in `ir.py` until E-6, clearly documented as
lemma-store-only. `stage.py:_term_decode`'s legacy `term_from_data`
fallback retires on its own timeline (record version gate already
protects it).

---

## 5. Canonical envelope decisions (pick at E-1 ratification)

1. **Leaf record**: `{"Arr": {"dim":…, "value": {"__mlopt_leaf__":
   kind, "name", "shape"}}}` canonical; `__mlopt_leaf__:`-prefixed
   `Const` strings decode-accepted (legacy). Requires R1 or R2 above.
2. **`Op.source`**: emit `"Native"` always (law patterns pin
   `source:"native"`, exact-keyset matching — Foreign("tensor") would
   silently stop laws matching). Decode accepts/ignores any
   `ArrSource`. `{"Foreign":"tensor"}` becomes a legacy producer
   spelling (source-onnx keeps emitting it in raw docs; Python decode
   ignores it; it must never reach the eqsat wire — it can't, since
   the codec re-encodes from view terms which carry no source field).
   Extension point noted: if a future consumer needs Foreign ops, add
   `Op.source: str = "native"` to the view and a codec rule — do NOT
   add it now.
3. **`Op.dim`**: fixed `Const(1)` — not a view field; codec owns it.
   Tree spelling: nested `{"Const":{"value":1}}`; dag spelling: shared
   node index (engine-eqsat schema).
4. **Attrs channel**: `attrs` field canonical (kernel
   `Term::Op.attrs`); `lit.__mlopt_attrs__` decode-legacy only —
   matches WP0b's landed direction.
5. **`__nonfinite__`**: unchanged, recursive over attr/const values.
6. **`shape` on leaves**: `[…]` with `null` for unknown dims; `null`
   whole-shape = `TensorType(None)` wildcard (the hnl_import
   `_WILDCARD_SHAPE` contract).

---

## 6. Migration sequence

Ordering correction to the step-E plan's guess: **`onnx_rust` +
`arch_rust` first** (they're already JSON-native — the "flip" is a
decode-target swap plus an import change; `hnl.py` is NOT simplest —
it requires re-basing `hnl_import`'s 30 construction sites, still
mechanical under an API-compatible view but a bigger diff).

| step | work | repos | gate |
|---|---|---|---|
| **E-2a** | Land `mlopt_core/term.py` + `term_json.py` in eqsat mlopt-core (move ir.py class half verbatim; codec = hnl_codec generalized + `_kdag` budget/dim-index absorbed; `leaf=`/`budget=` params). `ir.py` → shim. `kit_discovery.hnl_codec` → shim. | eqsat, kit-discovery | round-trip property tests ported from `kit-discovery/tests/test_hnl_codec.py`; `cargo`-free pytest green in both repos; ty/ruff clean |
| **E-2b** | Drift guards: (a) `test_term_json_conformance.py` — codec output fed to `eqsat-run` (saturate smoke on a small term set, `MLOPT_EQSAT_RUN` or skip-gated) proving the emitted envelope parses under `dag.rs` rules incl. dim-index + leaf spelling; (b) a pinned-envelope fixture test (exact JSON for a small term — catches silent spelling drift); (c) `liftmap_drift`-style check that `term_json`'s accepted-key set matches the documented envelope. | eqsat (+ engine-eqsat only if R1) | guards committed with the codec |
| **E-3** | `eqsat.py` codec deletion → `mlopt_core.term_json` (profile per §4); `stage.py` imports repoint (payload bytes change → decide RECORD_VERSION: decoder is shape/tolerant so v3 can stay, but bump to v4 if we want emit-side honesty — recommend v4 bump since dim/leaf/source spellings change); kit-mlopt `RECORD_SCHEMA_VERSION` repin. | eqsat, mlopt, kit-prelude | `python -m mlopt stage lift|search|verify|deliver` green + `hnl run` pipeline example + eqsat differential drivers (`eqsat_differential.py`, `eqsat_parity_sweep`) green |
| **E-4a** | `onnx_rust`/`arch_rust`: `from mlopt_core.term_json import term_from_json` (or keep `kit_discovery.hnl_codec` shim — recommend direct import for honesty); `from mlopt_core.ir import …` → `from mlopt_core.term import …`. Everything else unchanged. | mlopt | `test_onnx_rust_source.py`, arch spec tests, one real-model `mlopt optimize` smoke |
| **E-4b** | `hnl.py`: `hnl_import` re-bases imports to `mlopt_core.term` (or rides the ir shim — prefer direct); `hnl.py` adapter import swap. Evaluate `compile-term` overlap per step-E plan but don't block on it. | kit-discovery, mlopt | `test_hnl_source.py` + emit→import round-trip tests |
| **E-4c** | `whisper`/`kokoro`/`mlopt_torch`: `torch_export`/`torch_bridge`/`torch_eval`/`torch_bindings` — under an API-compatible view this is an **import repoint, not a rewrite**; the 2.4k-line bindings table consumes `Op`/attrs identically. Gated on kit-torch split (D-1) as before. | graphopt→kit-torch, mlopt | adapter tests + whisper smoke |
| **E-5/R1** | engine-eqsat `arr`-enode round-trip fix (dag.rs `op_record` `"arr"` arm) → collapse codec `leaf=` profile to canonical; source-onnx `term.rs` leaf_json optionally aligned (its `{"Foreign":"tensor"}` source spelling can stay — decode-ignored). | engine-eqsat, source-onnx | engine-eqsat tests + eqsat differential re-run |

Rollback per step: E-2 is new modules (delete); E-3 is a repoint
(git-revert; v3 decoder tolerance means old records still read); E-4
is per-adapter import swaps.

---

## 7. Test strategy

- **Codec round-trip** (port `kit-discovery/tests/test_hnl_codec.py`
  → `eqsat/packages/mlopt-core/tests/test_term_json.py`): tree/dag,
  `dag=True/False/None`, shared terms, nonfinite floats, tuple/list/
  None attrs, shape=None wildcard, `allow_dag=False` refusal,
  malformed-record refusals, legacy-spelling decode (`lit.
  __mlopt_attrs__`, nested-dim dag, Const-string leaves).
- **Rust conformance** (the drift guard that matters): feed
  `term_to_json` output for a corpus (the synthetic fixture terms +
  one lifted model's root) into `eqsat-run` — assert `ok:true`.
  Skip-gated on `eqsat_binary()` resolution like existing e2e tests.
  This is the only test that proves Python's envelope matches
  `dag.rs`'s strictness (dim-index, leaf tags, attrs channel).
- **Differential**: existing `onnx_lifter_differential.py` (Python
  oracle vs source-onnx) and `eqsat_differential.py`/`eqsat_parity_
  sweep` stay green — they ARE the cross-implementation check.
- **Adapter tests**: `test_onnx_rust_source.py`, `test_hnl_source.py`,
  `test_stage.py`, `test_emit_surface.py` — all construct view terms
  post-migration; the drift surface is identical to today.
- **Census freeze** (E-1 deliverable): checked-in table of allowed
  `mlopt_core.ir` import sites — the D5 scoreboard.

---

## 8. Honest risks

- **R1 — Option B is *nominal* Term-native.** The Python objects remain
  Python objects; the kernel `Term` exists only at the eqsat-run JSON
  boundary. D5's measurable criterion ("Sources/Sinks produce/consume
  kernel Terms") is satisfied in the *serde* sense — one type, one
  wire — not the object sense. If the intent is literal kernel Terms
  in Python, only option A (PyO3 `hnl_engine::Term` binding) delivers
  it, and B becomes the view A replaces later (the plan's stated
  sequencing). State this in the E-1 ratification so D5 isn't claimed
  "done" on codec grounds alone.
- **R2 — Envelope drift vs the Rust parser.** serde ignores unknown
  fields but requires present ones; the dangerous drifts are
  *semantic*: dim-as-index vs nested (Rust strict, Python tolerant —
  a Python emitter regressing to nested-dim dag would pass Python
  tests and fail only at eqsat-run → the §7 conformance test is the
  guard, not optional); `source` spelling (law matching is
  exact-keyset — a Foreign("tensor") leak onto the eqsat wire
  silently stops laws firing → codec hardcodes `"Native"`, decode
  ignores source); leaf spelling (Arr-records are input-OK but
  output-fatal until the engine-eqsat `arr` fix — profile R2 keeps
  them off the eqsat wire by construction).
- **R3 — `term_to_data` dialect collision.** Three things named
  `dag.program` exist: kernel envelope (`{"Opaque":{"__tag__":…}}`),
  term_to_data (`{"__tag__":…}` top-level, `{"op":…}` nodes), and
  stage v≤2 payloads. The codec module docstring must name the
  boundary; `_HNL_CODEC_KEYS`-style shape dispatch in `stage.py`
  already separates them — keep it.
- **R4 — `Op.make` interning is load-bearing.** `_has_sharing`/dag
  dedup rely on structural sharing being object sharing. The view
  MUST keep `_INTERN` (or the codec must dedup by content key —
  recommended anyway; note `Const`/`Var`/`Param` are also
  content-hashable so the whole term is a valid dict key).
- **R5 — `validate_attrs` coupling.** `Op.make(validate=True)`
  consults `ATTR_SCHEMA`/`ATTR_REQUIRED` — law/mint behavior depends
  on it. The view keeps `mlopt_core.attrs` as-is (same package, no
  move). Do not "simplify" validation away.
- **R6 — kit-mlopt schema pin.** If RECORD_VERSION bumps to v4,
  kit-prelude's `kit-mlopt` `RECORD_SCHEMA_VERSION` must repin in the
  same change window (cross-repo contract; the version gate protects
  old records but the pin is the honesty mechanism).
- **R7 — hnl_codec shim import cycles.** `kit_discovery` already deps
  mlopt-core — the shim direction (kit_discovery → mlopt_core) is
  safe; never import `kit_discovery` from mlopt-core (that IS the
  cycle that created `_kdag`).
- **R8 — e-graph cost of Arr-leaf records.** If R1 lands and Arr
  records go over the eqsat wire, leaves become `arr` op enodes (dim
  child + value attr) instead of `Term::Leaf` — slightly more e-nodes
  and a rewrite surface laws could theoretically match (none target
  `"arr"` today). If e-graph size becomes measurable, Const-string
  leaves are the cheaper spelling — keep the profile parameter until
  measured, don't pre-optimize.
