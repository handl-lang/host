---
title: Step E tail plan — remaining Term-JSON codec migration after plan B (eqsat v0.1.12)
type: report
date: 2026-11-17
status: audit complete — read-only; no source changes made
scope: ordered codec-site migration, wire-dialect matrix, deprecation schedule, effort/dependency graph, D-2 interaction
---

# Step E tail plan — Term-JSON codec migration after plan B

Plan B landed in `eqsat` at `d67d1fc` / tag `v0.1.12`
(`mlopt_core.term` + `mlopt_core.term_json` + the `mlopt_core.ir`
compat shim + the `mlopt_orchestrator.eqsat` delegation). This report
audits every remaining codec site, decides the wire dialect each
consumer should emit/accept, schedules the deprecations, and sequences
the work against the D-1e/D-2 splits.

**Bottom line.** The codec consolidation itself is done and proven
(eqsat fast lane green incl. live `eqsat-run` e2e). What remains is a
*consumer* migration, not a codec migration: one shim
(`kit_discovery.hnl_codec` — already drafted, uncommitted, in
`/home/luis/tmp/kit-discovery-stepe`), a handful of decode-target
swaps in `mlopt`, mechanical `mlopt_core.ir → mlopt_core.term`
repoints in the view-class consumers, one engine-binary rebuild gate
for the `Arr` leaf profile, and a D-2 timing decision for the `mlopt`
product files. Nothing here requires new codec code.

---

## 1. Verified current state

| repo | HEAD | relevant state |
|---|---|---|
| `eqsat` | `d67d1fc` = `v0.1.12` | `term.py` (757 ln, the ir.py class half), `term_json.py` (682 ln, the one codec), `ir.py` (265 ln shim + lemma-store codec), `eqsat.py` quartet → thin delegates (`eqsat.py:382-432`) |
| `kit-discovery` | `7d507c5` (main, ahead 3 unpushed) | `hnl_codec.py` still the 479-line implementation on `main`; the shim exists **uncommitted** in worktree `/home/luis/tmp/kit-discovery-stepe` (detached `fbb61e7`) |
| `discoverykit` | `4464acf` | pins eqsat `v0.1.12`; 17 src files import `mlopt_core.ir` (view classes only, no Term-JSON) |
| `kit-carriers` | `34c1339` | pins eqsat `v0.1.12`, kit-torch `v0.1.2`; 10 src files import `mlopt_core.ir` |
| `kit-onnx` | `084897d` | pins eqsat `v0.1.12`; 6 src files import `mlopt_core.ir`; `lift.py`/`source.py` produce view `IR`, not Term-JSON |
| `kit-host-tensor` | `1f5912a` | pins eqsat `v0.1.12`; 3 src files import `mlopt_core.ir`; `wire.py` is the f64 tensor codec — unrelated to Term-JSON |
| `kit-cuda` | — | pins eqsat `v0.1.12`; no Term-JSON sites found |
| `kernelopt` | `5a1679b` (ahead 1, `pyproject.toml` dirty) | pins eqsat `v0.1.10` in tree; `ir_bridge.py:61` imports `mlopt_core.ir` classes — a structural translator to `lib_kernel_ir`, **not** a JSON codec |
| `mlopt` | `9e2c53e` (ahead 1, `pyproject.toml` + `scripts/train_delivery_prior.py` dirty, untracked `arch-gpt2-small.*`) | pins eqsat `v0.1.10` in tree (dirty pyproject bumps kit-* but not eqsat); adapters decode via `kit_discovery.hnl_codec`; `stage.py` `RECORD_VERSION = 3` |
| `graphopt` | `e4c639b` | still contains `packages/mlopt-torch` (D-1e removal **not** on main); bench audit drivers carry codec copies; D-2 dry-run merge commit `aff8902` exists on a side ref, not main |
| `kit-prelude` | `2d3f265` | `kit-mlopt` `RECORD_SCHEMA_VERSION = 3` (`crates/kit-mlopt/src/lib.rs:92`); version-gates records, never parses term payloads |
| `source-onnx` | `f4cf0e7` | Rust direct wire producer: `Arr` leaves + `attrs` channel + `{"Foreign":"tensor"}` source + `dag.program` with **nested-record `dim`** (`term.rs:241-310`) |
| `engine-eqsat` | `818c9ed` | `dd4815e` `arr_leaf_record` on main; `target/release/eqsat-run` is **stale** (pre-`dd4815e`), `target/debug` is current |
| `kit-torch` | repo = `/home/luis/tmp/kit-torch-work` @ `f0c1bb3a` (tags `v0.1.0`-`v0.1.2`, pins eqsat `v0.1.12`) | **not missing** — it is a real checkout of `handl-lang/kit-torch`; `handl-lang/kit-torch` as a sibling dir does not exist. Graphopt main still ships the pre-rename `packages/mlopt-torch` |

Two findings that update the earlier summary:

1. **The `kit_discovery.hnl_codec` shim is already drafted.** The
   named dry-run agent id `dc06971c` is not resolvable in local repo
   search, but the worktree `/home/luis/tmp/kit-discovery-stepe`
   carries exactly the planned change, uncommitted on detached
   `fbb61e7` — which **predates** `main` `7d507c5` (the D-1e cascade
   commits), so landing means a real branch + rebase, not a fast-forward: `hnl_codec.py`
   479 → 63 lines (pure re-export shim), `tests/test_hnl_codec.py`
   updated for canonical spellings, `pyproject.toml` eqsat pins →
   `v0.1.12` **plus** a `[tool.uv] override-dependencies` block whose
   own comment says *"NOT for landing as-is — the real bump waits on
   the graphopt repin."* `hnl_import.py` is untouched (still
   `mlopt_core.ir` — fine, the shim covers it).
2. **`graphopt/packages/mlopt-torch` is still live on main** and the
   D-1e removal is an uncommitted mass-rename in
   `/home/luis/tmp/graphopt-d1e` (hundreds of `M` files). The canonical
   torch package for migration purposes is the `kit-torch` repo
   (`kit_torch.*`), not the graphopt leftover.

---

## 2. Ordered migration sequence

Ordering rule: codec availability first (done), then the shim, then
consumers in increasing risk order, with the engine-binary rebuild
running in parallel throughout. Steps marked **[parallel]** have no
hard dependency on each other.

### Step 0 — prerequisites (parallel, mostly done)

- **eqsat `v0.1.12` tag exists** — `term.py`/`term_json.py`/`ir.py`
  shim are published substrate. Done.
- **Per-repo eqsat repins to `v0.1.12`** where still `v0.1.10`:
  `kit-discovery` (in the worktree), `mlopt`, `kernelopt`, `graphopt`.
  `discoverykit`, `kit-carriers`, `kit-onnx`, `kit-host-tensor`,
  `kit-cuda`, `kit-torch` already pin `v0.1.12`. **[parallel]** — each
  repo's own `pyproject.toml`; the kit-discovery worktree shows the
  cascade is not clean (its `override-dependencies` workaround exists
  because transitive pins still propagate `v0.1.10` — resolve by
  landing the graphopt/mlopt-torch repin, not by keeping the
  override).
- **engine-eqsat release rebuild** — `cargo build --release` in
  `engine-eqsat` so `target/release/eqsat-run` includes `dd4815e`;
  re-run the dry-run's `Arr`-leaf conformance probe against the
  *release* binary. **[parallel]** — independent of all Python work;
  gates only the `const-str → arr` flip in Step 5.

### Step 1 — land the `kit_discovery.hnl_codec` shim **[after kit-discovery eqsat repin]**

The worktree draft is the right shape:

```python
from mlopt_core.term_json import TermJsonError as HnlCodecError
from mlopt_core.term_json import term_from_json, term_to_json
__all__ = ["HnlCodecError", "term_from_json", "term_to_json"]
```

(`/home/luis/tmp/kit-discovery-stepe/src/kit_discovery/hnl_codec.py:51-63`)

Review/landing checklist:

- **Signature compatibility holds.** Old callers use
  `term_to_json(term, *, dag=None)` and `term_from_json(data, *,
  allow_dag=True)`; `term_json` accepts all of those plus
  `leaf=`/`budget=`/`validate=` with defaults. No aliasing needed.
- **Emit spelling changes** (this is the point): `source:
  {"Foreign":"tensor"}` → `"Native"`, `lit.__mlopt_attrs__` →
  `attrs`, dag `Op.dim` nested record → node-table index, leaf stays
  structured `Arr`. Decode accepts every old spelling, so all
  `term_from_json` consumers are unaffected; only *raw-JSON* readers
  see a difference (audited in §4 — none found that breaks).
- **Test deltas already made** in the worktree: `lit.__mlopt_attrs__`
  → `attrs` assertions, dag node count 6 → 7 (shared `Const(1)` dim
  node), `dim` now asserted as an index, error-message matches
  updated (`unencodable`→`wireable`, `bad leaf encoding`→
  `undecodable`). Grep consumers for those message strings before
  landing — error *text* is not API, but tests/scripts may match it.
- **Do not land the `override-dependencies` block.** Land the real
  eqsat `v0.1.12` source bump once the transitive cascade allows;
  the override is a worktree-local resolution hack.
- `hnl_import.py` stays on `mlopt_core.ir` for now (shim covers it);
  its repoint to `mlopt_core.term` is Step 3b, not a shim blocker.

Gate: `uv sync --group dev && .venv/bin/python -m pytest tests/
test_hnl_codec.py -q`, ruff/ty/lint-imports per kit-discovery AGENTS.

### Step 2 — `mlopt` adapter decode swaps **[parallel with Step 1 once mlopt pins eqsat v0.1.12]**

These are decode-target swaps, not rewrites — `term_from_json` returns
the same view classes either way.

| file | change | notes |
|---|---|---|
| `mlopt/src/mlopt/adapters/onnx_rust.py:580-591` | `from kit_discovery.hnl_codec import term_from_json` → `from mlopt_core.term_json import term_from_json`; `from mlopt_core.ir import IR, Op, Param, TensorType, Var` (line 67) → `mlopt_core.term` | decodes `doc["root"]` from `source-onnx`; `_sanitize_param_names` (line 325) is raw-JSON and **Arr-leaf-only** — see §4 |
| `mlopt/src/mlopt/adapters/arch_rust.py:447-461` | same decode swap; reuses `_onnx._sanitize_param_names`/`_census` | same Arr-only caveat |
| `mlopt/src/mlopt/adapters/hnl.py:47,280,395` | `from mlopt_core.ir import …` → `mlopt_core.term`; `hnl_import` call unchanged | depends on Step 3b for full honesty, but the `ir` shim makes this safe today |
| `mlopt/src/mlopt/stage.py:169-190` | `_term_encode`/`_term_decode` lazy imports `kit_discovery.hnl_codec` → `mlopt_core.term_json`; legacy `term_from_data` fallback (line 187) stays on `mlopt_core.ir` | see §5 RECORD_VERSION decision |
| `mlopt/src/mlopt/emit.py:115` | `from mlopt_core.ir import Const, Op, Param, Var` → `mlopt_core.term` | **not a codec site** — emit.py walks view terms to `.hnl` surface; repoint is hygiene only |

Recommend importing `mlopt_core.term_json` **directly** in the
adapters rather than riding the `hnl_codec` shim: `mlopt-core` is
already a declared dep, it removes the codec dependency on
kit-discovery for JSON decode, and it makes the emitted/accepted
dialect explicit at the call site. (`hnl.py` still needs
`kit_discovery.hnl_import` for `.hnl` *surface* import — that dep
stays regardless.)

Gate: `test_onnx_rust_source.py`, `test_hnl_source.py`,
`test_stage.py`, `test_emit_surface.py`, `test_emit.py`,
`test_emit_embedding.py`, one real-model `mlopt optimize` smoke.

### Step 3 — view-class import repoints `mlopt_core.ir → mlopt_core.term` **[parallel across repos; defer bulk to E-6]**

The shim makes every one of these optional *today*. Do them lazily —
only in files already being touched, or in a dedicated E-6 census pass.
Exception: repoint the **construction** sites first (they define the
canonical spelling of "who builds terms").

| repo | construction sites (class-a) | consumer sites (class-b) | effort |
|---|---|---|---|
| `kit-discovery` | `hnl_import.py:49` (imports `Const, Op, Param, TensorType, Var`; ~30 `Op.make`/`Var`/`Param`/`Const` construction sites at lines 378-1025) | many proposer/oracle modules | M — mechanical, API-identical |
| `kit-torch` (`/home/luis/tmp/kit-torch-work`) | `torch_export.py:23` (torch.export → IR), `torch_bindings.py:29,2445,2461` (`_OP_REGISTRY` private — lives in `term.py`, re-exported by `ir.py:22`) | `torch_bridge.py:48`, `torch_eval.py:19,695-797`, `adapters.py:25`, `calibrate.py:319,366`, `folds.py:23`, `meta_eval.py:18`, `region_fuse.py:66`, `executors/base.py:35` | M — ~10 src files; **no Term-JSON anywhere** (torch_export builds objects, TorchSource/Sink consume objects) |
| `graphopt/packages/mlopt-torch` | same files, old name | same | **skip** — slated for removal by D-1e; migrate `kit-torch` only |
| `kernelopt` | `ir_bridge.py:61` (recognizer/translator → `lib_kernel_ir`) | `play.py`, `board.py`, `scan.py`, `sink.py`, `patterns.py`, `cost.py`, `laws.py`, `priorart.py` | XS for ir_bridge; rest defer |
| `kit-onnx` | `lift.py:56` (ONNX → IR Source), `liftmap.py:58`, `lower.py:47` | `source.py:36`, `sink.py:24`, `opmap.py:56` | S — 6 files |
| `kit-carriers` | carrier lowerings construct `Op`s | 10 files | M or defer |
| `kit-host-tensor` | `executor.py:24`, `sink.py`, `bindings.py` | 3 files | XS or defer |
| `discoverykit` | proposers construct terms | 17 files (several lazy imports) | M or defer; torch-free contract unaffected (`term.py` is torch-free) |

Tests across all these repos still `from mlopt_core.ir import …`
(hundreds of sites) — **leave them on the shim**; test imports are not
migration-critical and the shim is the compat surface.

### Step 4 — `graphopt` bench/audit codec copies **[parallel; after Step 1 for the import swap]**

- `bench/drivers/audit/onnx_lifter_differential.py:97-100` — imports
  `kit_discovery.hnl_codec.term_to_json/term_from_json` (Python oracle
  side) and `mlopt_core.ir` types. Swap to `mlopt_core.term_json` /
  `mlopt_core.term`. Its `_canonicalize`/`_add_record` already handles
  `attrs` **and** `lit.__mlopt_attrs__`, ignores `source`, and handles
  tree + both dag `dim` spellings — canonical emit keeps parity.
- `bench/drivers/audit/eqsat_differential.py:299-417` — carries its
  **own** `_kterm`/`_pyterm` codec emitting `const-str`
  `__mlopt_leaf__:`/`__var__:` payloads for `eqsat-run`. This is an
  engine-profile codec, not Term-JSON; porting to
  `term_json.term_to_json(…, dag="tree", leaf="const-str")` requires
  the fixtures' leaves to be real `TensorType`s (the driver's
  `Var(name, "f64")` shorthand would fail `_shape_data`). Either type
  the fixtures and port, or document it as a deliberate engine-wire
  exception. `eqsat_shape_guards.py` reuses it.
- `bench/drivers/probes/term_ir_spike.py:73-136` — historical spike
  codec emitting `{"Foreign":"tensor"}` + `lit.__mlopt_attrs__` +
  `Arr`; probe-only, not shipped. Retire or leave; do not extend.

### Step 5 — eqsat `const-str → arr` leaf-profile flip **[gated on Step 0 engine rebuild]**

Once the **release** `eqsat-run` binary contains `dd4815e` and the
conformance probe passes:

- `eqsat.py:391-393,401-403,416-418` — change `leaf="const-str"` →
  `leaf="arr"` (or drop the profile arg once `const-str` is retired);
  `_kterm`/`_kdag`/`_terms_wire` wrappers stay or move to public API
  (decide whether `test_eqsat_engine.py` keeps pinning private names).
- Keep `term_from_json` accepting `const-str` **permanently** — old
  records and any not-yet-rebuilt consumer binaries still emit it.
- Re-run `test_eqsat_engine.py` real-binary tests +
  `eqsat_differential.py`/`eqsat_parity_sweep`.

This is the only step that touches the engine wire; everything before
it is Python-side and safe with the stale binary because `const-str`
decodes everywhere.

### Step 6 — optional `source-onnx` alignment **[not blocking]**

`source-onnx/src/term.rs` already emits canonical `Arr` leaves +
`attrs`. It still emits `source: {"Foreign":"tensor"}` (lines 262,295)
and nested-record `dim` inside `dag.program` nodes (line 289) — both
decode-ignored by `term_json`, and the nested `dim` would actually be
**rejected** by `engine-eqsat`'s `dag.rs` `add_op_node` (which requires
a dim *index*) if a `dag.program` record ever reached `eqsat-run`
directly. Today it cannot: `onnx_rust`/`arch_rust` decode to view
terms and the eqsat bridge re-encodes via `term_json` (dim-index,
`"Native"`). Aligning source-onnx to `"Native"` + dim-index is
cosmetic canonical purity; do it only if a future consumer feeds
source-onnx records to the engine without the Python re-encode, or as
part of the `Foreign` deprecation below.

---

## 3. Wire-dialect decision matrix

Profiles:

- **canonical** = `term_json.term_to_json` defaults: `leaf="arr"`,
  `source:"Native"`, `attrs` field, tree `dim`=`Const(1)` record, dag
  `dim`=node index, `dag=None` auto tree/dag.
- **eqsat-transitional** = `leaf="const-str"`, otherwise canonical.
  Required only on the `eqsat-run` wire until the release binary is
  rebuilt.
- **legacy-hnl** = old `hnl_codec` emit: `source:{"Foreign":"tensor"}`,
  `lit.__mlopt_attrs__`, `Arr` leaves, dag `dim` nested record.
  Decode-accepted forever; no longer emitted by Python.
- **source-onnx** = `Arr` leaves + `attrs` + `Foreign("tensor")` +
  nested-record `dim` in dag nodes (a hybrid: canonical leaf/attrs,
  legacy source/dim).

| producer / consumer | role | tree/dag | leaf | source | attrs | dim (dag) | verdict |
|---|---|---|---|---|---|---|---|
| `mlopt_orchestrator.eqsat` → `eqsat-run` | emit + decode | dag records (`terms_to_json`) + `dag="tree"` for `_kterm` | **const-str** until release rebuild, then arr | `"Native"` | `attrs` | index | emit const-str now; flip on Step 5; decode auto |
| `engine-eqsat` (`dag.rs`) | decode + emit | dag.program + tree | accepts `Arr` (post-dd4815e) and `Const`-str leaves; emits `Arr` leaf records via `arr_leaf_record` | requires `source` on Op input; emits native/foreign/judgment/dynamic | `attrs`→`__attrs__`, `lit`→`lit` | requires dim **index** in dag nodes | rebuilt binary = canonical; stale = const-str only |
| `kit_discovery.hnl_codec` (shim) | re-export | delegates | arr (default) | `"Native"` | `attrs` | index | landed shape = canonical; decode auto |
| `mlopt.adapters.onnx_rust` / `arch_rust` | decode only (`doc["root"]`) | accepts both | Arr (source-onnx) | ignores | accepts both | accepts both | decode `term_json.term_from_json`; raw pre-pass `_sanitize_param_names` is Arr-only |
| `source-onnx` `term.rs` | emit (Rust, direct wire) | tree or dag.program | **Arr** | `{"Foreign":"tensor"}` | `attrs` | **nested record** (legacy) | keep as-is; Python decodes all of it; never feeds engine directly |
| `mlopt.stage` records | emit + decode | both (auto) | **arr** (must NOT be const-str — old `hnl_codec` would misdecode const-str leaves as `Const` strings) | `"Native"` | `attrs` | index | canonical emit; decode union incl. `term_to_data` for v≤2 |
| `kit-prelude` `kit-mlopt` | forward + version-gate | opaque | opaque | opaque | opaque | opaque | no dialect; only `v` |
| `kit_onnx.lift`/`source`/`lower`/`sink` | view-object producer/consumer | n/a (no JSON) | n/a | n/a | n/a | n/a | repoint imports only |
| `kit_torch.torch_export`/`torch_bridge`/`torch_eval` | view-object producer/consumer | n/a | n/a | n/a | n/a | n/a | repoint imports only |
| `kernelopt.ir_bridge` | view→`lib_kernel_ir` translator | n/a | n/a | n/a | n/a | n/a | repoint import only |
| `kit_carriers` / `kit_host_tensor` / `discoverykit` | view consumers | n/a | n/a | n/a | n/a | n/a | optional repoint / ride shim |
| `graphopt onnx_lifter_differential` | decode + canonicalize | both | Arr (oracle default) | ignored | both channels merged | both accepted | swap import to `term_json`; keep oracle on `leaf="arr"` |
| `graphopt eqsat_differential` | emit + decode (engine wire) | tree (`dag="tree"`-equivalent) | const-str + `__var__:` fallbacks | `"Native"` | `lit.__mlopt_attrs__` | n/a (tree) | engine-profile exception; port or document |
| `graphopt term_ir_spike` | spike emit/decode | tree | Arr | `Foreign` | `lit` | n/a | historical; do not extend |

Hard rules the matrix encodes:

1. **`Foreign("tensor")` must never reach the eqsat wire.** Law
   patterns pin `source:"native"` under exact-keyset attr matching; a
   Foreign op silently stops laws firing. `term_json` hardcodes
   `"Native"` on emit and ignores `source` on decode, and view terms
   carry no source field — so the leak is impossible through the
   codec. `source-onnx`'s Foreign spelling is fine *only* because its
   records are decoded to view terms before any eqsat encoding.
2. **`const-str` leaves must never appear in stage records** — the
   pre-shim `hnl_codec` decoder (still deployed in older mlopts)
   reads a `Const`-string leaf payload as a scalar `Const`, silently
   losing `Var`/`Param` typing. Stage emit stays `leaf="arr"`.
3. **Decode tolerance is permanent** for `const-str`, `Foreign`,
   `lit.__mlopt_attrs__`, and nested-record dag `dim` — retained
   records and not-yet-rebuilt binaries still produce them.

---

## 4. Raw-JSON consumer audit (the pre-decode walkers)

These read the wire *before* `term_from_json` and are therefore
spelling-sensitive even though the decoder is tolerant:

- **`mlopt/src/mlopt/adapters/onnx_rust.py:325-356`
  `_sanitize_param_names`** — walks `doc["root"]` in place renaming
  `param` leaves to `setattr`-safe identifiers. It recognizes **only**
  the structured spelling `{"Arr": {"value": {"__mlopt_leaf__":
  "param", "name": …}}}` (line 343-351). A `const-str` leaf
  (`{"Const": {"value": "__mlopt_leaf__:{…}"}}`) would be walked but
  not renamed → unsafe `setattr` names downstream in
  `kit_torch.torch_eval.IRModule`. **Today this is safe** because
  `source-onnx` emits `Arr` leaves. Constraint: keep `source-onnx` on
  `Arr`, or extend the sanitizer to the const-str payload, before any
  producer feeding these adapters switches leaf profile.
- **`graphopt/bench/drivers/audit/onnx_lifter_differential.py`** —
  canonicalizes raw records for Python-vs-Rust parity: merges
  `attrs`/`lit.__mlopt_attrs__`, ignores `source`, accepts tree/dag.
  It does **not** understand `const-str` leaf payloads (would compare
  as scalar `Const`). Keep the Python oracle on `leaf="arr"` (the
  `term_json` default) and parity holds across the `source-onnx`
  hybrid spelling.
- **`graphopt/bench/drivers/audit/eqsat_differential.py:299-417`** —
  private codec emitting `const-str`/`__var__:` payloads *to*
  `eqsat-run`; correct for the stale release binary, but it is a
  third codec copy. Port to `term_json` (requires typed leaves) or
  mark as a deliberate engine-wire exception.
- **`kit-prelude/crates/kit-mlopt/src/lib.rs:101-…` `check_record_schema`**
  — reads only `rec["v"]`; never inspects `ir`/`term` payloads.
- **`hnl` pipeline scripts** (`kit-prelude/examples/pipeline/`) —
  thread records opaquely; the only payload touch is top-level
  object-spread (`{...r, "out": …}`). No term-spelling inspection.
- **`mlopt/stage.py:166` `_HNL_CODEC_KEYS`** — shape-dispatches
  Term-JSON vs legacy `term_to_data` on top-level keys
  `{Op, Arr, Const, Opaque}`; unchanged by the spelling migration.

No raw consumer was found that *requires* the legacy
`Foreign`/`lit`/nested-dim spelling — the emit-side change is safe.

---

## 5. `RECORD_VERSION` analysis (`mlopt/stage.py` + kit-prelude)

Facts:

- `mlopt/src/mlopt/stage.py:135` `RECORD_VERSION = 3`; `:333` accepts
  integer `v` up to 3, refuses higher; `:905/915/923` stamps `v` on
  every emitted record.
- `kit-prelude/crates/kit-mlopt/src/lib.rs:92`
  `RECORD_SCHEMA_VERSION = 3` mirrors it; `check_record_schema`
  refuses `v > 3` loudly on both stage input and output.
- v3 payloads are Term-JSON (`{Op/Arr/Const/Opaque}`); v0–v2 payloads
  are `term_to_data` (`{op/var/param/const/mvar}`) — still decoded via
  the `ir.py` lemma-store codec (`stage.py:187`).

The codec swap changes *spelling inside* the v3 term payloads
(`Foreign→Native`, `lit→attrs`, dag `dim` nested→index) but **not**
the top-level record schema, and the decode union is compatible in
**both** directions:

- new `term_json.term_from_json` reads every old v3 spelling
  (Foreign/lit/Arr/nested-dim) — verified in `term_json.py:631-682`
  and `_dec_dag`'s nested-`dim` acceptance (`term_json.py:602-613`);
- old `hnl_codec.term_from_json` reads the new canonical v3 spelling
  (ignores `source`/`dim`, accepts `attrs`, decodes `Arr` leaves,
  accepts dag `dim` indices because `_dec_dag` never reads `dim`) —
  verified against `hnl_codec.py:371-444`.

**Decision: keep `RECORD_VERSION = 3`.** This deliberately overrides
the plan's tentative "recommend v4" (`step_e_term_repr_b_plan.md`
E-3 row, `step_e_dryrun.md:65`): a bump buys emit-side honesty but
costs a synchronized `kit-mlopt` repin and hard-fails every
mixed-version deployment (old kit refuses v4 input *and* output),
while buying no decode capability — the v3 decoder already accepts
the union. `v` versions the *record schema* (envelope keys, stage
fields), not the codec's internal spelling; the spelling is
codec-versioned inside `term_json`, which has no version field and
needs none because decode is permanently tolerant.

Bump to v4 **only if** one of these becomes true:

1. a raw-JSON consumer appears that must distinguish canonical from
   legacy spelling (none found — §4);
2. the top-level record shape changes (fields added/removed/renamed);
3. legacy `term_to_data` decode is dropped (then v≤2 handling
   changes — but that is a decode change, not an emit version, and
   retained records still need it; prefer keeping decode).

If v4 is ever chosen: bump `mlopt/stage.py:135` **and**
`kit-prelude …/lib.rs:92` in the same window, update
`stage_stub.py` `vok`/`vfuture` fixtures and `test_stage.py` version
assertions; old kits refuse v4 loudly by design.

`emit.py` is unrelated to `RECORD_VERSION` — it emits `.hnl` surface
artifacts, not stage records.

---

## 6. Deprecation schedule (evidence-gated)

| item | status | retire when | evidence gate |
|---|---|---|---|
| `mlopt_core.ir` term re-exports (shim half) | active compat | all shipped `from mlopt_core.ir import <term names>` migrated to `mlopt_core.term`, **or** remaining sites explicitly documented as permanent compat; census → 0 for non-lemma imports | frozen census (`step_e_ir_census.md`) re-run; `grep -r "mlopt_core.ir" --include=*.py` across consumer repos shows only `term_to_data`/`term_from_data` users + intentional compat; all repos on eqsat ≥ v0.1.12 |
| `mlopt_core.ir.term_to_data`/`term_from_data` (lemma-store codec) | **stays** — not Term-JSON | separate E-6 lemma-store/pattern-wire migration | stage v≤2 decode fallback retired first; lemma stores re-encoded; do **not** bundle with the term shim removal |
| `const-str` leaf profile (emit) | eqsat-transitional | release `eqsat-run` rebuilt with `dd4815e` + repinned; conformance probe green on the *release* binary; `eqsat_differential` green with `leaf="arr"` | `cargo build --release`; `term_to_json(leaf="arr")` → `eqsat-run` `ok:true` on release binary; then flip `eqsat.py` leaf arg |
| `const-str` leaf profile (decode) | permanent | never for retained records | n/a — decoder accepts both forever |
| `{"Foreign":"tensor"}` source spelling | legacy producer spelling | Python already emits `"Native"`; `source-onnx` may align optionally once no consumer keys on `source` (differential ignores it; engine re-encode uses Native) | consumer census shows zero raw `source` inspection; source-onnx alignment is cosmetic — decoder tolerance never removed |
| `lit.__mlopt_attrs__` attrs channel | decode-legacy | canonical `attrs` is the only emit; old records keep decoding | same census gate; decoder tolerance permanent |
| nested-record dag `dim` | decode-legacy | `term_json`/`hnl_codec` emit dim-index; `source-onnx` may align | same; note engine-eqsat **requires** dim-index in dag nodes — nested-dim records are Python-bound only |
| `kit_discovery.hnl_codec` real implementation | already replaced by shim in worktree | n/a — shim is the end state | land Step 1 |
| `RECORD_VERSION` | stays 3 | §5 conditions only | kit-prelude repin if ever bumped |

---

## 7. Effort estimates + dependency/parallelism

| work item | repo(s) | effort | depends on |
|---|---|---|---|
| eqsat codec (term.py/term_json.py/ir.py/eqsat.py) | eqsat | **done** (`d67d1fc`) | — |
| `hnl_codec` shim land + test updates | kit-discovery | **S-M** (draft exists; review + gates) | kit-discovery eqsat `v0.1.12` repin (worktree has override hack — resolve via cascade) |
| adapter decode swaps (`onnx_rust`, `arch_rust`, `stage`, `hnl` import, `emit` repoint) | mlopt | **M** (~5 files + tests) | mlopt eqsat `v0.1.12` repin; Step 1 for hnl_codec shim *if* keeping that import path (direct `term_json` import removes this dep) |
| `hnl_import` term rebase | kit-discovery | **M** (~30 construction sites, mechanical) | eqsat `v0.1.12` (already in worktree) |
| torch view repoint (`torch_export`/`torch_bridge`/`torch_eval`/`torch_bindings`/…) | kit-torch | **M** (~10 src files; tests stay on shim) | eqsat `v0.1.12` (kit-torch already pins it); independent of D-1e merge |
| `ir_bridge` repoint | kernelopt | **XS** | kernelopt eqsat repin `v0.1.10→v0.1.12` (pyproject dirty) |
| kit-onnx import repoint | kit-onnx | **S** (6 files) | already `v0.1.12` |
| kit-carriers / kit-host-tensor / discoverykit repoints | respective repos | **S-M each or defer to E-6** | already `v0.1.12` |
| graphopt bench codec copies (`onnx_lifter_differential`, `eqsat_differential`, spike) | graphopt | **S-M** | Step 1 landed (for import swap); eqsat_differential port needs typed-leaf decision |
| engine-eqsat release rebuild + conformance | engine-eqsat + eqsat tests | **S** | none — parallel throughout |
| eqsat `const-str→arr` flip | eqsat | **XS** (3 call sites + tests) | release rebuild + conformance green |
| source-onnx `Foreign`/nested-dim alignment | source-onnx | **M optional** | none; cosmetic |
| RECORD_VERSION bump | mlopt + kit-prelude | **XS but coordinated** | only if §5 conditions trigger |

**Parallel lanes:** (a) kit-discovery shim + hnl_import rebase;
(b) mlopt adapter/stage swaps (once mlopt repins eqsat); (c) kit-torch
+ kernelopt + kit-onnx + kit-carriers/host-tensor/discoverykit import
repoints; (d) graphopt bench drivers; (e) engine-eqsat rebuild. All
are independent repos/files.

**Serialized chain:** eqsat `v0.1.12` tag → per-repo repins →
kit-discovery shim → (mlopt direct `term_json` imports need only the
mlopt repin; `hnl_codec`-shim consumers need the kit-discovery tag) →
consumer swaps → engine rebuild → `const-str→arr` flip → deprecation
gates.

---

## 8. D-2 merge interaction (`mlopt` → `graphopt/packages/mlopt`)

D-2 dry-run (`aff8902`, worktree `/home/luis/tmp/graphopt-d2-dryrun`)
moves the `mlopt` product into `graphopt`. Codec changes in `mlopt`
(`adapters/*`, `stage.py`, `emit.py`) will be **file-moved** by that
merge. Two clean options:

- **Preferred — land mlopt codec swaps before the D-2 cut.** The
  swaps are small, low-risk, and D-2 then carries final files once.
  mlopt main is already ahead 1 with a dirty cascade pyproject —
  finishing the eqsat `v0.1.12` repin + codec swaps as one commit
  series *before* D-2 execution keeps the merge mechanical. Risk:
  adds surface to an already-messy mlopt working tree.
- **Acceptable — hold mlopt swaps until after D-2** and apply them
  once in `graphopt/packages/mlopt/...`. Avoids touching mlopt twice,
  but the edits then land on post-merge paths and must be coordinated
  with whatever the merge resolution did to those files.

Do **not** do both. Recommendation: if D-2 execution is imminent,
hold and apply post-merge in `packages/mlopt`; otherwise land the
Step-2 mlopt swaps on mlopt main first. The kit-discovery, kit-torch,
kernelopt, kit-*, source-onnx, engine-eqsat, and graphopt-bench work
is **unaffected** by D-2 and should proceed in parallel regardless.

Related: D-1e (`mlopt-torch` → `kit-torch`) is itself uncommitted in
`/home/luis/tmp/graphopt-d1e` and `graphopt` main still ships
`packages/mlopt-torch`. Migrate **kit-torch only**; do not patch the
graphopt leftover — it is deletion-bound. If D-1e lands first,
graphopt's `mlopt_core.ir` import count drops by ~20 sites for free.

---

## 9. Verification / evidence plan

Per-repo gates follow each AGENTS file (uv sync, pytest, ruff, ty,
lint-imports; kit-prelude/source-onnx/engine-eqsat use cargo gates).

- **Codec unit/property**: `eqsat/tests/test_term_json.py` (ported
  `test_hnl_codec.py` suite — already landed); kit-discovery
  `test_hnl_codec.py` worktree updates must go green.
- **Rust conformance (the guard that matters)**: feed
  `term_json.term_to_json` output — canonical `Arr`/`"Native"`/`attrs`/
  dim-index — into the real `eqsat-run` for a small term corpus;
  assert `ok:true`. Skip-gated on binary resolution. Must be re-run
  against the **release** binary before the `const-str→arr` flip.
- **Pinned-envelope fixture**: exact-JSON test for a small term to
  catch silent spelling drift.
- **Accepted-key/drift guard**: assert `term_json`'s accepted key set
  matches the documented envelope (cf. `liftmap_drift.rs` pattern).
- **Stage**: `test_stage.py` round-trips incl. a v3 record carrying
  **legacy** spellings (`Foreign`/`lit`/nested-dim/const-str leaf)
  proving decode tolerance; `python -m mlopt stage
  lift|search|verify|deliver` + `kit-prelude` pipeline example green.
- **Adapters**: `test_onnx_rust_source.py`, `test_hnl_source.py`,
  `test_emit_surface.py`, `test_emit.py`, `test_emit_embedding.py`,
  arch-spec tests; one real-model `mlopt optimize` smoke.
- **Differential**: `onnx_lifter_differential.py` (Python oracle vs
  source-onnx) and `eqsat_differential.py`/`eqsat_parity_sweep`
  green — these are the cross-implementation parity checks.
- **Deprecation gates**: census re-run → 0 non-lemma `mlopt_core.ir`
  imports before shim removal; release-binary conformance before
  `const-str` emit retirement.

---

## 10. Open questions / blockers

1. **kit-discovery shim landing mechanics** — the worktree draft is
   uncommitted on a detached `fbb61e7` with a not-for-landing
   `override-dependencies` hack. Needs a real branch, the eqsat
   `v0.1.12` repin resolved through the cascade (not the override),
   and the kit-discovery gates. The named agent id `dc06971c` is not
   resolvable locally — treat the worktree as the reference.
2. **mlopt/kernelopt/graphopt eqsat `v0.1.10 → v0.1.12` repins** —
   required before `mlopt_core.term`/`term_json` imports resolve;
   mlopt and kernelopt pyprojects are already dirty with partial
   cascades.
3. **engine-eqsat release binary** — stale; rebuild + repin gates the
   `const-str→arr` flip only.
4. **D-1e/D-2 landing order** — decides whether mlopt codec swaps land
   on mlopt main or once in `graphopt/packages/mlopt`; kit-torch vs
   graphopt `packages/mlopt-torch` duplication resolves with D-1e.
5. **`eqsat_differential.py` codec** — port to `term_json` (needs
   typed-leaf fixtures) vs documented engine-wire exception.
