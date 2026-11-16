---
title: mlopt dissolution plan — module disposition + ordered migration
type: report
date: 2026-11-2x
status: plan (post-search-arc; binds ADR-0174 P3 tail, ADR-0175 dissolution map, ADR-0176/0178)
---

# mlopt dissolution plan

**Scope.** Concrete module-by-module disposition for `handl-lang/mlopt`
(`src/mlopt/`, ~10.9k lines) against ADR-0175's layer map, plus the
ordered migration sequence. Preconditions already landed: engine-eqsat
is the default everywhere (eqsat `main`), the law corpus is canonical
in `kit-eqsat` with an empty divergence allowlist, the repin cascade is
green (graphopt 8988/0-fail), and `verify-run` is proven on 3 arch
families. What remains is the *dissolution*: mlopt becomes a thin kit —
adapters + stage protocol + deliver — with machinery living in
engine-*/compile-*/lib-*/host-* repos.

Inputs: ADR-0174 (end-state + invariants), ADR-0175 (dissolution map),
ADR-0176 (Python only at ecosystem edges), ADR-0178 (codec/IR
demotion), `mlopt_egraph_demotion_plan.md`, live census of
`mlopt/src/mlopt/`, `kit-prelude/crates/kit-mlopt`,
`graphopt/packages/mlopt-kit`, `compile-term`, `host-tensor`,
`engine-eqsat`, `lib-tensor`.

---

## 1. Module disposition table

### (a) Product surface — stays in mlopt (the kit's Python edge)

| module | lines | role | end-state home |
|---|---|---|---|
| `cli.py` | 783 | `mlopt models\|optimize\|export\|doctor\|stage`; owns `--verify-run` numeric gate (`_verify_run_export`, `_verify_run_tolerance`) | stays — product CLI; `hnl mlopt …` command registration is a separate gap (§3-B) |
| `optimize.py` | 315 | lift → `Optimizer.search` → `lower(verify=True)` → `OptimizeReport`; engine-agnostic already | stays — the composition the kit wraps |
| `stage.py` | 858 | `python -m mlopt stage lift\|search\|verify\|deliver` — the JSON protocol endpoint `handl-kit-mlopt` shells to | stays — **this IS the kit seam**; needs codec/schema updates (§3-A) |
| `doctor.py` | 96 | lift-gap diagnostics | stays |
| `models/` (`__init__.py`, `catalog.py`) | ~160 | adapter registry/catalog | stays |
| `adapters/whisper.py`, `kokoro.py`, `_kokoro_packed.py` | ~1900 | HF model lifters — Python-at-ecosystem-edges per ADR-0176 | stays |
| `adapters/hnl.py` | 444 | `.hnl` Source (`mlopt_discovery.hnl_import`) | stays — Source port |
| `adapters/onnx_rust.py`, `arch_rust.py` | ~1530 | subprocess doors to `source-onnx` Rust lifter (`lift`/`arch` examples) | stays as adapters; **note:** the arch-spec assembly machinery is arguably source-onnx's, not product's — revisit at dissolution (see §4) |
| `deliver.py` | 445 | `select_runner` heuristic + `_bench` timing; region_fuse/cuda_graph picks | stays — product "deliver" |
| `deliver_game.py` | 627 | `DeliveryBoard` on gamekit `Board`; measured argmin referee under verify gate | stays — product delivery-as-game |
| `play.py` | 320 | `register()` seating `random`/`playbook`/`linear*` via `gamekit.players.registry` | stays — plugin seam |
| `weights.py` | 20 | `DELIVERY_PLAYER_WEIGHTS` baked table | stays — data for `play.py` |
| `deliver_train.py` | 310 | delivery-prior trainer (imitation + REINFORCE over the deliver board) | stays — product-specific prior; meta-tier libs (gamekit) are the machinery it *uses*, not its home |
| `__init__.py`, `__main__.py` | 16 | package entry | stays |

### (b) Movable machinery — candidates, gated on D5

| module | lines | what it is | candidate target | verdict |
|---|---|---|---|---|
| `emit.py` | 2802 | term → `.hnl` over `tensor.*`; 199-op `_OPS` builder table, drift-guarded against `mlopt_host_tensor.host_surface.json` (237 ops); lazy `hnl-run` KitRunner | **short-term: stays** (codec is explicitly allowed in mlopt per ADR-0174 "no IR of its own (codec only)"); **long-term: host-tensor** (owns the tensor `Language` fragment + `host_surface.json`) or a `compile-*` emitter once adapters go Term-native | **defer to D5** — emit consumes `mlopt_core.ir` types (`Const`/`Op`/`Param`/`Var`, `shape_of`) today; moving earlier means new Python packaging with no layer win |
| `carrier_expand.py` | 97 | structural `om_*` → `tensor.*` re-spelling for the emit path | moves **with** emit.py, wherever that lands | bound to emit |

`emit._OPS` is one of the five Python-side tables ADR-0178 invariant 4
censused; its end-state per that ADR is *generated from
`host_surface.json` or retired*. That flip is a mlopt-internal change
(generator + `--check` drift test), not a repo move — do it before any
physical move so the moved artifact is already canonical.

### (c) Dead / demoted

| item | state |
|---|---|
| e-graph engine usage | **none in shipped paths** — `optimize.py` goes through `ports.Engine`; eqsat is default. Sole residue: `stage.py:489–494` constructs a placeholder `EGraph(truncation_level=1)` to fill `SearchResult.eg` — python-egraph plumbing debt, removable once `SearchResult.eg` tolerates a null/non-EGraph (eqsat-side protocol relaxation) |
| `python-egraph` opt-out | stays available by design (oracle role is permanent per ADR-0178) — not dead |
| `mlopt_native` | lives in eqsat, retires at D6 — not an mlopt-repo concern |
| `hnl-ext-mlopt` crate (`crates/hnl-ext-mlopt`, ~90 lines argv forwarding) | **dies with dissolution** per ADR-0175 ("category dies — extensions are kits registering commands/ops") — but only after `handl-kit-mlopt` covers the full command surface (§3-B) |

### External consumers (repin surface)

| consumer | coupling | blast |
|---|---|---|
| `handl-lang/kit-prelude:crates/kit-mlopt` | spawns `python -m mlopt stage <name>`; `MLOPT_PYTHON`/`python3` resolution; pipeline example `examples/pipeline/` | protocol changes need a `schema:` pin (ADR-0178 WP0a) + example re-run |
| `hnl` ext discovery | `HANDL_EXT_DIR`/`~/.handl/ext/` loads `libhnl_ext_mlopt.so` → `python -m mlopt` | dies when kit command registration lands |
| `mlopt/scripts/train_delivery_prior.py`, `whisper_*_bench.py` | in-repo | none |
| graphopt / kernelopt / eqsat / discoverykit | **no `import mlopt` anywhere** (verified — only a stale `.pyc` hit) | zero — the package is a leaf |

---

## 2. What already exists (homes checked)

- **`handl-kit-mlopt`** (`kit-prelude/crates/kit-mlopt`) — Rust crate,
  `pipeline` module = lift/search/verify/**deliver** stage-ops over the
  JSON protocol. Pipeline examples exist and run
  (`kit-prelude/examples/pipeline/{model,pipeline,delivered}.hnl`,
  README documents the `>>>` right-assoc caveat and the
  `artifact_executed` honesty field). **Gap: it implements stage-ops
  only — not the `models|optimize|export|doctor` command surface**, so
  `hnl mlopt optimize …` still needs `hnl-ext-mlopt` today.
- **`engine-eqsat`** — search interpretation over kernel `Term`;
  `saturate`/`extract_best`/`optimize`, guards-as-data, cost specs.
  No home for emit/verify-run and none needed — those aren't search.
- **`host-tensor`** — tensor `Language` fragment (`data/language/
  tensor/ast.yaml`, 238 ops), `host_surface.json` producer-side,
  `lib-tensor` mirror. **Natural long-term home for a Term-native
  emit** (it owns the op surface the emitter targets).
- **`compile-term`** — AST→Term-JSON compiler (ADR-0114); wrong
  direction (source→Term, not Term→`.hnl` source). Not the emit home.
- **`mlopt-kit`** (`graphopt/packages/mlopt-kit`) — Rust composition
  seam (CatoptKit, `hnl-run` bin, conv_bn `.hnl`); absorbs into the
  renamed-mlopt repo at dissolution; no emit/verify machinery.
- **`source-onnx`** — Rust lifter (`lift`/`arch` example binaries)
  that `adapters/{onnx_rust,arch_rust}.py` already subprocess into.

---

## 3. Ordered migration plan

Each step names its gate and blast radius. Ordering principle from
ADR-0175: *relabeling + moving, not rewriting* — codec/schema work
first (it's mlopt-internal), command-surface parity second (unblocks
ext death), table canonicalization third (shrinks what moves), repo
dissolution last (gated on ADR-0175 phase rows).

### Step A — stage protocol codec modernization (smallest, unblocks B)

- Add the `schema:` pin to stage records (ADR-0178 WP0a remainder);
  migrate `term_to_data`/`term_from_data` usage in `stage.py` toward
  the `hnl_codec` envelope (WP0b).
- Remove the `EGraph(truncation_level=1)` placeholder in
  `stage.py:489` — needs an eqsat-side relaxation of
  `SearchResult.eg` (nullable or protocol object). **Cross-repo:**
  eqsat `packages/mlopt-orchestrator`.
- *Gate:* `kit-prelude` pipeline example green end-to-end +
  `pytest tests/` in mlopt.
- *Blast radius:* mlopt (`stage.py`, tests) + eqsat
  (`SearchResult` field tolerance — additive, non-breaking) +
  kit-mlopt crate (schema pin is additive).

### Step B — `hnl mlopt` command parity, then ext death

- Extend `handl-kit-mlopt` (or kit command registration generally)
  to cover `models|optimize|export|doctor` — today only stage-ops
  exist. Each forwards to `python -m mlopt <cmd>` initially (same
  subprocess pattern as stage-ops); the Python package remains the
  edge.
- *Gate:* `hnl mlopt optimize <ref>` ≡ `python -m mlopt optimize
  <ref>` on a smoke ref; `hnl mlopt models`/`doctor` parity.
- Then delete `crates/hnl-ext-mlopt` and drop `HANDL_EXT_DIR`
  instructions for mlopt.
- *Blast radius:* kit-prelude (new commands) + mlopt (crate deletion,
  AGENTS/docs) + host-cli docs. **Blocked on Step A only for the
  stage verbs; the other verbs are independent.**

### Step C — emit table canonicalization (before any physical move)

- Generate `emit._OPS`' signature/binding half from
  `host_surface.json` (the `torch_port_gen.py`/`emit_views` pattern
  from ADR-0178); keep hand-written *rendering* builders, generated
  *specs*. `carrier_expand.py` unchanged.
- *Gate:* `test_emit_surface.py` drift suite + `hnl-run` acceptance
  re-run on the 3 proven arch families (`--verify-run` where cheap).
- *Blast radius:* mlopt only — pure refactor behind existing drift
  tests. **Do this before Step D so the artifact that moves is
  already canonical.**

### Step D — repo dissolution per ADR-0175 (P3 gate)

Per the ratified map, in dependency order:

1. **graphopt split**: `mlopt-torch`/`mlopt-onnx`/`mlopt-cuda` →
   `kit-torch`/`kit-onnx`/cuda-sink kit; `mlopt-discovery` →
   discoverykit-side or kit data per its layer; `mlopt-host-tensor`
   (Python bindings) follows host-tensor's `lib-tensor` move;
   `mlopt-carriers` → kit data (carrier laws) or stays with
   kit-torch pending its own layer call.
2. **Rename graphopt → `mlopt`** (it already hosts the domain
   packages; the name is the product).
3. **Old mlopt merges in**: `src/mlopt/` lands as the kit's Python
   edge inside renamed-mlopt (e.g. `packages/mlopt` or top-level
   `src/mlopt` — packaging decision at merge time). `emit.py` +
   `carrier_expand.py` arrive **as codec** — legal per ADR-0174.
4. Archive the old `mlopt` repo after pins repoint.

- *Gate:* ADR-0175 phase gates — contents moved AND pins repointed
  AND suites green in the new homes; the mlopt merge waits for
  "after P0.5+P1" which are landed, so the real gate is the graphopt
  split completing.
- *Blast radius:* **largest step** — every downstream pin of
  graphopt/mlopt packages (`mlopt` product pyproject, kernelopt,
  kit-prelude's `MLOPT_PYTHON` path, docs). Mechanical but wide.

### Step E — D5/D6 tail (post-dissolution, not blocking D)

- Adapters go Term-native; `mlopt_core.ir`/`term_to_data` retire
  (ADR-0178 D5).
- **emit.py decision point**: with Terms native, evaluate porting the
  emitter to Rust inside `host-tensor` (it owns `tensor.*` +
  `host_surface.json`) or a `compile-*` repo. If the Python emitter
  still pays, it stays in merged-mlopt as codec — the ADR permits
  it. Decide on evidence, not schedule.
- `mlopt_native` retired (eqsat-side, D6); egraph stays oracle.
- *Gate:* ADR-0178 D4→D6 sequencing; differential harness stays green.

---

## 4. Blockers / open questions

1. **`handl-kit-mlopt` covers stage-ops, not commands.** `hnl mlopt
   optimize` has no kit path today — Step B is real work (command
   registration surface in kit-mlopt or platform), not a repin.
2. **`stage.py` ↔ `SearchResult.eg` coupling.** The placeholder
   `EGraph` is the only e-graph import left in mlopt; removing it is
   an eqsat protocol change — sequence it inside Step A, not as an
   afterthought.
3. **`emit.py` can't leave early.** It imports `mlopt_core.ir`
   (`Const, Op, Param, Var`, `shape_of`) — moving it to a Rust repo
   before D5 means either a Python package in a new home (packaging
   churn, no layer benefit) or a premature Rust port against a
   moving Term target. ADR-0174 explicitly allows codec in mlopt;
   the honest answer is *emit stays until D5, then decide on
   evidence*.
4. **`arch_rust`/`onnx_rust` adapter placement.** They wrap
   source-onnx binaries via env/sibling-checkout resolution —
   dev-machine-specific (`SOURCE_ONNX_LIFT`, `cargo build --example`
   fallback). At dissolution, decide: adapters stay product-side
   (Source ports — consistent with ADR-0176 edges) vs. the arch-spec
   machinery graduating into source-onnx itself. Recommend: stay,
   revisit if a second consumer appears.
5. **Name sequencing.** ADR-0175: graphopt→mlopt rename happens *as*
   the kit-mlopt home is created; old mlopt merges in "after
   P0.5+P1" (landed) but the merge order within Step D matters —
   old mlopt must archive or merge *before* the rename frees the
   name, or GitHub redirect semantics get confusing. Sequence
   sub-steps 3→2 or rename-with-redirect deliberately.
6. **`deliver_train.py`/`weights.py`/`play.py` tier question.** They
   straddle product (deliver board) and meta (training machinery).
   Current call: they stay — the board is product-specific and
   gamekit supplies only the generic engine. If a second board ever
   needs a trainer, revisit.

## Recommended first step

**Step A** — it's small, mlopt+eqsat only, removes the last
e-graph import from the product package, and lands the stage
`schema:` pin that Step B's command surface will version against.
Everything else gates on it or is independent.
