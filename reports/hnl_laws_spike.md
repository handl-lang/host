---
title: 'Spike: can `.hnl` programs BE eqsat law definitions?'
type: report
date: 2026-11-15
---

# Spike: can `.hnl` programs BE eqsat law definitions?

**Question (user direction).** "Wouldn't eqsat laws be written in hnl IR
too? it is possible apparently!" — i.e. could laws be *authored* in the
Handl surface language and compile to engine-eqsat `LAW_FORMAT = 2`
records, instead of living as JSON?

**Verdict up front.** Yes, *expressible* — but only as **sugar that
compiles to the JSON record**, never as the canonical form and never
interpreted at match time. The decisive prior art already exists and
was measured: `discoverykit.handl` lowers mlopt laws into handl
`TransformDef` fragment rows, and the Phase-B report
(`graphopt/docs/reports/handl_law_bridge_phase_b.md`) quantified
exactly which law shapes survive that lowering. A `.hnl` *text*
surface is a further, smaller step beyond that — feasible for the
shallow-law subset, not worth it for the full `cond`/`dspec` guard
DSL. **JSON stays canonical** — which is also what the pending
ADR-0177 design doc already decided (§1: "`.hnl` is rejected" as the
record format; "`.hnl` may *embed* law records later"). This spike
found no evidence that should change that decision; it adds the
authoring-surface nuance below.

---

## 1. Expressibility — can a `.hnl` program express a law?

A `LAW_FORMAT = 2` law is `{lhs, rhs}` patterns in the `Pat` view
(`{mvar}|{const}|{op, dim, args, attrs, lit, source}`) plus a `cond`
predicate tree, `dspec` derive spec, tags, provenance. Three distinct
claims need separating:

### 1a. Does `.hnl` have pattern matching that fits?

`.hnl`'s `match` is **ADT/constructor dispatch on runtime values** —
`data` declarations, `__ctor__` objects, index-binder `<…>` patterns
(`host/tests/smoke/data_run.hnl`, `types.hnl`, `eval.hnl`). It matches
*values* a program evaluates, not compile-time term structures. Laws
match `Pat` trees over kernel `Term`s — a different IR level entirely
(the e-graph's internal `handl_egraph::Term`/`PNode` encoding,
`egraph/src/pattern.rs`, `engine-eqsat/src/pat.rs`). There is **no
existing `.hnl` construct** that says "match this op-tree shape, bind
these metavariables, require repeats to be equal". You would have to
*invent* the surface syntax — e.g. `law comm_add := add(a, b) ~> add(b, a)`
with `<>`-style binders or a dedicated `law` form — and a compiler that
lowers it to the record.

So: expressible as *new sugar*, not expressible in today's `.hnl`.

### 1b. What maps, what doesn't (measured, not assumed)

The honest expressibility data already exists — Phase B compiled the
**whole shipped bank** (144 laws + 58 expand premises) to handl
`TransformDef` rows and classified every law
(`discoverykit/packages/discoverykit/src/discoverykit/handl.py`,
report `graphopt/docs/reports/handl_law_bridge_phase_b.md` §4):

| Law feature | `.hnl`/TransformDef status |
|---|---|
| Root op + depth-1 child op kinds + arity | ✅ `node_kind` + `KindAt`/`ChildCount` |
| Metavar → child path binding (`KeepPath`) | ✅ first occurrence; rhs `Create`/`KeepPath` |
| Root-eliminating rhs (bare metavar) | ✅ `Replace { path }` (handl ADR-0127) |
| Attr presence / literal attrs / repeated attr metavars | ✅ since the attr vocabulary landed: `AttrEq`, `SameAttr`, `$attr_of` payload markers (`kernel/.../transform_def.rs:257-292`, `tool-transform-rewrite/src/step.rs:123,322`) |
| Attr-scoped `cond` predicates (`attr-eq/-is/-in/-eq-attr`, `attr-cmp ==`) | ✅ translated to the same clauses |
| **Ops at depth ≥ 2 in lhs** | ❌ `when` pins depth-1 kinds only — `spine_deep` (35 laws) |
| **Repeated *argument* metavars** (subtree equality) | ❌ binds first path only — `repeat_mvar` (14 laws) |
| **Concrete leaves at depth ≥ 2** | ❌ |
| **Shape/broadcast/dtype `cond` predicates** (`bcast-eq`, `axes-noop`, `flat-map-unsq`, …) | ❌ handl AST carries no shape oracle |
| `dspec`/`derive` (computed rhs attrs/bindings) | ❌ |
| `error_bound` approximate laws | ❌ |
| `Param` leaves | ❌ kind-key can't distinguish `Param` from same-named `Var` |

Phase B measured **89/202 ≈ 44%** faithful *before* the attr
vocabulary; the attr work closed `attr_rhs`/attr-guard gaps, so the
current faithful fraction is higher but the structural gaps
(`spine_deep`, `repeat_mvar`, shape guards, `dspec`) are unchanged —
those are *vocabulary* gaps, not spelling gaps, and a `.hnl` surface
doesn't fix them either (a `.hnl` law would still compile to the same
`cond`/`Pat` record, which engine-eqsat's `guard.rs` already
interprets natively — see §5).

### 1c. The key architectural point

The `.hnl`-as-*value* path is a dead end for laws: a law is compile-time
rewrite *specification*, not a runtime value. Encoding term patterns as
`.hnl` records and `match`ing them at eval time would (a) require
interpreting patterns per-firing — the performance non-starter in §3 —
and (b) duplicate the matcher `handl_egraph` already has compiled
(`PNode`/`Prog` with precomputed keysets and child-op constraints,
`egraph/src/pattern.rs:1-60`). The only honest `.hnl` role is
**authoring syntax → `Law`/`Rewrite` at load time**.

## 2. Compilation path — who compiles `.hnl` law → LAW_FORMAT=2?

Options, in order of increasing machinery:

1. **`.hnl` exports the record as data (zero new machinery).** A
   `.hnl` module whose top-level binding *is* the law-record value
   (records/arrays are first-class). A kit-eqsat loader evals the
   module once and feeds the value to `LawSet::from_json` /
   `Law::from_data` (`engine-eqsat/src/lawset.rs`, `law.rs`). This is
   "`.hnl` embeds law records" — already blessed by the design doc —
   but buys almost nothing over JSON: same nested `{mvar:…}` verbosity,
   plus an eval step. Not recommended as a feature.
2. **`.hnl` surface DSL → record (the real sugar).** A dedicated
   `law` form (new grammar fragment — data change per ADR-0044, but
   the *compiler* is new code) parsed by a host-side pass that emits
   the `Pat`-view JSON, then the existing `Law::from_data` path.
   Precedent for exactly this shape: `mlopt_discovery.hnl_import`
   (ADR-0172) parses `.hnl` *source* into mlopt terms at lift time —
   a scoped importer, honestly bounded. A law importer would be the
   same pattern: `.hnl` law file → `Law` → `LawSet`. Where it lives:
   **kit-eqsat tooling** (the kit layer owns laws/costs per ADR-0175's
   dissolution row), with the record decoder staying in
   `hnl-engine-eqsat` — the engine must not learn `.hnl`.
3. **Reuse the `TransformDef` path** — already built
   (`discoverykit.handl.law_to_handl` → `transforms.yaml` →
   `hnl-tool-transform-rewrite`). **Not viable as the law
   representation**: those rules run the *AST rewriter*
   (`Ast→Ast`, bottom-up/fixpoint — no e-classes, no saturation, no
   cost model) and, per Phase B, the unfaithful subset **over-fires
   unsoundly** (`chunk_single` deletes any chunk,
   `transpose_noop` deletes real transposes). The TransformDef is a
   *projection* of the law for the handl-rewrite oracle, not the law.

**Recommended path if sugar is wanted:** option 2, scoped to the
faithful subset — `op(args…)` patterns with `mvar`/literal leaves,
attr metavars, and `cond` embedded as the existing JSON tree (don't
re-spell the 88-op guard DSL in surface syntax — embed it verbatim,
exactly as `mlopt.law` already round-trips `cond`/`dspec` in the
TransformDef bridge). Anything outside the subset refuses loudly at
compile time, same posture as `LawError::GuardUnsupported`.

## 3. Performance — compile to Rust, never interpret

Hard requirement, and already satisfied by the target: `Law::compile`
lowers `Pat` → `handl_egraph::Rewrite` with patterns compiled **once
per e-graph** into `PNode`/`Prog` (interned leaf probes, sorted attr
programs, precomputed child-op constraints for targeted eligibility —
`egraph/src/pattern.rs`). Guards are parsed once into `Cond`/`Dspec`
ASTs and interpreted per firing over `BoundEnv` — cheap predicate
evaluation, not pattern matching.

Any `.hnl` authoring path terminates at `Law` → `Rewrite`; the `.hnl`
side is a **load-time** compile (laws load once per `LawSet`), so even
a slow parse is irrelevant. The forbidden design — evaluating a `.hnl`
function per match — is not on the table and shouldn't be: it would
also break the honesty properties (`serializable`/`missing_hooks`
refusals, `GuardUnsupported` declines) that make the record format
trustworthy.

## 4. Dogfooding — does `.hnl` actually simplify authoring?

Honest split:

- **For**: hand-authoring nested `{"op": "add", "args": [{"mvar": "a"},
  {"const": 0}]}` JSON is genuinely error-prone (the Phase-B fixture
  needed `priority` disambiguation for same-kind collisions; leaf
  spellings like `{"const": 0}` vs `{"mvar": …}` are easy to botch).
  A `add(a, 0) ~> a` spelling *is* nicer — this is the same argument
  that justified `hnl_import` for terms.
- **Against**: the laws that most need authoring help are exactly the
  ones `.hnl` can't help with — deep spines, repeated metavars, shape
  guards. For the shallow subset that *is* expressible, JSON is
  already data and the record is machine-generated in practice
  (`dump_fragments.py`, `law_to_handl`, minted stores emit records,
  humans rarely hand-write them — the shipped bank is
  machine-authored). The audience for hand-authoring is small; the
  audience for *reading* laws benefits more from the JSON record +
  docs than from a second spelling.

**Assessment: legitimate sugar, marginal value.** If pursued, scope it
to the faithful subset and keep JSON canonical — do not let `.hnl`
become a second source of truth (the Phase-B lesson: two spellings of
one law drift; the bridge stayed honest only because `mlopt.law`
carried the record *verbatim* as authoritative).

## 5. Prior art — are law guards just `TransformCondition`s?

Partially — and this overlap is already exploited:

- `TransformCondition` was **extended for law guards**: `AttrEq`,
  `SameAttr` (repeated attr metavars), and the `$attr_of` payload
  marker (rhs attr bindings) are explicitly documented as "the mlopt
  bridge's encoding" (`kernel/crates/handl-language/src/fragments/
  transform_def.rs:78-86, 257-292`; resolved in
  `tool-transform-rewrite/src/step.rs`). `discoverykit.handl.
  _cond_condition` translates the attr-decidable fragment of the
  `cond` DSL into these clauses.
- But the guard DSL is much larger: engine-eqsat `guard.rs` (2.4k
  lines) interprets ~88 `cond`/`dspec` ops — shape inference
  (`shape.rs`, 3k lines), broadcast/dim reasoning, tensor-channel
  predicates. `TransformCondition` covers only the attr/scalar
  fragment; shape/bcast/dtype predicates have no handl-AST analogue
  (no shape oracle on ASTs). **Law guards ≠ TransformConditions**;
  they share a deliberately-built subset.
- The full prior-art chain: `discoverykit.handl` (law→TransformDef,
  lossless-by-construction via the embedded `mlopt.law` record) →
  `scripts/dump_fragments.py` → committed fixtures
  (`graphopt/tests/fixtures/handl-spike/fragments-all/transforms.yaml`,
  15.6k lines — the whole bank as handl data) →
  `kit-data-prelude/tests/catopt_fragments.rs` (proves a catopt
  "language" assembles from data) → `mlopt_discovery.handl_engine`
  (differential oracle) → Phase-B report (the measured verdict above).
- Reverse direction also exists: `hnl_import`/`hnl_surface` lift `.hnl`
  programs *into* mlopt terms (ADR-0172) — terms-as-`.hnl`-syntax is
  already a shipped, bounded bridge.

## 6. Recommended design

1. **JSON `LAW_FORMAT = 2` stays the canonical law record** —
   consistent with ADR-0177 §1 (already decided; this spike confirms).
2. **Optional `.hnl` sugar, compile-time only**: a scoped importer in
   the kit-eqsat layer — `.hnl` law file → `Pat`-view JSON →
   `Law::from_data`. Surface covers: op patterns, `mvar`/literal
   leaves, attr metavars + literals, `cond`/`dspec` embedded verbatim
   as JSON. Anything else → loud refusal at load (never approximate).
   Effort: **~1–2 weeks** for the subset (parser is the cheap half —
   `hnl_import`'s `_Parser` is a working template; the `Pat` emitter
   is mechanical). Re-spelling the full guard DSL: **not worth it**
   (months, and the JSON embedding already covers it).
3. **Do not** route laws through `TransformDef`/`hnl-tool-transform-
   rewrite` as the representation — wrong semantics (AST rewrite, not
   e-graph law), measured unsound on the unfaithful subset. The
   TransformDef projection remains useful *only* as the handl-side
   differential oracle, gated to the faithful subset per Phase-B §6.1.
4. **ADR?** No new ADR warranted. ADR-0177's decision stands; if the
   sugar ships, it deserves a short section in kit-eqsat docs (or a
   one-paragraph amendment note to ADR-0177), not a new ADR — it's an
   authoring convenience over an already-contracted format.

## Evidence index

- `engine-eqsat/src/law.rs` — `Law`, `LAW_FORMAT=2`, `to_data`/`from_data`
- `engine-eqsat/src/pat.rs` — `Pat` authoring view + bridge encoding
- `engine-eqsat/src/guard.rs`, `shape.rs` — cond/dspec interpreter (~88 ops)
- `engine-eqsat/src/lawset.rs` — `LawSet::from_json` loader
- `engine-eqsat/docs/design/p2-search-contracts.md` §1 — ".hnl rejected" decision
- `egraph/src/pattern.rs`, `term.rs` — compiled `PNode`/`Prog`, `Term`
- `kernel/crates/handl-language/src/fragments/transform_def.rs` —
  `TransformDef`/`TransformRule`/`RewriteStep`/`TransformCondition` +
  `$attr_of`/`AttrEq`/`SameAttr` law-guard vocabulary
- `tool-transform-rewrite/src/step.rs` — `$attr_of` resolution
- `discoverykit/packages/discoverykit/src/discoverykit/handl.py` —
  `law_to_handl`/`store_to_handl`, gap classifier, `Native` refusals
- `graphopt/docs/reports/handl_law_bridge_phase_b.md` — measured
  expressibility (89/202 pre-attr-vocab; gap taxonomy §4)
- `graphopt/scripts/dump_fragments.py`,
  `graphopt/tests/fixtures/handl-spike/fragments-all/transforms.yaml` —
  whole-bank TransformDef dump
- `kit-data-prelude/crates/handl-kit-data-prelude/tests/catopt_fragments.rs`
  + `tests/data/catopt-fragments*/` — catopt-as-data proof
- `graphopt/packages/mlopt-discovery/src/mlopt_discovery/hnl_import.py`
  — `.hnl`→term importer precedent (ADR-0172)
- `host/tests/smoke/{data_run,types,eval}.hnl` — `.hnl` match = ADT
  value dispatch, not term-pattern matching
