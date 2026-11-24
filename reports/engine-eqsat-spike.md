---
title: 'Spike: P2 `engine-eqsat` — the e-graph as a search interpretation over kernel `Term`s'
type: report
date: 2026-11-15
---

# Spike: P2 `engine-eqsat` — the e-graph as a search interpretation over kernel `Term`s

Date: 2026-11-2x · Status: feasibility spike (design report + prototype
evidence, not production) · Serves ADR-0174 phase P2 / ADR-0175
dissolution row `eqsat → engine-eqsat-* + kit-eqsat`.

Prototype: `handl-lang/egraph` `tests/engine_eqsat_spike.rs`
(commit `fc47534`, local only — **not pushed**). Two tests, both
passing:

```
test attrs_payload_round_trips_through_a_rewrite ... ok
test saturation_loop_closes_over_kernel_terms ... ok
```

## TL;DR

**The critical-path risk is already retired.** `handl-egraph`
(`handl-lang/egraph`, crate `handl-egraph`) is a finished, tested Rust
port of the Python e-graph spec — union-find + congruence closure,
compiled pattern matcher, bounded saturation with rule budgets and
stop policies, deterministic DAG-aware extraction, and
`ProofEdge`/`Application` provenance — with a **total bridge**
(`src/term_bridge.rs`) between `hnl_engine::Term` and the crate's
internal `Term`. ADR-0175's "P2 = engine-eqsat, NOT the egraph crate"
is not a gap; it is a layering statement: `engine-eqsat` is a new thin
*interpretation* crate that consumes `handl-egraph` as a library,
exactly as `hnl-tool-transform-rewrite` consumes
`hnl-tool-transform`. The prototype proves the loop the
interpretation needs already closes end-to-end over kernel `Term`s:
lift → saturate → extract → lower → provenance, with the P0.5
`attrs` payload rewrite-transparent.

What remains genuinely new for P2: the interpretation shell (law
loading, cost-model binding, the `hnl optimize` surface), a serialized
law/pattern format (`kit-eqsat` data), and the migration/differential
story against `mlopt_core.egraph`.

## 1. Where does `engine-eqsat` live?

**Recommendation: a new `handl-lang/engine-eqsat` repo** (crate
`hnl-engine-eqsat`), per the ADR-0175 map — `eqsat` dissolves into
`engine-eqsat-*` (INTERPRET layer) + `kit-eqsat` (KIT layer, laws/costs/
minted.db as data). Do not extend the `egraph` crate and do not route
through `mlopt-native`.

Reasoning, against the three candidates:

- **Extending `egraph`**: wrong layer. The crate's contract is
  "substrate only — domain alphabets arrive as data, never baked in"
  (`egraph/README.md`). Laws, cost models, and the optimize command
  are domain. Folding them in would recreate the monorepo smell the
  dissolution map exists to remove.
- **`mlopt-native` (maturin/PyO3)**: solves the wrong problem. It is
  eqsat's optional Rust *engine backend* for the Python e-graph; P2's
  point is that search interprets kernel `Term`s natively, with
  `egraph::Term` engine-internal. Keeping a Python host would
  perpetuate the dual-IR census ADR-0174 §5 is trying to shrink.
- **New `engine-eqsat-*` repo**: matches the INTERPRET-layer pattern
  every sibling already follows (`exec-*`, `tool-typecheck-*`,
  `tool-transform-*`, `compile-*`). It depends on `hnl-engine` +
  `handl-language` + `handl-egraph` as git deps with the standard
  `[patch]` dev-link tables, and exposes kernel-`Term`-only APIs.

The one honest complication: **rewrite patterns need metavariables,
and kernel `Term` has none.** `Term::Var`/`TermAttr::Meta` exist only
in `handl_egraph::Term`. Two ways to square this with "egraph::Term
stays engine-internal":

1. `engine-eqsat` deserializes law patterns (kit data, JSON or `.hnl`)
  into `handl_egraph::Term` *inside its own crate boundary* — the type
  never appears in its public API. This is the natural reading of the
  ADR and what the prototype does implicitly.
2. Define a metavariable surface syntax over kernel `Term` (e.g. a
  `Child{role}`/`Opaque` convention) — more machinery, no payoff yet.

Option 1 is recommended; the pattern *format* is the main open question
(§6).

## 2. What porting is needed?

The Python spec (`eqsat/packages/mlopt-core/src/mlopt_core/`) splits
cleanly into substrate (already ported) and domain (stays data):

| Python (`mlopt_core`) | Lines | Rust status | P2 disposition |
|---|---|---|---|
| `egraph/core.py` (EGraph, matcher, saturation) | 1,785 | ported (`egraph.rs`, contract-tested) | done |
| `egraph/extract.py` (extraction, dag_cost) | 1,178 | ported (`extract.rs`) | done |
| `egraph/match.py`, `terms.py`, `types.py`, `certs.py` | ~1,277 | ported (`pattern.rs`, `term.rs`, `value.rs`, `subst.rs`) | done |
| `egraph/proof.py` (certificate reconstruction) | 709 | **not ported** — crate ships level-2 provenance (`ProofEdge`/`Application`) which is what this layer replays | decide: port only if verified delivery needs level-2 certs (§6) |
| `laws/` — `cond.py` guards/dspec, `tensor.py`, `priorart.py`, `headshare.py`, `pairing.py`, `scan.py`, `specials.py`, `attention.py`, `layout.py`, `factored.py`, `ruleset.py`, `serialize.py`, `minted.py`, `store.py`, `provenance.py`, `tags.py`, `base.py` | ~22k | n/a — domain | → `kit-eqsat` data (serialized laws + guards) |
| `cost/` — `basic`, `executor`, `roofline`, `fusion`, `params`, `backend` | ~3.1k | n/a — domain | cost models as kit data → Rust `cost_fn` closures or serialized cost tables |
| `typing.py`, `ir.py`, `meta.py`, `search_env.py`, `cells.py`, `pipeline.py`, `rulecache.py`, `trajectories.py`, `oracle.py`, `opmeta.py`, `ports.py`, `backends.py` | ~10k | n/a | P3 migration territory; `ir.py`/`meta.py` demote with the typed view |

The Rust crate is ~4,300 lines of `src/` + 546 lines of tests covering
the entire substrate half. **The porting job for P2 is not code — it
is data plumbing**: get laws and cost models to the engine as
serializable patterns + hooks.

Two representation-coupled spots to watch:

- **Guards.** Python `cond.py` side conditions are arbitrary Python
  (`check`/`derive`/`dspec`). The Rust `Rewrite` has `with_check` /
  `with_derive` over `BoundEnv` — closures, i.e. compiled code. Kit
  data cannot ship closures; guards must either be a small
  interpretable predicate language or stay Python-side behind a seam.
  This is the single largest porting decision (§6).
- **Cost models.** `cost_fn: FnMut(&Term) -> f64` is likewise a
  closure. Same question: compiled-in cost tables (baked per kit) vs
  an interpretable cost DSL vs Python callback.

## 3. Performance reality

Python-over-Term-JSON is **not** viable for production search, and the
numbers say so from both sides:

- The Python e-graph does the heavy lifting today and it is slow by
  construction: eqsat's own notes record hours-scale saturations — a
  single `div_one_sqrt` `apply_rule` enumeration observed >10 min, a
  CI run pegged a core >4 h (`eqsat/AGENTS.md`, `test_ported_square_rsqrt_laws`).
- The `term_ir_spike` measured ~12 ms to transport a 6.5k-node term —
  fine for batch lift/lower at pipeline boundaries, fatal per-rewrite.
- The Rust port exists *because* of this: it is the named port of the
  spec, "by contract, not by translation," with the Python kept as
  the spec. Saturation in Rust is the only path that preserves the
  current search budgets (`max_nodes` 100k default, rule budgets,
  improving-stop patience) at interactive timescales.

**Conclusion: Rust-over-Term is the only viable production path, and
it is already built.** A Python e-graph over Term-JSON would be a
regression, not a first version. The honest scope for any Python
involvement is orchestration (kit data, minted.db, discovery loops) —
which is exactly the kit/meta tier's job.

## 4. Prototype evidence

`egraph/tests/engine_eqsat_spike.rs` — the thinnest possible
`engine-eqsat` stand-in, using only the crate's public API (which is
the API the interpretation will consume):

**Test 1 — `saturation_loop_closes_over_kernel_terms`.**
Builds `add(mul(Const 2, Const 1), Const 0)` as `hnl_engine::Term`
via `op_native`, lifts with `from_kernel`, saturates under two rules
authored in pattern space over the bridge encoding
(`add(?d,?x,0)→?x`, `mul(?d,?x,1)→?x`), reaches `FixedPoint`,
`extract_best` under a size cost, lowers with `to_kernel` →
`Const 2`. A fragment evaluator confirms the rewrite preserved
meaning; `merge_log` names both firing rules and `applications()` is
non-empty. **The full interpret-loop closes over kernel `Term`s.**

**Test 2 — `attrs_payload_round_trips_through_a_rewrite`.**
`tensor.reshape(x, attrs={"shape":[2,2]})` → rule
`reshape(?d,?x,?a)→flatten(?d,?x,?a)` binding the whole
`__attrs__` payload via `attr_meta`. The flattened member lands in
the root e-class and lowers back to a kernel `Op` with a
byte-identical `attrs` payload. **P0.5 attrs are rewrite-transparent
end-to-end** — the thing ADR-0173/0174 P0.5 bought is already
load-bearing in the search path.

What the prototype also surfaces (design findings, §5): the bridge
encoding is *not* what a law author wants to write — patterns must
pin the exact attr keyset and know that `dim` is child slot 0 and
that `Const` leaves are self-describing JSON keys. That ergonomics
gap is real and belongs to the law-format design, not the engine.

## 5. Design findings / gotchas for the interpretation

- **Attr matching is exact-keyset.** `pattern.rs` compiles the
  pattern's attr names into a `keyset` gate (`egraph.rs` ~L795): a
  pattern with `{source}` will *not* match a node that also carries
  `lit` or `__attrs__`. Law patterns must enumerate the full bridge
  attr set — or the law compiler must fill it in. This is the top
  footgun for ported laws: today's Python laws match a different
  (typed-view) representation entirely.
- **`dim` rides child slot 0.** Every kernel `Op` encodes as
  `op(name, [dim, ..args], attrs)`. The prototype binds it as `?d`
  and ignores it — fine for `add(x,0)→x`, but laws that *use* dim
  (graded ops, `~[n]~>` arrows) need `attr_meta`-style binding or a
  `check` on the bound class. Law semantics over dims is an open
  design point, not a mechanical port.
- **Constant patterns are minted, not written.** A `Const` leaf's
  key is the term's own JSON serialization (`leaf_term`). Law data
  should carry constants as JSON values and let the engine mint keys
  via `from_kernel` — never as raw strings.
- **`to_kernel` is total.** Unknown enode ops lower to native
  `Term::Op`; undecodable leaf keys lower to an honest `Opaque`
  marker. Extraction output always produces a kernel `Term` — no
  partial-failure mode at the boundary.
- **Provenance is already the right shape.** `ProofEdge` (rule, both
  class ids, subst, note) + `Application` (matched/rhs classes,
  `rhs_root_enode`) + `enode_app`/`enode_origin`/`enode_birth` is
  exactly the data a gauntlet/cert layer replays. `EGraph::offer`
  covers non-local passes (the `pair_pass` precedent); `quotient()`
  covers truncation level 1.
- **Saturation semantics are spec-faithful**: deterministic member
  order, `rule_budgets` for explosive rules (comm/assoc),
  `Stop::FixedPoint`/`Stop::Improving` with patience + `cost_fn`.
  Budget numbers can carry over from the Python tuning.

## 6. Open questions (for the P2 design doc, not this spike)

1. **Law pattern format.** JSON schema in `kit-eqsat`? A `.hnl`
   surface? Reuse of `laws/serialize.py`'s existing serialization as
   the starting point? Must express: metavariables, attr metavars,
   literal leaves (as JSON values), and eventually guards.
2. **Guards (`check`/`derive`/`dspec`).** Interpretable predicate
   language in kit data vs. compiled-in per-kit rule packs vs.
   Python callback seam. `cond.py` is 2.4k lines of Python
   predicates — the honest answer may be "guards stay Python for P2,
   engine-eqsat v1 ships unconditional laws," but that changes what
   the 71 machine-shipped laws can express.
3. **Cost models.** `cost_fn` is a closure. Options: bake cost tables
   into `kit-eqsat` as data interpreted by a small Rust cost
   interpreter; per-kit compiled cost packs; or keep extraction
   cost-free (size/nops) for v1. The Python `cost/` algebra
   (roofline/fusion/params) is the hardest data to make
   representation-agnostic.
4. **Certificates.** Does "verified delivery" (ADR-0174) require
   porting `proof.py`'s level-2 certificate reconstruction, or do
   `ProofEdge`s + `Application`s suffice until a consumer demands
   replayable certs? The crate README explicitly leaves this layer
   out.
5. **Command surface.** Is `engine-eqsat` a `TransformEngine`-style
   trait implementation, or a standalone interpretation with its own
   request type (`optimize(lang, term, kit) -> Term + stats +
   provenance`)? There is no `SearchEngine` trait today — P2 may need
   to mint one (in `engine/` or a new contract crate) or stay
   concrete.
6. **Differential harness.** Before `mlopt_core.egraph` demotes (P3),
   a corpus-level differential — same laws, same inputs, compare
   extracted terms and cost — is the migration evidence. Scope it
   now so P2 lands with the harness, not after.
7. **Non-goal check.** ADR-0174: "`egraph::Term` stays
   engine-internal." The interpretation's public API must speak only
   `hnl_engine::Term`. Any place a caller needs to *see* a pattern
   (law inspection, `hnl doctor`) needs a kernel-Term or kit-data
   spelling, not `handl_egraph::Term`.

## 7. Migration path from `mlopt_core.egraph` (P2→P3 sketch)

1. **P2 lands thin**: `hnl-engine-eqsat` crate — kernel-Term in/out,
   `handl-egraph` inside, law/cost loading from `kit-eqsat` data,
   provenance out. No Python e-graph changes.
2. **Law port, batch 1**: the hand-promoted + machine-shipped laws
   whose guards are unconditional or expressible in the chosen guard
   subset → serialized kit data. Differential-test against Python on
   the discovery corpus.
3. **Cost models**: start with size/nops + the cheapest cost tables;
   port roofline/fusion policies as data as the cost-DSL question
   resolves.
4. **P3**: `mlopt_core.egraph` demoted to spec/reference (it already
   is, de facto — the Rust port mirrors it); `mlopt_orchestrator`'s
   `search`/`optimize` entry points route to `engine-eqsat` for
   kernel-Term workloads and keep the Python engine for the
   typed-view/torch-side until kit-eqsat covers it. `eqsat` repo
   retires per the dissolution map.

## Appendix: files touched

- Created `handl-lang/egraph/tests/engine_eqsat_spike.rs` —
  committed locally as `fc47534` on `main` (**not pushed**; the file
  is scratch spike evidence, marked as such in its docstring).
- This report. No shipped package, no `Cargo.toml`, no public API
  was modified anywhere.
