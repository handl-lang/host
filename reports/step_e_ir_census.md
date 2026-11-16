---
title: E-1 census freeze — every `mlopt_core.ir` import site org-wide (the D5 scoreboard)
type: report
date: 2026-11-17
status: frozen census for step_e_term_repr_b_plan.md §7 / ADR-0178 D5
method: ripgrep `from mlopt_core.ir import|from mlopt_core import ir|import mlopt_core.ir` + `mlopt_core.ir.` across /home/luis/GitHub/handl-lang/* and /home/luis/tmp/kit-torch-work; multi-line import blocks read for exact name lists; `mlopt_core.ir.` qualified uses verified to be docstrings/comments only.
classes: (a) term-construction · (b) term-consumption/walk · (c) lemma-store codec only (term_to_data/term_from_data — does NOT migrate to term_json) · (d) tests · (e) docs/comments/non-Python
---

# E-1 census freeze — `mlopt_core.ir` import sites

**757 import statements across ~330 files** in 12 repos/worktrees. `ir.py` itself
(`eqsat/packages/mlopt-core/src/mlopt_core/ir.py`, 956 lines) is the definition —
it has no self-import and stays as the compat shim per plan §2.

## Legend / classification rules applied

- **(a)** adapters, codecs, and `Source`-port producers that *construct* terms
  (`hnl_import`, `hnl_codec`, the mlopt `*.py` adapters, `torch_export`).
- **(b)** everything else in shipped `src/` that walks/consumes terms — laws,
  cost, typing, egraph, emit, sinks, bindings, bench drivers, scripts, examples.
- **(c)** sites whose *only* codec imports are `term_to_data`/`term_from_data`
  (+`attr_to_data`/`attr_from_data`) — the lemma-store/pattern wire; these do
  NOT migrate to `term_json`. Mixed sites are tagged `b+c`.
- **(d)** `tests/` (and `tests/`-adjacent helpers like `egglog_oracle.py`).
- **(e)** docs, AGENTS.md, `.ts`/`.mjs`, Rust comments — excluded from the
  migration set (enumerated at the end).

Bench drivers/suites/probes are class **(b)** — they construct terms only to
feed engines/costs; they ride whatever the substrate view becomes and need no
separate migration, but they are counted because they pin spellings.

---

## eqsat

### packages/mlopt-core/src/mlopt_core — the substrate itself (38 files)

The shim (`ir.py → from mlopt_core.term import *`) keeps every one of these
working unchanged; they are the *internal* users D5 must not break.

| file | lines | imports | class |
|---|---|---|---|
| `__init__.py` | 16 | IR, Const, Op, Param, TensorType, Var | b (re-export facade) |
| `typing.py` | 62 | Const, Op, Param, Var | b |
| `traversal.py` | 38 | Op | b |
| `pipeline.py` | 59 | IR (TYPE_CHECKING) | b |
| `ports.py` | 154 | IR, Op (TYPE_CHECKING) | b |
| `oracle.py` | 38 | Const, Op, Param, Var | b |
| `opdata.py` | 44 | Op | b |
| `meta.py` | 107, 1072 | Const, Op, Param, Var, op_repr; TensorType | b |
| `features.py` | 22 | Op, Param, `_dag_postorder` | b |
| `trajectories.py` | 21 | op_repr | b |
| `rulecache.py` | 62 | attr_from_data, attr_to_data, term_from_data, term_to_data | **c** |
| `cells.py` | 53 | Const, Op, Param, Var, term_from_data, term_to_data | b+c |
| `cost/basic.py` | 17 | Op | b |
| `cost/executor.py` | 14 | Op, Param | b |
| `cost/params.py` | 18 | Const, Op, Param | b |
| `cost/backend.py` | 13 | Op | b |
| `cost/fusion.py` | 15 | Op | b |
| `cost/roofline.py` | 17 | Op | b |
| `egraph/types.py` | 10 | Const, Op, Param, Var, op_repr | b |
| `egraph/terms.py` | 12 | Op, op_repr_dag | b |
| `egraph/extract.py` | 27, 254 | Op, Var; op_repr | b |
| `egraph/match.py` | 45 | Op | b |
| `egraph/core.py` | 33 | Op | b |
| `egraph/certs.py` | 8 | attr_from_data, attr_to_data, term_from_data, term_to_data | **c** |
| `egraph/proof.py` | 20 | Op | b |
| `laws/tensor.py` | 29 | Const, Op | b |
| `laws/priorart.py` | 62 | Const, Op | b |
| `laws/minted.py` | 137 | Op, Param, TensorType, op_def | b |
| `laws/ruleset.py` | 38 | Op | b |
| `laws/layout.py` | 44 | Op | b |
| `laws/cond.py` | 280 | Const, Op, Param, TensorType | b |
| `laws/serialize.py` | 57 | Const, Op, term_from_data, term_to_data | **c** (lemma-store codec) |
| `laws/pairing.py` | 21, 129, 758, 879 | Op; Var; Param, TensorType ×2 | b |
| `laws/scan.py` | 19 | Const, Op | b |
| `laws/factored.py` | 42 | Param, TensorType, Var | b |
| `laws/attention.py` | 75 | Op | b |
| `laws/headshare.py` | 72 | Const, Op, Param, Var, `_attr_key` | b |
| `laws/specials.py` | 65 | Const, Param, TensorType | b |

(`opmeta.py:664`, `laws/minted.py:65/452/791`, `egraph/types.py:23`,
`meta.py:399`, `opdata.py:135`, `rulecache.py:85`, `egraph/certs.py:166`,
`features.py:63/119`, `egraph/core.py:768`, `egraph/terms.py:197`,
`traversal.py:3`, `laws/serialize.py:9/229` are docstring mentions — class e.)

### packages/mlopt-orchestrator/src/mlopt_orchestrator — 15 files, all (b)

| file | lines | imports |
|---|---|---|
| `optimize.py` | 68 | IR, op_repr |
| `eqsat.py` | 120 | Const, Op, Param, TensorType, Var, attr_from_data, attr_to_data — **the E-3 codec site** (codec quartet `_kterm`/`_kdag`/`_pyterm`/`_pyterm_dag` deleted here) |
| `regime.py` | 92 | IR, Const, Op, Param, Var, op_repr |
| `cache.py` | 20 | IR, Const, Op, Param, Var |
| `bounds.py` | 22 | IR, Op, Param |
| `criteria.py` | 47 | Op, Var |
| `diagram.py` | 77 | Op, Param, TensorType, op_repr_dag |
| `diagram_search.py` | 63 | IR |
| `autotune.py` | 101 | IR, Op, Param, TensorType, Var |
| `crossblock_cse.py` | 56 | Op, Param, op_repr_dag |
| `morphisms_kv.py` | 62 | IR, Const, Op, Param, TensorType, Var, op_repr_dag |
| `morphisms/reify.py` | 22 | IR, Const, Op, Param, Var, op_repr_dag |
| `morphisms/graph.py` | 16 | IR |
| `morphisms/signature.py` | 16 | IR, Op, Param, Var |
| `pricing.py` | 18 | Op |

### packages/mlopt-native/python/mlopt_native — 1 file (b)

| `engine.py` | 51 | Const, Op, Param, Var |

### eqsat/tests — ~100 files, all (d)

~100 test files import `mlopt_core.ir` names (Op/Var/Param/Const/TensorType/op_repr/term_to_data/generator/op_def/_dag_postorder etc.) — enumerated in the source census; they pin spellings, not module paths, and ride the ir.py shim unchanged. Notable: `test_ir.py` (full surface incl. term_to_data/term_from_data), `test_ported_public_api.py:21` (string-literal surface pin — class e, needs update when term.py/term_json.py land).

---

## kit-discovery

### src/kit_discovery — 36 files

| file | lines | imports | class |
|---|---|---|---|
| `hnl_import.py` | 49 | Const, Op, Param, TensorType, Var | **a** (.hnl → term; ~30 construction sites) |
| `hnl_codec.py` | 51 | Const, Op, Param, TensorType, Var, attr_from_data, attr_to_data | **a** (term↔Term-JSON codec → becomes `term_json` shim) |
| `emit.py` | 60, 454, 815 | Const, Op, Param, TensorType, Var (815: +IR) | b |
| `hnl_surface.py` | 63 | Const, Op, Param, Var | b |
| `handl_egraph.py` | 166 | Op | b |
| `census.py` | 54 | Const, Op | b |
| `shipped_primitives.py` | 119 | Const, Op | b |
| `meta_game.py` | 162 | Const, Op, op_repr | b |
| `vocabdir.py` | 720, 1488 | `_OP_REGISTRY` | b |
| `families.py` | 29 | Op, TensorType, Var | b |
| `propose.py` | 53 | Op, TensorType, Var | b |
| `workload_gen.py` | 85 | Const, Op, Param, TensorType, Var, op_repr_dag | b |
| `oracle.py` | 71 | Const, Op, Param, TensorType, Var, op_repr | b |
| `pipeline.py` | 88 | Const, Op, op_repr | b |
| `handl_engine.py` | 55 | Op, attr_to_data | b |
| `gap_gen.py` | 83 | Op, Param, TensorType, Var, op_repr | b |
| `vocab.py` | 77 | Op, TensorType, Var | b |
| `arena.py` | 125 | Const, Op, op_repr, op_repr_dag | b |
| `prim_mint.py` | 50 | Const, Op, op_def | b |
| `forms.py` | 100 | TensorType, Var, op_repr | b |
| `learned_propose.py` | 74 | Const, Op | b |
| `shape_proposal.py` | 62, 353 | Op; op_repr | b |
| `play/gen.py` | 582 | IR | b |
| `play/search.py` | 24 | Const, Op, TensorType, Var | b |
| `play/sites.py` | 18, 82, 2150 | Const,Op,Param,Var; Const; Const,Op | b |
| `play/probes.py` | 13, 37, 166 | Op,TensorType,Var; IR; IR | b |
| `play/torch_arm.py` | 324 | IR | b |
| `play/fragment.py` | 19 | Const, Op, TensorType, Var | b |
| `play/meta_joint.py` | 14 | Op, TensorType, Var | b |
| `model_corpus.py` | 78, 244 | IR; Op | b |
| `open_verdict.py` | 75 | Op | b |
| `open_game.py` | 67 | Op, Var | b |
| `grammar.py` | 46 | Const, Op, op_repr | b |
| `enumerate.py` | 153 | Const, Op, TensorType, Var, op_repr | b |
| `state_reindex.py` | 67 | Op | b |
| `training.py` | 25, 268 | Const,Op,Param,Var,op_repr; TensorType,Var | b |
| `impact.py` | 68 | IR, Op, Param, Var | b |
| `intake/ledger.py` | 25 | Op, TensorType, Var, term_from_data, term_to_data | **b+c** |

(`__init__.py:58`, `hnl_surface.py:13`, `handl_egraph.py:160`,
`shipped_primitives.py:50/273`, `vocabdir.py:706`, `intake/__init__.py:43` —
docstrings, class e.)

### kit-discovery/tests — ~57 files, all (d)

~57 test files (test_discovery_* suite, test_hnl_codec, test_hnl_import-adjacent, test_om_*, test_vocabdir, etc.) — all ride the ir.py shim; `test_hnl_codec.py:18` is the codec round-trip suite that ports to `test_term_json.py` in E-2a.

---

## discoverykit

### packages/discoverykit/src/discoverykit — 17 files, all (b)

`object_synthesis.py` (71,2377) · `evidence.py` (1086) · `proposal_core.py` (96) ·
`index_abstract.py` (64) · `corpus_cov.py` (63) · `handl.py` (53) ·
`genkernel.py` (536,587,1706) · `meta_player.py` (53) · `lemma_cert.py` (539) ·
`law_order.py` (45) · `meta_arena.py` (120,2599) · `verifier.py` (60) ·
`coherence.py` (67) · `antiunify.py` (51,787) · `form_propose.py` (96) ·
`pay_gate.py` (194) · `combinator.py` (63)

### discoverykit/tests — 5 files (d)

`test_antiunify.py` · `test_discovery_handl.py` · `test_index_abstract.py` ·
`test_loop_propose.py` · `test_meta_arena_supported.py`

---

## kit-carriers

### src/kit_carriers — 10 files, all (b)

`om.py` (81) · `trace_lift.py` (112) · `decode_geom.py` (79) ·
`scan_lower.py` (45) · `trace.py` (104) · `decayed_lower.py` (59) ·
`om_lower.py` (51) · `omd_lower.py` (57) · `xcarrier.py` (145) ·
`decode_laws.py` (77)

### kit-carriers/tests — ~40 files, all (d)

Full carrier suite (test_om_*, test_xcarrier*, test_trace*, test_scan_*, etc.).

---

## kit-cuda — 1 file (d)

`tests/test_runners.py` (248, 469, 509, 966 — all `IR, Op, Param, TensorType, Var`). src clean.

## kit-onnx — 6 src files (b) + 1 test (d)

`sink.py` (24: IR) · `opmap.py` (56: Op) · `liftmap.py` (58: Const) ·
`lower.py` (47: IR,Const,Op,Param,Var) · `source.py` (36: IR) ·
`lift.py` (56: IR,Const,Op,Param,TensorType,Var) · `tests/test_onnx_opmap.py` (d)

## kit-host-tensor — 3 src files (b) + 1 test (d)

`sink.py` (30,148,175) · `executor.py` (24) · `bindings.py` (57, lazy Const) ·
`tests/test_host_tensor_sink.py` (d)

---

## kernelopt

### src/mlopt_kernel — 9 files, all (b)

`play.py` (53,432) · `board.py` (36,63) · `scan.py` (58) ·
`sink.py` (34,46) · `patterns.py` (48) · `ir_bridge.py` (61) ·
`cost.py` (23) · `laws.py` (69) · `priorart.py` (63)

### kernelopt/bench — 6 files (b, drivers)

`drivers/overnight.py` (95) · `drivers/screen/novelty_sieve.py` (94) ·
`drivers/screen/priorart_verify.py` (46) · `drivers/probes/kernel_scan_probe.py` (80) ·
`drivers/probes/kernel_vocab_probe.py` (180,196,316) · `bench/discovery/kernel.py` (27)

### kernelopt/tests — 6 files (d)

`test_ir_bridge.py` · `test_kernel.py` · `test_kernel_priorart.py` ·
`test_kernel_sieve.py` · `test_overnight.py` · `test_patterns.py`

---

## mlopt (product repo)

### src/mlopt — 7 files

| file | lines | imports | class |
|---|---|---|---|
| `adapters/onnx_rust.py` | 67 | IR, Op, Param, TensorType, Var | **a** |
| `adapters/arch_rust.py` | 60 | IR | **a** |
| `adapters/hnl.py` | 47 | IR, Op, Param, TensorType, Var | **a** |
| `emit.py` | 115 | Const, Op, Param, Var | b (term → `tensor.*` walk; ~199-op table) |
| `deliver_game.py` | 558 | Op (lazy) | b |
| `carrier_expand.py` | 39 | Op | b |
| `stage.py` | 187, 207 | term_from_data; IR (both lazy) | **c + b** — 187 is the transitional v≤2 `term_from_data` decode (retires with ir); 207 rebuilds `IR` |

(`optimize.py`, `deliver.py`, `cli.py`, `models/` — `IR` appears only in
docstrings; no import sites.)

### mlopt/tests — 7 files (d)

`test_cli.py` (92,187,227,262,297) · `test_emit.py` (6) ·
`test_emit_embedding.py` (14) · `test_emit_surface.py` (35) ·
`test_hnl_source.py` (182) · `test_onnx_rust_source.py` (283) · `test_stage.py` (353)

---

## graphopt

### packages/mlopt-torch/src/mlopt_torch — 10 files (→ kit-torch split)

| file | lines | imports | class |
|---|---|---|---|
| `torch_export.py` | 23 | IR, Const, Op, Param, TensorType, Var | **a** (torch.export → IR construction) |
| `torch_bridge.py` | 48 | IR, Const, Op, Param, TensorType, Var | b |
| `torch_bindings.py` | 29, 2439, 2455 | Op; `_OP_REGISTRY` ×2 | b (2.4k-line bindings table) |
| `torch_eval.py` | 19, 695, 696, 733, 784, 796, 797 | IR,Const,Op,Param,Var + lazy Op/Param/TensorType | b |
| `adapters.py` | 25 | IR | b (TorchSink/TorchSource) |
| `executors/base.py` | 35 | Op | b |
| `region_fuse.py` | 66 | Const, Op | b |
| `folds.py` | 23 | Op | b |
| `meta_eval.py` | 18 | Const, Op, Param, Var | b |
| `calibrate.py` | 319, 366 | IR, Op, TensorType, Var | b |

### graphopt/bench — ~45 files, all (b, drivers/suites)

~45 bench files: referees/components, matrix/runner, discovery/runner,
suites (correctness/cost/evaluation/search/speedup/structure),
limits/hand_carrier, drivers/{overnight,audit/*,bridge/*,mint/*,screen/*,probes/*}
— full per-file line list in the source census (bench drivers pin term
spellings; they ride the substrate view unchanged).

### graphopt/scripts + examples — 2 files (b)

`scripts/dspec_audit.py` (322) · `examples/quickstart.py` (58)

### graphopt/tests — ~62 files, all (d)

The torch/onnx/carrier-bound suite (~62 files incl. test_public_api.py:32 —
string-literal surface pin, class e — and test_real_model_mint.py:64
`from mlopt_core import ir as _ir`).

---

## kit-torch (worktree `/home/luis/tmp/kit-torch-work`, P3 D-1e split — in flight)

Same `mlopt-torch` content post-`git subtree split` — 10 src files identical
to the graphopt table above plus ~50 carved test files (d). Re-grep at
ratification since the split is in flight.

---

## Zero-site repos (verified)

`gamekit` (kernel stdlib-only; `players/neural` lazily imports
`mlopt_core.features`/`trajectories` — **not** `.ir`), `gamekit/packages/benchkit`
(deps on `mlopt-core` but no `.ir` import), `eqsat/packages/mlopt-dtypes`,
`kit-prelude` (Rust + `hnl-ext-mlopt` shim — spawns `python -m mlopt`, no term
types), `host` (docs/reports only).

## Rust / non-Python notes (class e, wire-relevant)

- `source-onnx` (Rust): **emits** the hnl_codec Term-JSON wire
  (`{"Op"/"Arr"/"Const"/"Opaque"}` + `dag.program`, `{"Foreign":"tensor"}`
  source spelling) — it is a *producer of the envelope*, not an
  `mlopt_core.ir` consumer; zero `mlopt_core` imports (only doc comments in
  `src/arch.rs:83,4154`). Relevant to E-5/R1 leaf-spelling alignment.
- `engine-eqsat/src/shape.rs:367`, `src/guard.rs:31,275` — comments noting the
  `attr_to_data`/`attr_from_data` `__tuple__`/`__list__` wire envelopes it
  decodes; no `mlopt_core.ir` dependency (correct — it speaks kernel `Term`).
- `graphopt/packages/mlopt-kit/src/egraph.rs:9` — doc comment: the kit's
  egraph leg uses `handl_discovery`'s `term_to_data`/`term_from_data` codec — the
  **lemma-store dialect**, explicitly not the term wire.
- `eqsat/paper/paper.qmd:96`, `eqsat/README.md:7`,
  `eqsat/docs/site/{index.md,explanation/egraph.md,reference/frozen-api.md}`,
  `graphopt/docs/**` (~15 files), `graphopt/AGENTS.md:642`,
  `graphopt/docs/tutorials/course/**` (component `.ts`/`.html` path tables),
  `docs/.vitepress/sidebar-adrs.mjs:132`, `host-tensor/docs/reports/*.md`,
  `kernelopt/docs/reports/mlopt_ir_kernel_bridge.md` — all class e.

---

## Aggregate counts (files containing ≥1 import site)

| repo | (a) construct | (b) consume/walk | (c) codec-only | (b+c) mixed | (d) tests | total files |
|---|---|---|---|---|---|---|
| eqsat/mlopt-core src | 0 | 34 | 3 | 1 | — | 38 |
| eqsat/mlopt-orchestrator src | 0 | 15 | 0 | 0 | — | 15 |
| eqsat/mlopt-native | 0 | 1 | 0 | 0 | — | 1 |
| eqsat/tests | 0 | 0 | 0 | 0 | ~100 | ~100 |
| kit-discovery src | 2 | 33 | 0 | 1 | — | 36 |
| kit-discovery tests | 0 | 0 | 0 | 0 | ~57 | ~57 |
| discoverykit src | 0 | 17 | 0 | 0 | — | 17 |
| discoverykit tests | 0 | 0 | 0 | 0 | 5 | 5 |
| kit-carriers src | 0 | 10 | 0 | 0 | — | 10 |
| kit-carriers tests | 0 | 0 | 0 | 0 | ~40 | ~40 |
| kit-cuda | 0 | 0 | 0 | 0 | 1 | 1 |
| kit-onnx | 0 | 6 | 0 | 0 | 1 | 7 |
| kit-host-tensor | 0 | 3 | 0 | 0 | 1 | 4 |
| kernelopt src+bench | 0 | 15 | 0 | 0 | — | 15 |
| kernelopt tests | 0 | 0 | 0 | 0 | 6 | 6 |
| mlopt src | 3 | 3 | 0 | 1 (stage.py) | — | 7 |
| mlopt tests | 0 | 0 | 0 | 0 | 7 | 7 |
| graphopt src (mlopt-torch) | 1 | 9 | 0 | 0 | — | 10 |
| graphopt bench+scripts+examples | 0 | ~47 | 0 | 3 | — | ~50 |
| graphopt tests | 0 | 0 | 0 | 0 | ~63 | ~63 |
| kit-torch wip | 1 | 9 | 0 | 0 | ~50 | ~60 |
| **total** | **7** | **~205** | **3** | **6** | **~330** | **~480 files / 757 stmts** |

## Migration-critical set (a+b), ranked

**Class (a) — the only true construction sites (7 files):**
1. `mlopt/src/mlopt/adapters/onnx_rust.py:67` — E-4a first flip
2. `mlopt/src/mlopt/adapters/arch_rust.py:60`
3. `mlopt/src/mlopt/adapters/hnl.py:47` (+ `kit_discovery/hnl_import.py:49` — 30 `Op.make`/`Var`/`Param`/`TensorType` sites, E-4b)
4. `kit-discovery/src/kit_discovery/hnl_codec.py:51` — becomes the `term_json` shim (E-2a)
5. `kit-discovery/src/kit_discovery/hnl_import.py:49` — E-4b
6. `graphopt/packages/mlopt-torch/src/mlopt_torch/torch_export.py:23` (→ kit-torch; E-4c, gated on split)
7. `graphopt/packages/mlopt-torch/src/mlopt_torch/torch_bridge.py:48` (facade; rides the view)

**Class (b) hotspots by import-surface size** (files touching the most
names / private helpers — highest drift risk):
- `eqsat/.../mlopt_orchestrator/eqsat.py:120` — codec quartet + attr codec (E-3 deletes ~370 lines here)
- `eqsat/.../mlopt_core/laws/pairing.py` — 4 sites
- `eqsat/.../mlopt_core/laws/headshare.py:72` — imports private `_attr_key`
- `eqsat/.../mlopt_core/features.py:22`, `egraph/*`, `typing.py` — `_dag_postorder`, `op_repr_dag` users
- `graphopt/.../mlopt_torch/torch_bindings.py:2439,2455` + `kit_discovery/vocabdir.py:720,1488` — `_OP_REGISTRY` private-registry users
- `graphopt/bench/drivers/audit/eqsat_differential.py:73` — the Python side of the eqsat wire (pins `attr_to_data` + leaf spellings)
- `mlopt/src/mlopt/emit.py:115` — the 199-op `tensor.*` emit table (no drift guard today)

**Inside mlopt-core itself:** all 38 src files above are internal users —
the `ir.py` shim (`from mlopt_core.term import *`) keeps them working with
zero edits; `ir.py`'s lemma-store codec (`term_to_data`/`term_from_data`,
`_DAG_TAG`, `_leaf_to/from_data`) stays put per plan §2 — confirmed by the
(c) rows: `rulecache.py`, `egraph/certs.py`, `laws/serialize.py`, `cells.py`,
plus `mlopt/stage.py:187` and the `b+c` bench/driver sites.

**Honest caveat:** ~330 of the 757 statements are in `tests/` — they pin the
*spelling* of terms (op names, attrs, leaf shapes) more than the module path;
the `ir.py` shim means none of them need to move for E-2..E-5, but the
`test_ported_public_api.py`/`test_public_api.py` frozen-surface pins
(string literals listing `mlopt_core.ir` exports) **will** need updating
when `term.py`/`term_json.py` land — they are class-e rows today but are
effectively drift guards on the module layout itself.
