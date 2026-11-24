+++
id = "ADR-0173"
title = "ADR-0173: Term attrs are op metadata, program sharing is derived-form data, and non-finite floats are a codec tag"
status = "done"
priority = "high"
+++

---
title: Term attrs are op metadata, program sharing is derived-form data, and non-finite floats are a codec tag
type: adr
date: 2026-11-22
status: accepted
status_note: "Draft — Phase-0 design for ADR-0172. Answers the three calculus gaps the term_ir_spike measured (graphopt/docs/reports/term_ir_spike.md §5): attrs slot, DAG sharing, non-finite literals. Recommendation: one optional field on Term::Op; sharing stays out of the closed Term vocabulary (env leaves + engine hash-consing + an Opaque-carried dag record); __nonfinite__ adopted as a codec-profile convention, not a serde change. Implementation state (audit_pass_1): `Term::Op.attrs` landed across ~14 repos with serde contract tests; `__nonfinite__`/`dag.program` codec envelopes landed in graphopt v0.1.5 (`hnl_codec.py`). Ratified by user 2026-11-22."
applies_to: '`hnl-engine` term kernel (engine/src/term.rs, term_data.rs, term_interpreter.rs, nbe.rs), `handl-language` crate, `egraph` term_bridge, exec-* consumers, tensor Language fragment codec (graphopt term_hnl/hnl_import/hnl_surface)'
---

# ADR-0173: Term attrs are op metadata, program sharing is derived-form data, and non-finite floats are a codec tag

Phase 0 of ADR-0172. The spike proved the `Term` codec is lossless
but found three structural gaps before tensor DAGs can be Terms:
no attrs slot on `Term::Op`, tree-only Terms vs hash-consed DAGs,
and serde_json's rejection of NaN/±inf. This ADR decides each at the
calculus level. **Implementation status:** `Term::Op.attrs` is landed
and tested; the codec envelopes are landed in `hnl_codec.py`; ratification
is the remaining step.

## Decision

1. **`Term::Op` gains `attrs: Option<Value>`** — an optional,
   serde-defaulted metadata slot, kernel-generic ("an operation may
   carry declarative data"), *untyped* in the kernel: the `tensor`
   fragment (and any future domain fragment) declares the schema.
   `lit` keeps its documented role — the inline *literal operand* of
   `proj_env`/`proj_child` — and is not overloaded for attrs.
   Serialization of every existing term is byte-identical
   (`skip_serializing_if = "Option::is_none"`); old JSON parses with
   `attrs = None`.
2. **`Term` stays a tree. DAG sharing is expressed at three existing
   layers, none of them a new closed variant:**
   - **Weights/parameters → the environment.** Shared leaves are
     *named* (`Arr` leaf records / module-env bindings); the tensor
     program references `w12` by name and the environment holds the
     tensor once. This covers the dominant neural-net case (one
     weight feeding a dozen layers) with zero new machinery.
   - **Semantic sharing → engines.** Hash-consing/e-classing is a
     property of the *interpretation*: the e-graph already merges
     identical subtrees into e-classes, and the spike's
     `termjson_to_eg` adapter memoizes on canonical-subtree key so
     DAG→e-graph insertion is linear. Engines that need sharing
     intern; the syntax does not change.
   - **Wire compactness + direct DAG interpretation → a derived
     form.** A tagged record — `{"__tag__": "dag.program",
     "nodes": [...], "root": n}` (exact tag shape owned by the tensor
     fragment) — rides in `Term::Opaque` / `TermData`, the
     ADR-0103/0119 §7 fold for derived operators. The kernel carries
     it opaquely, `TermData::from_term`/`to_term` round-trips it
     exactly, engines that know the tag interpret the node table
     without materializing the tree, and engines that don't treat it
     as honest data — never a silent wrong answer. `.hnl` surface
     `let` may desugar to this record; the canonical wire *is* the
     record.
   - **No `Ref{name}`/let variant in the closed `Term` enum.** A
     name in an environment is not an arrow; a `Ref` variant would
     force every engine (tree-walk, bytecode, cranelift, nbe,
     typecheck, the egraph bridge — ~30 consumer files) to invent
     scoping/capture semantics the minimal core (ADR-0104b/0167)
     deliberately excludes. Any *syntactic* sharing requires names
     or ids; names are environment data and ids are DAG-wire data —
     both are fragment-level (`Opaque`), not kernel-level.
3. **Non-finite floats are a codec-profile convention:** the
   `{"__nonfinite__": "nan" | "inf" | "-inf"}` tagged object, adopted
   from host-tensor's wire codec, applied wherever a domain scalar
   maps into a `serde_json::Value` position (`Const.value`,
   `Arr.value`, `Op.lit`, `Op.attrs`). It cannot live *in* Term's
   serde layer — `serde_json::Value` cannot represent non-finite
   numbers at all (`Number::from_f64` returns `None`) — so it is a
   documented wire spelling in the tensor fragment's codec contract,
   sibling to the existing `__tuple__`/`__list__`/`__mlopt_attrs__`
   conventions. Producers must never emit bare `NaN`/`Infinity`
   (serde_json already refuses them at parse).
4. **The Phase-0 kernel diff is one optional field.** Everything else
   — the dag record, the non-finite tag, the attrs *schema* — is
   fragment/codec data. `mlopt_core.ir`, its ENodes, laws, costs and
   certificates are untouched (ADR-0172 §4).

## Why

- **Attrs are node identity, not operands.** The e-graph's node key
  was always the triple `(op, children, attrs)` — `ENode`/`Term::Op`
  in the `egraph` crate already carries `attrs` as first-class
  sorted pairs. The kernel `Term::Op` lacking the slot is the
  anomaly; `lit` was designed for literal *operands*, and overloading
  it collides the day an op needs both a literal operand and
  metadata. A generic `Option<Value>` field preserves ADR-0044
  genericity — the kernel stays domain-free; *typing* is declared by
  the fragment, exactly as op names and `Foreign` boundaries already
  are. Rejected alternatives: attrs-as-extra-`Const`-args (changes
  arity and every matcher), attrs-inside-`lit` under a reserved key
  (the spike's `__mlopt_attrs__` hack — works, but untyped, collides
  with `lit`'s documented role, and pushes a kernel concern into a
  payload slot), attrs-as-a-child-record (breaks the e-node walk).
- **Sharing is a representation property, and representations are
  layered.** Syntactic sharing (compact `.hnl`, compact wire) and
  semantic sharing (one weight, one e-class) are different concerns
  that happen to coincide in mlopt's hash-consed DAGs. Forcing both
  into the closed `Term` enum buys exponential-risk tree expansion
  *and* a non-arrow variant; keeping `Term` a tree and letting
  (a) the environment hold named leaves, (b) engines intern, and
  (c) `Opaque` carry a dag record gets all three without touching
  the calculus. The `Opaque` route is not a loophole — it is the
  designed escape: ADR-0119 §7 folded `Path`/`Equiv`/`CellId`/
  `Whisker`/… out of the closed vocabulary for exactly this reason,
  and `TermData::to_term` is total over unknown tags.
- **Cell/Whisker/Handle do not already provide sharing — checked.**
  `CellId`/`Whisker`/`CellCompV`/`Src`/`Tgt` are globular 2-cell
  machinery (identity and pasting of cells, ADR-0120); `Child`/
  `Handle` are effect projection/handling (ADR-0076/0081);
  `RecordWith`/`RecordProj` build and project *values*, and a
  record-of-terms still needs the record term spelled at each use —
  the sharing problem recurses. None give subterm identity.
- **Non-finite is already solved elsewhere.** host-tensor's codec
  proved the `__nonfinite__` wrapper; serde_json's `Value` makes a
  kernel-side representation impossible anyway, so the honest home
  is the codec profile. Old serialized Terms are unaffected — the
  tag is just an object spelling that parses today.
- **Blast radius stays small.** No new variant means no
  exhaustive-match breakage; the attrs field is compile-enforced at
  ~153 `Term::Op {…}` construction sites (35 files) — mechanical,
  and serde-compatible in both directions for *reading* (see
  Consequences for the one hazard).

## Not doing

- **No `Ref`/let/`Share` variant in `Term`.** Revisit only if a
  consumer needs *identity* semantics (two occurrences must denote
  the same node, not merely equal nodes) that e-class merging cannot
  express — no such consumer is known; the e-graph's congruence
  closure already *is* the identity.
- **No typed attrs in the kernel.** `attrs` stays `Value`; schemas
  live in `Language` fragment declarations (ADR-0044), with the same
  honest refusal taxonomy as op dispatch.
- **No serde shape change** to the `Term` enum, no
  `deny_unknown_fields`, no version marker — git-pinned deps make
  mixed-version wire a non-goal; the tensor codec profile owns
  conformance.
- **No changes to `Cell`/`Whisker`/`Handle`/`Child`** — they are
  unrelated machinery.
- **No migration work** — `mlopt_core.ir` remains canonical for
  search until ADR-0172's later phases.

## Consequences

- `engine/src/term.rs`: `Term::Op` gains the field; `desugar`,
  `eliminate_handlers`, `substitute_capability` clone it; new
  constructors (`op_*_attrs`) alongside the `lit` family.
- **Construction-site churn:** ~153 `Term::Op {…}` literals across
  ~35 files must add `attrs: None` (or a value): `engine/src`
  (term.rs, term_interpreter.rs, nbe.rs, term_data.rs),
  `kernel/crates/handl-language` (instantiate, language,
  capabilities, desugar, dynamic_handlers, law_inference,
  quantified_laws, fragment_set, fragments/*), `egraph/src/
  term_bridge.rs` (attrs ↔ `TermAttr` pairs both directions),
  `exec-bytecode`, `exec-cranelift`,
  `tool-typecheck-nbe`, `compile-js`/`compile-llvm` lowers,
  `host` plugins (`plugin-ext-refinement`, `plugin-lib-handl`,
  `handl-runtime`), plus tests. Compile-enforced; no semantic change
  when `None`.
- **Node identity:** attrs must join hash-cons/e-match keys wherever
  `Term` feeds an e-graph — `egraph::term_bridge::from_kernel` maps
  them to `TermAttr`s; `term_interpreter`/`nbe` pass them through.
  This is the *point*: `(op, children, attrs)` is the enode.
- **Serde compatibility:** old Terms parse (`attrs` defaults);
  terms with `attrs: None` serialize byte-identically. **Hazard:**
  serde ignores unknown fields, so an *old* engine reading new JSON
  silently drops `attrs` — mitigated by exact git pins and by the
  tensor codec profile declaring a format version; recorded here so
  it is a known, accepted risk, not a surprise.
- **`TermData` totality preserved:** the dag record rides `Opaque`;
  `is_hott_tag`/derived-tag tables gain the fragment's tag when the
  fragment lands — kernel untouched.
- **Codec work (graphopt side, separate change):** `attr_to_data`
  emits `__nonfinite__` wrappers; encoder emits dag records for
  shared subterms; `termjson_to_eg` learns the dag tag and the
  attrs field. Spike scripts (`term_ir_spike.py`) are the starting
  point.

## Open questions

1. `attrs` payload shape: `Option<Value>` (JSON object spelling) vs
   normalized `Vec<(String, Value)>` pairs (egraph-style). Recommend
   `Value` — JSON-natural, fragment-normalized at match time; confirm
   before implementation.
2. Tag name/record shape for the dag encoding (`dag.program` vs a
   `let`-family of records) — defer the concrete schema to the
   tensor-fragment codec ADR, or fix it here?
3. Should `TermData::to_term`'s unrecognised-tag→`Opaque` path get a
   conformance test pinning the dag record round-trip now, or when
   the fragment lands?
