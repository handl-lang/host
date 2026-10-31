+++
id = "ADR-0157"
title = "ADR-0157: mlopt on the fragment tower — domain vocabularies as `Language` fragments"
status = "open"
priority = "medium"
+++

---
title: mlopt on the fragment tower — domain vocabularies as `Language` fragments
type: adr
date: 2026-10-24
status: proposed
applies_to: '`graphopt`, `kernelopt`, `eqsat`, `gamekit`, `kernel`'
---

# ADR-0157: mlopt on the fragment tower — domain vocabularies as `Language` fragments

  (`handl-language`, `hnl-engine`), `egraph`, `host` (`host-tensor`),
  `engine-transform-rewrite`, `engine-typecheck-nbe`

## Context

The mlopt discovery stack (eqsat substrate + graphopt tensor domain +
kernelopt codegen domain + gamekit play layer) measured its own
boundary this week and named it honestly: **the machinery is
domain-agnostic, but the generator space is hand-drawn.** Every
producer the machine searches is a designed template:

- the op vocabulary is a fixed Python registry (`op_def` in
  `mlopt_core` / sink `_IR_TO_TORCH` tables);
- law schema classes (`fold:*`, `mixed:*`, census mints) are
  hand-declared Python families (`mlopt_discovery.pipeline`,
  `handler_synth._FAMILIES`);
- handler grammars are fixed body templates (`ew`/`norm`/`contract`/
  `scan`);
- minted `synop_*` primitives register — but only into a Python-side
  map, and nothing downstream composes over them as mintable atoms.

Measured consequence (`docs/reports/primitives_finds.md`,
`prim_handlers.md` in graphopt): machine-originals appeared *only*
when the vocabulary grew — 21/24 `synop_*` folds classify
machine-original — and the residual frontier is exactly the shapes the
hand-drawn grammar cannot express (`select`/`linear`-bearing bodies
refuse at the handler gate). Composition inside a fixed vocabulary is
derivable-by-construction and provably cannot yield originality.

The principled fix is the platform's own thesis: **a vocabulary is a
`Language` fragment set; engines dispatch on data; a foreign producer
emits fragments the engines execute unchanged**
(`docs/site/explanation/language-as-data.md`). If the discovery stack
sits on that tower, minting *is* fragment emission and vocabulary
growth *is* the ordinary fragment merge — one mechanism at every
morphism level instead of per-level Python redesign. Conversely: if
the flagship research stack cannot live on the fragment tower, the
language-as-data claim is unproven on its hardest corpus.

### Verified current state

- `handl-egraph` (`egraph/` repo) is the ported `catopt_core.egraph`
  spec — union-find + congruence closure + compiled matcher +
  deterministic extraction + provenance, over the kernel's
  `Term`/`Value`.
- `mlopt-handl-peer` (graphopt `packages/mlopt-handl-peer`) is a
  working PyO3 seam: a mlopt fragment directory loads as a handl
  `Language`, all shipped laws load, `RewriteEngine` fires a rule at a
  named node, and the result agrees with `EGraph.apply_rule`
  (`tests/test_handl_peer.py`).
- `host-tensor` exists as a handl package (`modules: ["tensor"]`);
  the numeric contract it must satisfy is mlopt's fp64 verify leg.
- mlopt-side serializations already exist: `law_to_data`/`law_from_data`,
  handler mint records, `_FUSION_SCHEMAS` tables, `shipped_primitives`
  `_RECORDS` — the vocabulary is *already* data; it just lives in
  Python-side registries instead of a `Language`.
- The discovery loop (`bench/drivers/overnight.py` + kernelopt's port)
  is engine-parameterized: corpora `zoo-real`/`zoo-xmodule`/`zoo-prim`
  are already pluggable pools; `engine=` already selects Python vs
  optional Rust (`mlopt-native`) backends.

## Decision

Migrate the mlopt stack onto the fragment tower in five phases, in
dependency order — fragments first (data layer), bridges second,
semantics last. Each phase ships standalone value; none gates the
discovery loop on the next.

### Phase 0 — canonical fragment vocabulary

Emit the discovery vocabulary as a fragment directory — one
`Language`-shaped fragment set covering: op declarations
(`op_def`-equivalent rows), the law bank (`law_to_data` rows), handler
specs, `_FUSION_SCHEMAS`, `shipped_primitives._RECORDS`, and the
handler-grammar family tables. The fragment dir becomes the single
source of truth; the Python registries are *generated from* it (or
read it at import). Exit: `mlopt-handl-peer` loads the same dir both
engines use; a fragment added by hand or by machine is visible to
both.

### Phase 1 — handl as a first-class engine

`engine="handl"` seat beside the Python e-graph and `mlopt-native`:
laws execute through `handl-egraph`/`RewriteEngine`; the Python
engine remains the differential oracle (`handl_differential.py`
pattern — two engines, one must agree or the run refuses). Exit: an
overnight arm runs `--engine handl` with parity stats; divergence is a
loud refusal, never a silent difference.

### Phase 2 — denotations as Models

`synop_*` bodies and handler mints become handl handler records over
`host-tensor`. `TorchSink` remains the delivery path and the fp64
oracle until parity is measured; a minted op whose Model verifies is
"handl-shipped", otherwise Python-shipped — provenance records which.

### Phase 3 — mint emits fragments (the fix)

The mint's outputs become fragment emissions merged into the ambient
`Language`: a mined recurring shape emits an op row + denotation
fragment; a refused-spec analysis emits a handler-family fragment;
antiunify over failed mints emits schema fragments. The generator
space is then corpus-derived by construction — **the bank grows under
play at every morphism level**, which is the stated research claim
("a bank that grows under play") finally holding for generators, not
only rules.

### Phase 4 — play and saturation over the tower

`Board` seats whose move space is fragment application; `handl-egraph`
becomes the saturation engine pending the perf-parity gate; law
guards (`check`/`derive`/`cond`) migrate toward type-level predicates
as `engine-typecheck-nbe` matures (ADR-0152/0153/0154 lineage).

## Alternatives considered

- **Tower discipline in Python only** — give each generator a
  `{schema, merge, dispatch-on-data}` triple without migrating.
  Cheaper, fixes the search problem, but leaves the language-as-data
  thesis unproven on the flagship corpus and forks the vocabulary
  schema from the platform's.
- **Big-bang rewrite in handl** — rewrite the loop in `.hnl`. Rejected:
  no measured benefit over fragments-first, and it discards the
  working Python oracle that the differential checker depends on.
- **Status quo** — per-generator Python patches. Rejected by the
  measured evidence: each hand-drawn level (families, schemas) is
  precisely where the machine stalls.

## Consequences

- The fragment dir is the contract: schema evolution there is a
  breaking change for both engines — version it like `law_to_data`.
- `host-tensor` fp64 parity and `handl-egraph` saturation perf are the
  two hard gates; both are measurable before any semantic migration.
- The discovery loop gains a new honesty class: a mint that emits a
  malformed fragment is refused at the `Language` merge, not at a
  Python try/except — schema-validity is checked once, structurally.
- kernelopt (private) follows the same phases; its kernel-vocabulary
  fragments stay repo-private per the visibility closure (ADR-0150).

## Open questions

- Whether law guards should become type predicates (Phase 4) or stay
  a separate `cond` fragment class — depends on the typecheck-nbe
  roadmap's dependent-eliminator surface.
- Whether `shipped_primitives`/`MACHINE_SHIPPED` promotion stays a
  Python-side registry entry or becomes a fragment tag — decide at
  Phase 0 with the provenance owner.
- Cross-repo versioning of the fragment schema while `eqsat`/
  `graphopt`/`kernelopt` pin independently — the `uv.lock` bump
  pattern already exists; a fragment schema bump must ride it loudly.
