---
title: P3 Step E plan — Term-native adapters, `mlopt_core.ir` retirement, emit decision, D5/D6
type: report
date: 2026-11-2x
status: plan (read-only analysis; binds ADR-0174 done-means, ADR-0175 dissolution map, ADR-0178 D5/D6)
inputs: mlopt_dissolution_plan.md, mlopt_step_d_impact_map.md, live census of mlopt/graphopt/eqsat/kernelopt/discoverykit/kit-onnx/kit-carriers
---

# P3 Step E — concrete plan

**Scope.** Step E of `mlopt_dissolution_plan.md` §3: adapters go
Term-native, `mlopt_core.ir` retires or becomes a thin view
(ADR-0178 D5), the `emit.py` relocation decision, `mlopt_native` /
python-egraph cleanup (D6), and the ADR-0174/0175 exit criteria.
Steps A–C are landed (stage v3 records + `SearchResult.eg=None`,
`hnl-ext-mlopt` dropped to kit-prelude, `emit._OPS` mechanical half
generated from `host_surface.json`). D-1 splits are in flight:
`kit-onnx` v0.1.0 and `kit-carriers` have shipped as repos;
`mlopt-torch`/`mlopt-cuda`/`mlopt-discovery`/`mlopt-host-tensor`/
`mlopt-kit`/`mlopt-experiments`/`mlopt-handl-peer` still live in
`graphopt/packages/`. D-2 (merge + rename) is pending.

**Headline finding.** "Term-native" is blocked on a missing
primitive: **there is no Python-side kernel `Term` object anywhere
in the org.** The kernel `Term` exists only as serde JSON — on the
`eqsat-run` wire (`mlopt_orchestrator/eqsat.py::_kdag`,
`mlopt_discovery/hnl_codec.py`) and inside Rust
(`hnl_engine::Term`, `engine-eqsat`). Every Python consumer —
adapters, laws, cost, typing, emit, verify, deliver — speaks
`mlopt_core.ir` (`Op`/`Var`/`Param`/`Const`/`TensorType`/`IR`,
956-line `ir.py` in eqsat `packages/mlopt-core`). Step E therefore
starts with a *representation decision*, not adapter edits.

---

## 1. Census — who still depends on `mlopt_core.ir`

`mlopt_core.term` does not exist; `ir.py` is the only term module.
Classification by role (import sites verified by grep; file lists
abbreviated to representatives — full lists reproducible via
`grep -rn 'mlopt_core\.\(ir\|term\)'`):

### 1a. The definition + engine internals (eqsat `packages/mlopt-core`)

`ir.py` itself, plus every consumer inside the substrate:

| consumer | role | ir usage |
|---|---|---|
| `egraph/{core,extract,match,terms,types,certs,proof}.py` | the Python EGraph — terms, e-nodes, certificates | `Op`/`Var`/`Param`/`Const` throughout; `term_to_data`/`term_from_data` for merge-log/cert codec |
| `laws/*.py` (~12 modules) | law bank — patterns, minted records, serialization | `Op.make`, `op_def`, `_OP_REGISTRY`, `op_repr` |
| `cost/*.py` (7 modules) | cost algebra — flops/roofline/fusion | term walkers over `Op` |
| `typing.py`, `opmeta.py`, `opdata.py`, `attrs.py` | shape inference, op metadata registry | `TensorType`, `shape_of`, `INVALID` |
| `ports.py` | `Source`/`Sink`/`Engine`/`Executor` protocols | `IR` is the port currency (`Source.lift -> (IR, leaves)`, `Sink.lower(ir, params)`) |
| `pipeline.py`, `meta.py`, `rulecache.py`, `trajectories.py`, `traversal.py`, `cells.py`, `features.py`, `oracle.py` | orchestration/support | term types + `term_to_data` |

### 1b. Orchestration (eqsat `packages/mlopt-orchestrator`)

`optimize.py` (search/lower/`discover_alternatives`/`SearchResult`),
`eqsat.py` (**the engine boundary** — `_kterm`/`_kdag` encode ir →
kernel-Term JSON; a mirror decoder rebuilds ir from the result),
`morphisms/*`, `autotune.py`, `cache.py`, `diagram*.py`, `pricing.py`,
`regime.py`, `crossblock_cse.py`, `bounds.py`, `criteria.py`.
Classification: **engine boundary + pipeline**. Even with
`engine="eqsat"` (the default since WP4), ir remains the in-process
IR — the bridge translates at the subprocess edge only.

### 1c. Domain packages (post-D-1 homes)

| package | repo today | role | ir usage |
|---|---|---|---|
| `mlopt-torch` | graphopt (→ kit-torch pending) | torch Source/Sink: `torch_export` (export → IR), `torch_bridge`, `torch_eval`, `torch_bindings` (2.4k-line op table), `executors/`, `region_fuse`, `calibrate`, `composer` | **engine boundary** — produces/consumes IR at the torch edge |
| `mlopt-discovery` | graphopt (destination TBD — open question 6) | `hnl_codec` (ir ↔ kernel-Term-JSON envelope), `hnl_import` (.hnl → ir), `hnl_surface` (ir → .hnl, ~40 ops), plus the whole mining layer | **codec** (`hnl_codec`/`hnl_import`/`hnl_surface`) + pervasive term use in proposers/verifiers |
| `kit-carriers` | kit-carriers (split done) | `om`/`trace`/`xcarrier`/`*_lower` carrier laws + lowering | laws/lower over `Op`/`IR` — carrier ops are ir-native |
| `kit-onnx` | kit-onnx (split done, v0.1.0) | `lift.py`/`lower.py`/`source.py`/`sink.py`/`opmap.py`/`liftmap.py` | ONNX → **ir** `IR` (the Python oracle); Rust `source-onnx` emits kernel Term-JSON instead |
| `mlopt-host-tensor` | graphopt (→ host-tensor/lib-tensor pending) | `host_surface.json` producer side, bindings, executor | surface/bindings; `emit.py` reads its JSON |
| `mlopt-cuda` | graphopt | cuda-graph runner | executor-side, light ir use |

### 1d. Product package (mlopt `src/mlopt/`)

| file | ir usage | class |
|---|---|---|
| `emit.py` (2971 ln) | `Const, Op, Param, Var` + `typing.shape_of`/`INVALID` | **codec/emitter** — the `.hnl` render path |
| `carrier_expand.py` (97 ln) | `Op` | bound to emit (carrier re-spelling) |
| `stage.py` (925 ln) | `term_from_data` legacy decode (transitional, retires at D5), `IR` rebuild in `_ir_decode` | **codec** — v3 records already emit `hnl_codec` Term-JSON; only *decode* still touches ir |
| `adapters/hnl.py` | `IR, Op, Param, TensorType, Var` via `hnl_to_term` | **adapter (Source)** |
| `adapters/onnx_rust.py` | `IR, Op, Param, TensorType, Var` — decodes source-onnx `dag.program` JSON via `hnl_codec.term_from_json` | **adapter (Source)** — already receives kernel Term JSON |
| `adapters/arch_rust.py` | `IR` only (reuses onnx_rust decode) | **adapter (Source)** — same |
| `adapters/whisper.py`, `kokoro.py`, `_kokoro_packed.py` | **zero direct ir imports** — they build `ModelSpec`s whose `source` port lives in `mlopt_torch` | adapter (indirect) |
| `optimize.py`, `cli.py`, `doctor.py`, `deliver*.py`, `play.py`, `weights.py`, `models/` | **zero ir/egraph imports** — IR flows opaquely through `Optimizer`/`SearchResult` | composition only |

Tests: `mlopt/tests/{test_emit,test_emit_surface,test_emit_embedding,
test_hnl_source,test_onnx_rust_source,test_stage,test_cli}.py` all
construct ir terms directly — they are the drift surface for any
representation change.

### 1e. Other repos

- **kernelopt** `src/mlopt_kernel/` — `ir_bridge.py`, `laws.py`,
  `patterns.py`, `sink.py`, `scan.py`, `board.py`, `cost.py`,
  `priorart.py`, `play.py` + bench/tests: heavy ir use (the
  codegen domain's term currency).
- **discoverykit** `packages/discoverykit/` — ~20 modules
  (`antiunify`, `coherence`, `combinator`, `lemma_cert`, `handl`,
  `verifier`, `genkernel`, …) + tests: ir is the mining layer's
  term type.
- **graphopt bench** — ~60 drivers/suites/probes import
  `mlopt_core.ir`/`egraph` (differential drivers, mint loops,
  measurement probes). Dev tooling; follows whatever the substrate
  keeps.
- **eqsat `mlopt-native`** — `python/mlopt_native/engine.py`
  imports `ir` + `egraph.*` heavily: it is a PyO3 port of the
  *Python* egraph, so it dies with D6 regardless.
- **host-tensor, gamekit, kit-prelude (Python), source-onnx** —
  no `mlopt_core.ir` imports (verified).

### 1f. What "kernel Term" means in Python today

- `hnl_codec.term_to_json`/`term_from_json`
  (`mlopt_discovery/hnl_codec.py`, 460 ln) — encodes/decodes
  **mlopt_core.ir terms** into the kernel-`Term` JSON envelope
  (`{"Op"/"Arr"/"Const"/"Opaque"}`, `dag.program` sharing). It is a
  *view codec over ir*, not a kernel-Term object.
- `mlopt_orchestrator/eqsat.py::_kdag` — a second, parallel
  ir → kernel-Term-JSON encoder for the `eqsat-run` subprocess
  (leaf prefix `__mlopt_leaf__:` carries `TensorType` payloads the
  kernel `Term` cannot express natively).
- Rust side: `hnl_engine::Term` (`handl-lang/engine`),
  `handl_egraph` internal term domain, `engine-eqsat` public API
  (`saturate`/`extract_best`/`optimize` — kernel Terms in/out).
- **No PyO3 `Term` binding exists.** `mlopt-handl-peer` exposes
  rewrite-check calls, not term objects; `host-tensor`'s PyO3 module
  is tensor values, not terms; `compile-term` goes AST → Term-JSON
  (source direction, wrong way for adapters).

**Consequence:** D5's "Sources/Sinks produce/consume kernel `Term`s"
has three possible meanings, and the plan must pick one:

- **(A) PyO3 Term binding** — expose `hnl_engine::Term` (or a thin
  `Term`/`Op` wrapper) to Python via a new/maturin crate
  (`mlopt-handl-peer` precedent). Adapters then hold real kernel
  Terms; `hnl_codec` becomes pure serde. Cost: new FFI surface,
  attrs/leaf-payload impedance (kernel `Term` has no typed-leaf or
  metavariable leaves — the `__mlopt_leaf__:` hack becomes a real
  type or stays a codec convention), and every ir consumer
  (laws/cost/typing/egraph) still needs *something* — either the
  binding grows the whole ir API surface in Rust (huge) or Python
  keeps a view layer.
- **(B) Python kernel-Term view** — re-implement the ir *type
  contract* (`Op`/`Var`/`Param`/`Const`/`TensorType`/`IR` + attrs +
  `op_def` registry) as a thin module whose serialization *is* the
  kernel Term-JSON envelope, generated/bound to the kernel `Term`
  schema. `mlopt_core.ir` retires as a name; the view lives wherever
  the codec lands. Cost: medium; keeps every consumer working if
  the view is API-compatible; honest reading of "thin view" in
  ADR-0178.
- **(C) JSON-native adapters** — adapters emit/consume Term-JSON
  documents and a minimal Python `Term` wrapper (dataclasses over
  the JSON records) serves the pipeline. Cheapest, but pushes
  parse/validate cost into every consumer and leaves the "one Term"
  claim weaker (a dict-shape, not a type).

Recommendation: **(B), with (A) as the long-term substrate** — the
view is what ADR-0178 literally ratified ("`mlopt_core.ir` becomes a
thin view or retires"), it unblocks adapters without an FFI
project, and a PyO3 Term binding can replace the view's guts later
without touching consumers. Decide formally at E-1; the deciding
evidence is how much of `ir.py`'s API (attrs mutability, `op_def`
registry, `TensorType.shape` `None`-wildcard semantics, interned
`Op.make`) a kernel-`Term`-shaped view can honestly express — the
`__mlopt_leaf__:`/`TensorType` payload is the known gap (kernel
`Term` leaves are untyped `Const`s; the leaf typing is mlopt-side
metadata that must ride in attrs or a side channel).

---

## 2. Adapter landscape — what "Term-native" means per adapter

All adapters sit behind `mlopt.models` `ModelSpec` + the
`mlopt_core.ports.Source` protocol (`lift -> (IR, leaves)`); the
product pipeline (`optimize.py`) is type-opaque. Term-native flips
the *decoded object* inside each Source, not the CLI surface.

| adapter | today produces | Term-native target | migration shape |
|---|---|---|---|
| `whisper.py` (887 ln) | `ModelSpec` w/ torch source → `mlopt_torch.torch_export` → **ir `IR`** | kernel Term (or view-B term) | **blocked on kit-torch split** — the lift lives in `mlopt_torch`, not here. Adapter file itself needs ~zero changes; the work is in `torch_export`/`torch_bridge` |
| `kokoro.py` + `_kokoro_packed.py` (~1050 ln) | same — torch submodule specs | same | same — rides the mlopt-torch flip |
| `hnl.py` (444 ln) | `mlopt_discovery.hnl_import.hnl_to_term` → **ir term** | kernel Term | `hnl_to_term` must emit the new representation; alternatively route `.hnl` → `compile-term` AST→Term-JSON (the *correct-direction* compiler already exists) — evaluate overlap, don't build a third parser |
| `onnx_rust.py` (810 ln) | source-onnx `lift` example → `dag.program` JSON → `hnl_codec.term_from_json` → **ir** | kernel Term | **closest to done** — the binary already emits kernel Term JSON; the flip is "decode into the new repr instead of ir". Also carries safetensors payload binding + seeded-input probes |
| `arch_rust.py` (718 ln) | source-onnx `arch` example → same path | kernel Term | reuses onnx_rust's decode — flips with it |

**Ordering inside E:** `onnx_rust`/`arch_rust` first (pure decode
target swap, already JSON-native, env-gated tests exist), `hnl`
second (parser repr swap), `whisper`/`kokoro` last (blocked on the
kit-torch D-1 split + the deepest ir surface: `torch_export`,
`torch_bridge`, `torch_eval`, `torch_bindings`).

**What does NOT change:** `ModelSpec`, `catalog.py`, CLI flags,
`--adapter-opt`, report shapes, the `Source` port *signature* (its
return type's name may change, its role doesn't).

---

## 3. `emit.py` relocation analysis

`mlopt/src/mlopt/emit.py` (2971 ln) — term → `.hnl` over `tensor.*`.

### Dependencies (verified)

| dep | what | relocation impact |
|---|---|---|
| `mlopt_core.ir` (`Const, Op, Param, Var`) | term types walked by `_OPS` builders | **hard dep** — emit cannot leave mlopt before D5 lands the view/binding; moving earlier = porting emit against a moving target |
| `mlopt_core.typing.shape_of`, `INVALID` | `-1` shape resolution | same — needs a shape oracle in the new home |
| `mlopt.carrier_expand` | `om_*` → `tensor.*` re-spell | bound to emit; moves with it |
| `mlopt_host_tensor.host_surface.json` | drift-guarded op surface (239 ops); read **as a file** via `importlib.util.find_spec` + path join — does NOT import the package (numpy/PyO3-free read, emit.py:1949-1965) | the *producer* lives in `mlopt-host-tensor` (graphopt → lib-tensor/host-tensor pending); a moved emit needs the JSON reachable — vendoring a generated copy re-introduces an unguarded table (invariant-4 violation) unless the drift test crosses repos |
| `hnl-run` binary | `KitRunner` subprocess; `_hnl_run_candidates` hardcodes sibling `graphopt/target` + `.ci-target/graphopt` (emit.py:2815-2820 — already flagged in impact map A.5.1, must be fixed at D-2 regardless) | repo-agnostic resolution needed wherever it lands |
| safetensors sidecar | own `struct`-level writer (`sidecar=` path) | self-contained, moves free |
| stdlib only otherwise | `array/json/os/shutil/subprocess/Decimal` | torch-free, binary-free at import — already package-clean |

### Candidate homes (from dissolution plan §1b/§4.3, now with evidence)

1. **Stay in merged-mlopt as codec** — ADR-0174 explicitly allows
   codec in the product repo; Step C already made `_OPS` canonical
   (mechanical half generated). Zero move cost. The honest default.
2. **host-tensor / lib-tensor** — owns `tensor.*` `ast.yaml` +
   `host_surface.json` producer side. But host-tensor's end-state is
   a **Rust** crate; a 2971-line Python emitter there is packaging
   churn with no layer win *unless* ported to Rust — and a Rust port
   needs the Term-native repr first (E-2) plus a `.hnl` *emitter*
   (text writer), which is compile-*/host territory, not obviously
   lib-tensor's. Only justified if a second emitter consumer appears
   (see hnl_surface below).
3. **A `compile-*` repo** — emit is Term → surface-source, arguably
   an emitter. But compile-* crates are Rust AST→Term-JSON
   compilers; a Python `.hnl` renderer fits awkwardly and splits the
   codec story.

### The consolidation finding (new)

There are **two** term→`.hnl` emitters today:

- `mlopt.emit` — 199-op product emitter (sidecar safetensors,
  `emit_function`, verify-via-hnl-run, drift-guarded).
- `mlopt_discovery.hnl_surface` — 546-ln, ~40-op `_SPECS` table
  emitter + its own `KitRunner`, graduated from the bench driver,
  consumed by discovery/bench paths.

Invariant 4 ("one codec, generated views or named retirement")
argues these must converge *before* any physical move — either
`hnl_surface` grows into `emit`'s table (unlikely: different
honesty bounds) or `hnl_surface` is declared the bench-grade
subset and `emit` the product emitter with the relationship
documented and drift-guarded. **Recommend: converge or explicitly
partition at E-5; do not move either until then.**

### Recommendation

**Keep `emit.py` + `carrier_expand.py` in merged-mlopt as codec**
(the ADR-permitted outcome), *unless* E-5's consumer census shows
the emitter is needed Rust-side. Revisit a Rust port only when:
(a) adapters are Term-native (kernel `Term` in Python or Rust),
(b) a second consumer needs Term→`.hnl` outside the product CLI,
(c) `host_surface.json` has a stable cross-repo read contract.
The decision is evidence-gated, not scheduled — exactly as the
dissolution plan says.

---

## 4. `mlopt_native` + python-egraph oracle — what retires vs stays

### `mlopt_native` (eqsat `packages/mlopt-native`)

- What it is: maturin/PyO3 port of the **Python egraph's search
  core** (union-find, enode storage, compiled matching, rebuild,
  extraction via `_ExtractMixin`) — *not* engine-eqsat.
- Reachability: optional distribution, workspace-excluded,
  **never auto-detected** — reachable only as an explicit
  `engine=` instance (`NativeEngine`).
- D6 retires it: superseded by `engine="eqsat"` (the default since
  WP4 v4 GO). Retirement = delete `packages/mlopt-native`, drop
  `engine.py`, scrub `optimize.py`/`ports.py` doc references,
  remove the `mlopt_native` extra/peer pins in eqsat pyproject.
- Gate: confirm no consumer passes a `NativeEngine` instance —
  grep org for `NativeEngine`/`mlopt_native` imports outside eqsat
  (currently only doc mentions + `test_native_engine.py`/
  `test_native_extract.py`, self-skipping when unbuilt).

### python-egraph (`EGraph`)

ADR-0178 is explicit and must not be misread: **"EGraph remains
the correctness oracle permanently (differential harness is a kept
test, not a phase)."** What D6 retires is "the Python egraph
*proper*" — i.e., its role as a shipped search engine — while the
machinery survives as oracle/spec. Tension to resolve at E-1:

- `engine="python-egraph"` is a *named opt-out* today (the honest
  fallback when `eqsat-run` is missing is a loud
  `EqsatUnavailableError`, not silent fallback — good).
- **Feature pins keep it load-bearing:** `discover_alternatives`/
  `Optimizer.discover` default-pin `"python-egraph"`
  (optimize.py:1667, 1693) because frontier inspection
  (`.alternatives()`) is proof-carrying EGraph machinery;
  **certificates** (`.certificate()`, `egraph/certs.py`,
  `proof.py`) exist only on the Python engine — eqsat-side these
  are named declines (`CertUnavailable` precedent); `policy=`,
  `truncation_level`, `stop="improving"`, `error_budget`,
  `detect_*` passes all refuse-or-skip by name on eqsat.
- Consumers of the pinned paths: `bench/suites/cost/
  cost_fidelity.py` (frontier enumeration), graphopt tests
  (`test_search_lower_split`, `test_optimize_edges`,
  `test_pluggable_sink`, `test_optimize_compositional_edges`),
  discoverykit `lemma_cert.py` (certificate replay for minted-law
  admission evidence).

**So D6's honest end-state is not "delete the Python egraph" — it
is:**

1. `mlopt_native` deleted (clear-cut).
2. `engine="python-egraph"` demoted from *documented opt-out* to
   *oracle-only* — either removed from `_ENGINE_NAMES` (breaking:
   named decline `OracleEngineRequired` for certificate/frontier
   requests) or kept but documented as test/oracle infrastructure.
   **This is a user decision** — it changes the shipped engine
   surface.
3. The procedural-hook laws that can't re-express on engine-eqsat
   get the ADR-0178 "named declines" list published (the
   `GuardUnsupported`/`UnserializableLaw`/`UnknownCostKind` →
   `PolicyUnsupported`/`CertUnavailable` precedent extended).
4. The differential harness (`egraph_differential.py`,
   `eqsat_differential.py`, `eqsat_parity_sweep.py`,
   `handl_differential.py`) stays green permanently — it IS the
   oracle's job.

---

## 5. D5/D6 exit criteria — what's measurable

From ADR-0178 (the binding sequencing ADR) + ADR-0174 (done-means):

**D5 — adapter Term-native output**
- *Measurable:* `grep -rn 'mlopt_core\.ir' <adapter paths>` → zero
  non-view imports; Sources/Sinks produce/consume the chosen
  representation; `hnl_codec` (or successor) is the *only*
  translator; `mlopt_core.ir` is either deleted or reduced to a
  shim over the view; stage `RECORD_VERSION` bumped with
  kit-mlopt `RECORD_SCHEMA_VERSION` repin; differential harness
  green.
- *Honest scope warning:* D5 as written ("`mlopt_core.ir` becomes a
  thin view or retires") covers the **whole substrate**, not just
  `src/mlopt/adapters` — laws/cost/typing/egraph/mlopt-torch/
  mlopt-discovery/kit-carriers/kit-onnx/kernelopt/discoverykit all
  import it (§1). A plan that flips only the five product adapters
  leaves ir alive underneath — which is *fine and probably
  correct*: the view option exists precisely so the substrate keeps
  working. The exit criterion should be "no *adapter/codec* path
  requires ir-as-canonical; ir-or-view is the single Python-side
  term type" — not "ir.py deleted".

**D6 — retirements**
- *Measurable:* `packages/mlopt-native` gone from eqsat;
  `import mlopt_native` fails; no shipped path defaults to or
  silently reaches the Python egraph; certificate/frontier/
  `policy=` requests on `engine="eqsat"` produce the named declines;
  the un-re-expressed procedural-hook law list is published;
  `engine="python-egraph"` reachable only as explicit oracle opt-in
  (or removed — decision at E-1).

**ADR-0174 done-means (the audit loop Step E feeds):**
- "one Term, one codec" — census today: `engine::Term` (kernel,
  canonical), `egraph::Term` (Rust-internal, bridged — allowed),
  `mlopt_core.ir` (the demotion target), `lib_kernel_ir.Term`
  (native-path IR). Step E's contribution: collapse the Python side
  to ONE term type + ONE wire codec (`hnl_codec` envelope).
- "verified — honesty invariants at substrate": verify gates,
  decline paths, drift guards — Step E must not weaken any
  (`test_emit_surface`, `verify-run`, named refusals all stay).
- "tested — drift guards on every generated artifact": any new
  generated view (option B) needs its `--check` guard in the same
  commit.

---

## 6. Ordered step list

Ordering principle: **decision → primitive → codec → adapters →
emit decision → retirement → audit.** Steps E-1/E-2 can start
before D-2 completes (they're substrate-side); E-4's whisper/kokoro
arm waits on the kit-torch split.

### E-1 — Representation decision + census freeze (no code)

- Pick (A) PyO3 binding / (B) Python view / (C) JSON wrapper per §1f;
  pick the python-egraph end-state per §4 (oracle-only vs removed).
- Freeze the §1 census as a checked-in artifact (a
  `project/reports/` table or a drift-guard test listing allowed
  `mlopt_core.ir` import sites — the census *is* the D5 scoreboard).
- **Repos:** host (report), possibly a myco todo per AGENTS rules.
- **Gate:** user ratifies both decisions (they're ADR-level).
- **Blast radius:** none — documentation.
- **Rollback:** n/a.

### E-2 — Python kernel-Term surface (the primitive)

- Option B: new view module (home: wherever `mlopt_core.ir`'s
  successor lives — likely still eqsat `mlopt-core` as
  `mlopt_core.term` or a `kit-term` data/codec package; decide with
  E-1) implementing `Op`/`Var`/`Param`/`Const`/`TensorType`/`IR` +
  attrs + `op_def` registry + `term_to_data`-free serde over the
  hnl_codec envelope; `__mlopt_leaf__:` typed-leaf payload becomes
  a first-class record or documented attrs channel.
- Option A: PyO3 crate exposing `hnl_engine::Term` (mlopt-handl-peer
  precedent; maturin, workspace-excluded, opt-in like mlopt-native).
- **Repos:** eqsat (mlopt-core or new package) or kit-prelude;
  engine (`hnl_engine` bindings if A).
- **Gate:** round-trip `term → JSON → term` property test vs
  hnl_codec; `Op.make` interning semantics preserved or deliberately
  changed with consumer audit; ty/ruff clean.
- **Blast radius:** substrate-internal; zero downstream until E-3.
- **Rollback:** new module, nothing imports it yet — delete.

### E-3 — Codec consolidation (hnl_codec over the new repr)

- `mlopt_discovery.hnl_codec` re-based: encode/decode the E-2
  representation; `term_from_data` legacy decode path deleted;
  `mlopt_orchestrator/eqsat.py::_kdag`/`_kterm` deduplicated onto
  the one codec (today there are TWO parallel ir→kernel-Term
  encoders — a named drift risk).
- `stage.py`: drop `term_from_data` import (the last ir decode),
  bump `RECORD_VERSION` → v4 if payload shape changes; kit-mlopt
  `RECORD_SCHEMA_VERSION` repin (kit-prelude).
- **Repos:** mlopt-discovery's post-D-1 home + eqsat
  (mlopt-orchestrator) + mlopt (`stage.py`) + kit-prelude
  (`kit-mlopt`).
- **Gate:** stage lift→search→verify→deliver green via
  `python -m mlopt stage` AND `hnl run` pipeline example;
  `pytest` in each touched repo.
- **Blast radius:** medium — the stage protocol is a live
  cross-repo contract; version gate protects old records.
- **Rollback:** git-revert; v3 decoder path can be kept one release
  if desired (it's already shape-dispatched).

### E-4 — Adapter Term-native flip (leaf-first)

- **E-4a `onnx_rust` + `arch_rust`:** decode `dag.program` into the
  E-2 repr directly (skip the ir detour); safetensors binding +
  seeded inputs unchanged. Tests: `test_onnx_rust_source.py`,
  arch spec tests.
- **E-4b `hnl`:** `hnl_to_term` emits the new repr — or replace
  with a `compile-term`-backed path if that proves cleaner (it is
  the correct-direction compiler; evaluate, don't assume).
- **E-4c `whisper` + `kokoro`:** flip `mlopt_torch`'s
  `torch_export`/`torch_bridge`/`torch_eval`/`torch_bindings`
  Source/Sink side — the deepest arm; gated on kit-torch existing
  (D-1) and on how much of `ir.py`'s API the view covers.
- **Repos:** mlopt (adapters) + kit-onnx-adjacent decode +
  kit-torch (post-split) + mlopt-discovery home.
- **Gate per arm:** adapter tests green + one real-model
  `mlopt optimize` smoke + `--verify-run` where cheap; differential
  harness spot-check.
- **Blast radius:** per-adapter isolated behind `ModelSpec`;
  worst case is E-4c (torch bindings table is 2.4k lines).
- **Rollback:** per-adapter flag or revert; adapters are
  independently shippable.

### E-5 — emit.py decision (evidence-gated)

- Census emit consumers (`cli export`, `stage deliver`,
  `deliver_game`? — verify), compare `mlopt.emit` vs
  `mlopt_discovery.hnl_surface` overlap; decide: stay-as-codec
  (default) / consolidate / Rust-port / move.
- If staying: fix `_hnl_run_candidates` repo-agnosticism (already
  required at D-2), document the hnl_surface relationship,
  re-point `host_surface.json` read at whatever home
  `mlopt-host-tensor` lands in.
- **Repos:** mlopt (+ mlopt-discovery home if consolidating;
  host-tensor only if moving).
- **Gate:** `test_emit_surface.py` drift suite + `hnl-run`
  acceptance on the 3 proven arch families.
- **Blast radius:** mlopt-only if staying; cross-repo if moving.
- **Rollback:** n/a for stay-decision; git history for moves.

### E-6 — `mlopt_core.ir` view retirement

- Once E-3/E-4 land: `ir.py` either becomes the E-2 view (rename/
  shim — consumers unchanged) or is deleted with consumers ported.
  The §1 census is the worklist; prioritize by consumer count
  (egraph/laws/cost/typing are the mass — they may legitimately
  keep the view indefinitely).
- **Repos:** eqsat primarily; downstream repins per breakage.
- **Gate:** full eqsat suite + graphopt/mlopt/kernelopt/discoverykit
  suites green on the repin; `lint-imports` contracts updated.
- **Blast radius:** **largest Step-E blast radius** — every repo
  that imports `mlopt_core.ir` (~10 repos, hundreds of files).
  This is why E-6 is last and arguably open-ended: the *view*
  outcome means it never fully "retires" — the ADR allows this.
- **Rollback:** keep `ir.py` as a compat shim one release cycle;
  tags/pins are the restore points.

### E-7 — D6 retirements

- Delete `eqsat/packages/mlopt-native` (+ pyproject extra/peer
  scrub, doc updates); publish the named-decline list for
  certificate/frontier/`policy=` on eqsat; apply the E-1
  python-egraph decision (oracle-only demotion or removal).
- **Repos:** eqsat (+ consumer doc sweeps).
- **Gate:** eqsat suite green; `import mlopt_native` fails loudly;
  `engine="python-egraph"` behavior matches the E-1 decision;
  differential harness green.
- **Blast radius:** small-medium — opt-in package, few consumers.
- **Rollback:** git-revert eqsat; package is self-contained.

### E-8 — Exit audit

- arch_review protocol pass against ADR-0174 done-means +
  ADR-0178 D5/D6 criteria; Term census re-run (should show ONE
  Python term type + ONE wire codec); AGENTS.md/docs updates in
  every touched repo; myco statuses honest.
- **Gate:** a full audit finds no invariant violations, no
  unguarded tables, no undocumented seams (the ADR-0174 loop
  definition of done).
- **Blast radius:** docs-only.

---

## 7. Risks / open questions for the parent

1. **The representation decision (E-1) is genuinely open.** Option
   B (view) is the conservative ADR-literal path; option A (PyO3)
   is cleaner long-term but is a project, not a step. If the user
   wants D5 *real* rather than *nominal*, A may be unavoidable —
   surface this explicitly.
2. **`__mlopt_leaf__:` typed leaves.** Kernel `Term` has no typed
   variable/parameter leaf — shape/name/kind ride as encoded
   `Const` strings today. Any Term-native repr must decide: first-
   class leaf records (codec-level) vs attrs channel vs accepted
   limitation. Affects E-2 design directly.
3. **Two parallel ir→kernel-Term encoders exist** (`hnl_codec` vs
   `eqsat.py::_kdag`) — consolidation is required regardless of
   E-1's outcome; flag as its own drift item.
4. **python-egraph removal is user-facing.** Certificates and
   frontier inspection (`discover_alternatives`) are shipped
   features pinned to it; "oracle-only" demotion changes the
   `engine=` surface. Needs explicit ratification, not a silent
   default.
5. **D5 scope honesty.** "Adapters go Term-native" reads small;
   the census shows ir is the substrate's term type org-wide. The
   plan's E-6 may legitimately end at "view, not deletion" — say so
   in whatever status doc tracks D5 so the phase isn't claimed
   done on adapter flips alone.
6. **D-1 dependency.** E-4c (whisper/kokoro) and the emit
   `host_surface.json` home both wait on splits still in flight —
   sequence E-4c after kit-torch lands, not before.
