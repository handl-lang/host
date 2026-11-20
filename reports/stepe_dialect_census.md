---
title: Step E dialect census — remaining producers/consumers of the transitional Term-JSON wire dialects
type: report
date: 2026-11-17
status: census complete — read-only; one live conformance probe run against engine-eqsat binaries
scope: every remaining producer/consumer of const-str leaves, Foreign source, nested-record dag dim, lit.__mlopt_attrs__, and direct mlopt_core.term_json bypassers, across /home/luis/GitHub/handl-lang/* (plus the canonical kit-torch checkout at /home/luis/tmp/kit-torch-work and the kit-discovery-stepe worktree)
---

# Step E dialect census

Input to `stepe_tail_plan.md` §6 (deprecation schedule) and
`step_e_dryrun.md` (divergences). Supersedes the plan's engine-gate
status: **the `engine-eqsat` release binary has been rebuilt and now
handles `Arr` leaves — verified live today.**

## 0. State changes since `stepe_tail_plan.md`

| item | plan said | census found |
|---|---|---|
| `engine-eqsat` release binary | stale, pre-`dd4815e` | **rebuilt 2026-11-16 17:28** (`target/release/eqsat-run`), after `dd4815e` (10:43) and HEAD `818c9ed` (15:45). Live probe: `term_to_json(leaf="arr")` dag record → `ok:true` on **both** release and debug; returned term decodes with `Arr` leaf records intact. The `const-str→arr` emit flip is **unblocked**. |
| `source-onnx` dialect | hybrid (`Foreign` + nested-record dag `dim`) | **canonical at `eb87b63`** — "term: emit canonical mlopt_core.term_json dialect (Native source, dag dim index)". `src/term.rs:269,302` emit `"Native"`; `term.rs:296` emits dag `dim` as node index; `attrs` channel throughout. Plan Step 6 is **done**. `tests/synthetic.rs:369-425` now *asserts* no `Foreign` spelling. |
| `kit-discovery` `hnl_codec` | 479-line impl on main; shim drafted uncommitted in `/home/luis/tmp/kit-discovery-stepe` | unchanged: main `7d507c5` still ships the legacy impl (`hnl_codec.py` 479 ln, `_TENSOR={"Foreign":"tensor"}` at :87, `lit.__mlopt_attrs__` emit at :239,:300, nested-record dag `dim` at :287ff). Worktree still detached `fbb61e7` with the 63-line shim uncommitted. **kit-discovery main still pins eqsat `v0.1.10`** (`pyproject.toml:42-44`) — the shim cannot land until the repin resolves through the cascade. |
| eqsat pins | mlopt/kernelopt/graphopt `v0.1.10`; kit-* `v0.1.12` | confirmed: `mlopt/pyproject.toml:61-63`, `kernelopt/pyproject.toml:54-57`, `graphopt/pyproject.toml:132-152` all still `v0.1.10`; `kit-torch` (`/home/luis/tmp/kit-torch-work/pyproject.toml:55-57`), `kit-onnx`, `kit-carriers`, `kit-host-tensor`, `discoverykit`, `kit-cuda` all `v0.1.12`. |

## 1. `const-str` leaf producers (`{"Const":{"value":"__mlopt_leaf__:{…}"}}`)

| file:line | dialect element | verdict |
|---|---|---|
| `eqsat/packages/mlopt-orchestrator/src/mlopt_orchestrator/eqsat.py:392` (`_kterm`), `:402` (`_kdag`), `:417` (`_terms_wire`) | `leaf="const-str"` on all three emit delegates | **eqsat-transitional — flip now unblocked.** Release binary verified `Arr`-capable today; change to `leaf="arr"` (or drop the arg) + re-run `test_eqsat_engine.py` real-binary tests and `eqsat_parity_sweep`. |
| `graphopt/bench/drivers/audit/eqsat_differential.py:299-417` | private `_kterm`/`_pyterm` codec: `Const`-string `__mlopt_leaf__:` payloads for typed leaves **plus** the older `__var__:`/`__var__:param:` scalar fallback for untyped `Var(name,"f64")` leaves | **deliberate engine-wire exception.** The `__var__:` fallback exists because fixture leaves carry `typ="f64"` (a string, not `TensorType`) and cannot encode `Arr` (`_shape_data` would fail); they intentionally read as scalars on the Rust side. Porting to `term_json(leaf="arr")` requires typing every fixture leaf — a test-data project, not a codec swap. `eqsat_shape_guards.py:58` reuses `d._kterm` (no independent codec). |
| `eqsat/packages/mlopt-core/src/mlopt_core/term_json.py:282` (`_leaf_json` const-str arm), `:103` `_LEAF_PROFILES` | the profile implementation itself | codec machinery — stays; decode of const-str is permanent. |
| `eqsat/tests/test_term_json.py:116-169`, `eqsat/tests/test_eqsat_engine.py:581-600` | pin both leaf spellings | tests — update emit-side assertions when the flip lands; decode-tolerance tests stay. |
| `graphopt/bench/drivers/audit/eqsat_parity_sweep.py` | no codec of its own — drives `EqsatEngine` | inherits the flip automatically via `_terms_wire`. |
| `kit-discovery/build/lib/kit_discovery/hnl_codec.py` | stale build artifact of the legacy impl | ignore (regenerated on build); not a producer. |

**No other `const-str`/`__mlopt_leaf__:`-string emitters exist.** The
legacy `hnl_codec` emits structured `Arr` leaves (its leaf spelling was
never the problem — its `source`/`lit`/dag-`dim` were).

## 2. `{"Foreign":"tensor"}` source producers

| file:line | verdict |
|---|---|
| `kit-discovery/src/kit_discovery/hnl_codec.py:87` (`_TENSOR`), emitted at `:236` (tree) and `:297` (dag) | **transitional — the last live Python producer.** Every `term_to_json` call through `kit_discovery.hnl_codec` emits `Foreign` today: `mlopt/stage.py:171` (all v3 stage records), `mlopt/adapters/onnx_rust.py:580` + `arch_rust.py:447` decode-only, `graphopt/bench/drivers/audit/onnx_lifter_differential.py:214` (oracle emit). Retires automatically when the Step-1 shim lands. Decode tolerance is permanent. |
| `graphopt/bench/drivers/probes/term_ir_spike.py:58` (`_TENSOR`), `:84` emit | **historical scratch spike** (`/tmp` binary, `OUT_DIR=/tmp/term_ir_spike`) — deliberate exception per plan §4; do not extend, do not port. |
| `source-onnx/src/term.rs` | **already canonical** (`eb87b63`) — emits `"Native"` at `:269`/`:302`. Plan Step 6 closed. |
| `eqsat/term_json.py:91` + `eqsat/tests/test_term_json.py:422-637` | codec doc + decode-tolerance tests — not producers. |

No other `Foreign` emitters. `kit-prelude` Rust (`kit-mlopt`) and the
`hnl` pipeline scripts treat records opaquely — confirmed, no `source`
inspection anywhere.

## 3. Nested-record dag `dim` producers

Canonical = node-table **index** in `dag.program`; nested
`{"Const":{"value":1}}` record is the legacy spelling. Tree-mode
`dim = {"Const":{"value":1}}` is canonical and appears everywhere —
only dag nodes distinguish the dialects.

| file:line | verdict |
|---|---|
| `kit-discovery/src/kit_discovery/hnl_codec.py:~287` (`_enc_dag` emits `"dim": _DIM1` nested record inside dag `nodes`) | **transitional** — same shim landing retires it. Note engine-eqsat `dag.rs` *requires* the index spelling, so legacy dag records are Python-bound only (never reach `eqsat-run` — the orchestrator re-encodes via `term_json`). |
| `eqsat/packages/mlopt-core/src/mlopt_core/term_json.py:364` (`dim_index()`) | canonical emit — the reference. |
| `source-onnx/src/term.rs:296` | canonical (`eb87b63`) — `dim` is the shared Const node's index. |
| `eqsat_differential._kterm`, `term_ir_spike.enc`, all tree emitters | tree-mode nested `Const(1)` record = canonical; N/A. |

## 4. `lit.__mlopt_attrs__` attr-channel producers

| file:line | verdict |
|---|---|
| `kit-discovery/src/kit_discovery/hnl_codec.py:160-162` (`_attrs_payload`), emitted at `:239`/`:300` as `node["lit"]` | **transitional** — retires with the shim. |
| `graphopt/bench/drivers/probes/term_ir_spike.py:76-90,116-120,156-160` | historical spike — deliberate exception. |
| `eqsat/term_json.py:194-218` | canonical `attrs` emit + dual-channel decode (both-present = decline). |
| decode-only acceptors | `eqsat_differential.py:377-379`, `onnx_lifter_differential.py:395-397`, `hnl_codec.py:177-198` — permanent decode tolerance. |

**Do not confuse** with the *law-format* `("lit", v)` literal channel
(`eqsat/mlopt_core/laws/{cond,minted,serialize}.py`,
`kit-discovery/hnl_surface.py:112,331`, `hnl_import.py:148`) — that is
the pattern/attr-spec vocabulary, unrelated to the term-JSON `lit`
record. Zero producers of `lit.__mlopt_attrs__` outside the two files
above.

## 5. `mlopt_core.ir` shim consumers + direct `term_json` bypassers

Direct `term_json` importers (`import mlopt_core.term_json` /
`from mlopt_core.term_json import`):

- `eqsat/packages/mlopt-orchestrator/src/mlopt_orchestrator/eqsat.py:119` — the sanctioned codec owner/delegate.
- `eqsat/tests/test_term_json.py:30` — codec tests.

**Zero bypassers.** Every other consumer goes through
`kit_discovery.hnl_codec` (`mlopt/stage.py:171,179`,
`mlopt/adapters/onnx_rust.py:580`, `arch_rust.py:447`,
`graphopt/.../onnx_lifter_differential.py:97-99`) or a private codec
(`eqsat_differential._kterm`, `term_ir_spike.enc`). The plan's
recommendation stands: mlopt adapters should import `term_json`
directly at Step 2 — today they ride the shim.

`mlopt_core.ir` import census (src only, excluding tests/venv/target;
view-class consumers — fine indefinitely per plan §3):

| repo | src files | notes |
|---|---|---|
| eqsat (intra-package) | ~55 mlopt-core + ~14 orchestrator | modules inside the defining repo ride their own shim; could repoint to `.term`/`mlopt_core.term` but lowest value — the shim *is* the compat surface here. |
| kit-discovery | 37 | incl. `hnl_import.py:49` (the construction site, ~30 `Op.make`/`Var` sites) |
| graphopt | ~10 `packages/mlopt-torch` (D-1e deletion-bound — skip) + ~95 `bench/` drivers/probes/suites/examples/scripts | bench drivers are class-b consumers; migrate only files already touched |
| mlopt | 6 (`stage.py`, `adapters/{arch_rust,hnl,onnx_rust}.py`, `carrier_expand.py`, `emit.py`, `deliver_game.py`) | Step-2 swap targets |
| kit-torch (`/home/luis/tmp/kit-torch-work`) | 8 src (+102 test files — leave on shim) | construction sites: `torch_export.py`, `torch_bindings.py` |
| kernelopt | 9 src (+4 bench) | `ir_bridge.py:61` is the translator |
| kit-carriers | 10 | |
| discoverykit | 16 | |
| kit-host-tensor | 3 | |
| kit-onnx | ~1 (`lift.py:56`,`liftmap.py:58`,`lower.py:47`,`source.py:36`,`sink.py:24`,`opmap.py:56` per plan) | |
| kit-cuda, host-cli, host, kit-prelude, source-onnx, engine-eqsat | 0 | |

## 6. Retirement-gate status

| dialect element | emit status today | what unblocks the flip |
|---|---|---|
| `const-str` leaf (emit) | eqsat.py quartet only | **DONE gate**: release `eqsat-run` rebuilt + Arr conformance verified live (this census). Remaining: flip `eqsat.py:392,402,417` → `leaf="arr"`, re-run `test_eqsat_engine.py` + `eqsat_parity_sweep` + `eqsat_differential`. `eqsat_differential`'s own codec stays const-str/`__var__` by deliberate exception (untyped fixtures) — or gets ported if fixtures are typed. |
| `const-str` leaf (decode) | — | never retires (retained records + not-yet-rebuilt consumer binaries). |
| `{"Foreign":"tensor"}` | `hnl_codec` (all Python stage/adapter/differential emits) + `term_ir_spike` (scratch) | **kit-discovery shim land** — needs eqsat `v0.1.12` repin resolved through the cascade (kit-discovery main still `v0.1.10`; do NOT land the worktree's `override-dependencies` hack). Decode tolerance permanent. |
| `lit.__mlopt_attrs__` | same two producers | same gate as Foreign — one shim landing retires both. |
| nested-record dag `dim` | `hnl_codec._enc_dag` only (source-onnx fixed at `eb87b63`) | same gate. Engine requires index — legacy dag records are Python-bound only. |
| `mlopt` adapter decode path | `hnl_codec` import at `stage.py:171,179`, `onnx_rust.py:580`, `arch_rust.py:447` | mlopt eqsat `v0.1.10→v0.1.12` repin, then swap to `mlopt_core.term_json` direct (removes codec dep on kit-discovery for decode; `hnl.py` keeps `hnl_import` regardless). |
| `RECORD_VERSION` | 3 | stays 3 per plan §5 — v3 decoder already accepts the union both directions; bump only on the three listed triggers. |
| `mlopt_core.ir` shim | ~205 class-b files across 10 repos | last to retire; gate = census re-run → 0 non-lemma imports or documented permanent compat. `term_to_data`/`term_from_data` (lemma store) is a separate codec — never bundled. |

## 7. Recommended flip order

1. **eqsat `const-str→arr`** — unblocked *today* (release binary verified).
   XS: three call sites + emit-side test assertions. Do first while
   the evidence is fresh; `eqsat_differential`/`eqsat_parity_sweep`
   re-validate.
2. **kit-discovery shim land** — resolve the eqsat `v0.1.12` repin
   through the real cascade (mlopt/kernelopt/graphopt repins are the
   same blocker), rebase the `fbb61e7` worktree draft onto `7d507c5`,
   drop the override hack, run kit-discovery gates. This single landing
   retires all three legacy *emit* spellings (`Foreign`, `lit`,
   nested-dim) for every Python consumer.
3. **mlopt Step-2 swaps** — eqsat repin + direct `term_json` imports in
   `stage.py`/`onnx_rust.py`/`arch_rust.py` (+ `mlopt_core.ir→term`
   repoints in `hnl.py`/`emit.py`). Decide D-2 timing per plan §8.
4. **graphopt bench** — `onnx_lifter_differential` import swap
   (mechanical); `eqsat_differential` → document as deliberate
   engine-wire exception (recommended — its `__var__:` untyped-leaf
   path is load-bearing for the guard fixtures) unless someone wants
   to type the fixtures and port to `term_json`.
5. **view-class repoints** (`ir→term`) — lazy/E-6, construction sites
   first (`hnl_import`, `torch_export`, `ir_bridge`, `lift.py`).
6. **Deprecation gates** — re-run this census; `Foreign`/`lit`/nested-dim
   *emit* retirement is proven by (2); decode tolerance never removed;
   `ir` shim removal last.

## Probe log

`engine-eqsat` conformance, run this session from the eqsat venv
(`leaf="arr"`, `dag=True`, `Op.make("neg", Var("x", TensorType((2,3))))`):

- `target/release/eqsat-run` (built 2026-11-16 17:28): `ok:true`,
  returned term carries `Arr` leaf records — PASS.
- `target/debug/eqsat-run`: `ok:true` — PASS.
