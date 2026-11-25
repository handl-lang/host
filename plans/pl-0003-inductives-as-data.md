+++
id = "PL-003"
title = "Inductives as data — inductive specs as first-class values"
status = "open"
created = "2026-11-25"
owner = "devin"
priority = "medium"
links = ["inductives-as-data.md", "0152-data-nominal-inductive-families.md", "0200-inductive-types.md", "graded-arrows.md", "pl-0002-mtt-modes-migration.md"]
+++

Make inductive type specifications first-class Handl values: the spec
(parameters, constructor signatures, result indices) that today lives
only inside a `data`/`new Inductive` declaration's AST becomes a
computable, passable, inspectable `Value` — while `type.family` /
`type.family_app` / `type.ctor_sig` application machinery stays
unchanged. User scoped this IN (2026-11-25); graded-arrows.md §3 had
explicitly deferred "inductives-as-data / W-types / generic recursors"
as research-grade — this plan is the scoped-down version of that
deferral: specs-as-data with generic elimination, NOT full W-type
theory.

Design record: [`design/inductives-as-data.md`](../design/inductives-as-data.md)
— the six design questions (spec representation, eliminators, indices,
positivity, universe, `new Inductive` relationship) with recommendations.
Read it first; this plan executes its recommendations.

## Honest scoping (read first)

- **In scope**: spec value representation; specs bound by
  declarations and inspectable as data; positivity as a checkable
  property of spec values; non-dependent generic recursor; dependent
  `match` driven by spec values (refactor of existing machinery, not
  new theory); indexed families through spec values.
- **P-sized**: representation, inspection/projection, positivity port,
  `data`/`new Inductive` minting a spec value alongside today's
  bindings.
- **M–L (the risk center)**: dependent elimination driven by spec
  *values* instead of AST scans — a two-engine (model `.hnl` + NBE
  Rust) parity refactor of `synth_match_arms`/`refine_scrutinee_`'s
  input source. Tractable without kernel work; expensive in breadth.
- **Research-grade / out of scope**: universe-polymorphic inductives
  (only predicative `Type0`-sorted specs), W-types/initial algebras,
  generic recursion-scheme derivation (`map`/`fold`/`bind` for free),
  spec-level equality/decidability theory, quotiented inductives.
- **Blast radius**: `data` decls and `match` are everywhere in the
  corpus — every phase must keep existing `data`/`match` behavior
  bit-identical until the spec-driven path is proven by parity suites.

## Verified current state (2026-11-25, main HEADs)

- Checker footprint of a `data` decl: `type.family{name, base}` bound
  to the family name; each ctor bound to
  `type.ctor_sig{ctor, family, params, args, result, bare}` whose
  `args`/`result` are **AST node indices** into the source AST
  (model `collect_ctor_specs_` ~2431–2555, `instantiate_ctor_`
  ~2556–2640; NBE `parse_data_node` ~2640, `DepCtor` ~2620,
  `instantiate_ctor_full` ~3569). Specs are re-elaborated per lookup
  for fresh index metas — this discipline must survive the move to
  values.
- Dependent elimination: `refine_scrutinee_` (model ~3138) +
  `synth_match_arms` (model ~5933; NBE `dependent.rs` ~6782) walk the
  `data` AST via `collect_ctor_specs_`/`collect_ctors` and unify
  per-arm index equations (`eq_*` witnesses). The algorithm is
  family-agnostic once it has ctor sigs — only the *source* of the
  sigs is AST-bound today.
- Runtime eliminator: `desugar_data` (tool-transform-rewrite
  `native/desugar.rs` ~1176ff) emits per-family `F.rec` fold machines;
  `desugar_match_to_recursor` ~1974 lowers ctor-pattern `match` to
  `T.rec(motive, c0..)(indices.., v)`; `.hnl` overlay
  `lib-desugar/src/index.hnl` ~1086–1401 mirrors it (`scan_data` /
  `lookup_decl` resolve ctor name → decl by scanning `data` nodes).
  `eval.rec`/`ast.rec` (`eval.yaml` ~830, `ast.yaml` ~950) is the fold
  node.
- Positivity: `check_strict_positivity` (desugar.rs ~752) + type-expr
  parser (~792–1010) — syntactic, in the desugar native; the `.hnl`
  desugar overlay mirrors it; the checkers never verify it (0152 §4).
- `new` declaration syntax is **in-flight** (agent b2ef7ae0,
  `handl-lang-mtt` worktrees): `new Algebra(C) {...}` landed as
  design (graded-arrows.md §3, commit 90935812 on `project`), `new
  Inductive` is decided but NOT yet in kit grammar/ast data (verified:
  no `keyword.new`/`ast.new` in `host/crates/handl-kit-prelude-lang/
  data/` at main HEAD).
- Universes exist: `type.universe{level}`, `construct_universe` /
  `universe_level` / `universe_max` (model ~6541); `synth_data` already
  synths param sorts into `type.family.base`.

## Worktree / merge strategy

Same pattern as pl-0002: implementation work happens in per-repo
worktrees/branches; docs commits go to the REAL `host/project`
worktree (`/home/luis/GitHub/handl-lang/host/project`, branch
`project`). Affected repos: `tool-typecheck-handl` (model),
`tool-typecheck-nbe` (parity — mandatory), `host`
(`handl-kit-prelude-lang` data + possibly kit globals for spec helpers),
`tool-transform-rewrite` + `lib-desugar` (desugar natives / overlay),
`tool-parse-pratt` or kit grammar only if new surface syntax is needed
(I2's `new Inductive` depends on b2ef7ae0's `new` work landing first —
sequence around it, do not block I1 on it).

- Model `.hnl` and NBE must move together per phase — the parity
  suites (`handl_typecheck_parity`, `dependent_structure`,
  `cubical_p*`) are the guard; run them, don't just `cargo test -p`.
- Keep model edits additive (new funcs/`synth_*` ops) to survive the
  model-parity agent's concurrent edits (same collision note as
  pl-0002).

## Phases

- [ ] **I0 design lock** — this plan + `design/inductives-as-data.md`.
  No code. Commits: project repo (this file + design doc).
  Verification: doc review; resolve open questions below with user
  before I1.

- [ ] **I1 spec value representation** — the `inductive.spec` value
  shape, inert.
  - Define the spec record: `{ name, params: [{name, sort}], ctors:
    [{name, args: [{name, type-expr}], result: type-expr|null,
    bare}] }` where `type-expr` is an **owned AST/`Term` value**
    (design doc §1 option a — NOT elaborated `type.function` chains;
    NOT AST node indices). Tagged or reserved-key record — decide at
    implementation, `__tag__`-style matches ctor objects.
  - `synth_data` (model ~7139 + NBE port) additionally *materializes*
    the spec value from the `data` node's ctor AST and binds it
    (under a derived name — `Name.spec` or a `specs` table entry;
    naming decided in I0) alongside the existing
    `type.family`/`type.ctor_sig` bindings. Existing bindings unchanged.
  - Type-expr values must carry enough fidelity for re-elaboration:
    spans for diagnostics, role/child structure for
    `instantiate_ctor_`'s domain/result walk. Verify round-trip through
    eval (they are data).
  - Repos: tool-typecheck-handl (model), tool-typecheck-nbe (parity),
    host (any kit data for the spec tag/shape if needed).
  - Verification: `cargo xtask gate` per repo; parity suites; new unit
    tests asserting `data` decls expose inspectable spec values; fixture
    re-sync (`cp` model → `tests/fixtures/typecheck_model.hnl` in
    tool-typecheck-handl AND host `handl-runtime` fixture copy).
  - Merge: model + NBE together.

- [ ] **I2 declarations mint usable specs + positivity as a check** —
  specs become real values users can hold.
  - `data`/`new Inductive` binds the spec to a name users can project
    (`NatSpec.zero`, spec field access, `Ind.ctors(spec)` helpers —
    helper functions live in a lib (`lib-type`?) as ordinary `.hnl`;
    decide placement in I0).
  - `new Inductive` (once b2ef7ae0's `new` lands): desugars to spec
    value + family/ctor registration — same bindings as `data` plus
    the spec binding. If `new` slips, `data` alone satisfies I2.
  - Positivity: port `check_strict_positivity` to operate on spec
    type-expr values; expose as a checker judgment
    (`check_inductive_spec`) in model + NBE, invoked when a spec value
    is *bound as a declaration* (declaration-time check, same timing as
    today's desugar native) AND re-checkable on computed specs at use
    sites that need it. Desugar native keeps its early-error mirror.
  - Repos: tool-typecheck-handl, tool-typecheck-nbe, host kit data,
    lib-type (helpers), tool-transform-rewrite/lib-desugar (only if
    the positivity mirror moves).
  - Verification: gates + smoke; new `.hnl` fixtures: spec
    inspection, positivity accept/reject on hand-built spec values.
  - Merge: model + NBE together; kit/lib helpers can land first (inert).

- [ ] **I3 generic non-dependent eliminator** — `Ind.rec`-style fold
  interpreting the spec value at eval.
  - Spec value gains/uses ctor identity: ctor objects already carry
    `__ctor__` syms (0152); spec records map ctor name ↔ index.
    Generic `rec(spec, motive?, cases…, v)` — or `Spec.match(spec,
    v, handlers)` — reads the spec, dispatches on ctor identity,
    applies the case. Replaces per-family emitted `F.rec` machines
    *for spec-driven matches*; existing `F.rec`/`match` desugar keeps
    working (migration is opt-in via spec values).
  - Runtime side: the fold is already a data-driven interpreter
    (`eval.rec` / lib-desugar `fold_step_fn` ~936–1062); generalize
    the ctor table input from emitted constant to spec value.
  - Checker side (non-dependent only): typing `Ind.rec` application —
    motive/case types checked against spec ctor sigs via
    `instantiate_ctor_`-equivalent on spec values. No index
    refinement yet.
  - Repos: tool-typecheck-handl, tool-typecheck-nbe (typing), exec/
    eval side for the generic fold (engine or lib-desugar `.hnl` —
    decide; `.hnl` preferred per engine-genericity), host kit data.
  - Verification: gates + smoke; fixtures for generic rec on Nat-like
    and multi-ctor specs; parity.
  - Merge: together with I2's spec binding (needs specs as values).

- [ ] **I4 dependent elimination over spec values** — the risk center.
  - Generalize `synth_match_arms`/`refine_scrutinee_` (model ~5933/
    ~3138; NBE `dependent.rs` ~6782 + scrutinee refinement) to take
    ctor sigs from a *spec value* (resolved from gamma/`results`,
    normalized if computed) instead of `collect_ctor_specs_`'s AST
    walk. `instantiate_ctor_` re-elaboration runs on spec type-expr
    values.
  - `match` on a spec-typed scrutinee where the family came from a
    spec value gets full dependent refinement (`eq_*` witnesses,
    absurd-arm detection) — same algorithm, new input source.
  - Indexed families (`Vec<n,a>`-style specs with index exprs in
    `result`) must work through this path; the `bare` ctor /
    unrecoverable-index `lit null` convention carries over.
  - Specs that don't normalize to a spec value at match time (stuck
    meta, non-spec) → honest-bound diagnostic (the established
    convention), never a guess.
  - Repos: tool-typecheck-handl, tool-typecheck-nbe — both mandatory,
    together.
  - Verification: gates + FULL parity suites (`handl_typecheck_parity`,
    `dependent_structure`, `axis_flow_boundaries`, `cubical_p1..p4`) —
    not optional here; new fixtures: dependent match over spec-bound
    families incl. an indexed family; spec-computed-at-runtime match.
  - Merge: single coordinated change across the two typecheck repos.

- [ ] **I5 computed/composed specs + polish** — the "everything is
  data" payoff, gated on I4.
  - Functions returning spec values; spec combinators as ordinary
    `.hnl` (e.g. sum of two specs, spec with renamed ctors — ONLY if
    trivially expressible over the spec record; no derived-eliminator
    schemes).
  - `new Inductive` corpus availability (if `new` landed); docs sweep
    (0152/0200 status notes, AGENTS-level docs); decide whether the
    AST-scan fallbacks (`collect_ctor_specs_` on `data` nodes,
    `scan_data` in lib-desugar) get retired or kept as the
    declaration-time path.
  - Explicitly NOT in I5: W-types, generic `map`/`fold` derivation,
    universe polymorphism — record as future work in the design doc.
  - Verification: gates + smoke + parity; corpus unchanged-behavior
    audit (`cargo test --workspace` on host is the regression net).

## Risks

- **Dynamic spec resolution (highest)**: a *computed* spec must be
  normalized by the checker at match/elimination time. Spec values are
  data so they normalize like anything else, but a spec containing
  closures/metas can stick — honest-bound fallback required, and the
  diagnostic must be tested.
- **Two-engine parity**: every spec-semantics change lands twice
  (model `.hnl` canonical + NBE production). I4 is where drift would
  hurt most; mitigate by landing I1–I3 with parity green first.
- **Type-expr fidelity loss**: today's AST node refs preserve spans
  and laziness; owned type-expr values must not degrade diagnostics
  (error spans pointing into `data` decls) or re-elaboration semantics
  (role resolution, `bare` convention, optional-role arity).
- **`new` dependency**: `new Inductive` syntax is b2ef7ae0's
  in-flight work — I2 sequences around it (`data` suffices); do not
  block I1/I3 on it, do not duplicate it.
- **Scope creep**: "specs as data" invites W-type/generic-scheme
  ambition. The line is drawn in Honest scoping — deriving eliminators
  generically is OUT; interpreting a user-supplied spec is IN.
- **Desugar/native duplication**: positivity currently exists twice
  (Rust native + `.hnl` overlay). I2 adds a third site (checker
  judgment) — keep them consistent or retire the mirrors explicitly.

## Open questions for user (resolve at I0)

1. **Spec binding name**: does `Nat := new Inductive {...}` bind `Nat`
   to the family former (today's behavior) with the spec under
   `Nat.spec`/`NatSpec`, or does the user write `NatSpec := ...`
   explicitly and register separately? Recommendation: former stays
   primary; spec is a projection/derived binding.
2. **Spec value tag**: `__tag__`-style internal tag vs plain record
   with reserved keys. Recommendation: tagged, matching `__ctor__` /
   `__former__` conventions.
3. **Generic eliminator surface**: builtin op (`Ind.rec(spec, ...)`)
   vs `.hnl` library function over spec inspection primitives.
   Recommendation: start `.hnl` for I3's non-dependent fold; promote
   to a judgment op only if typing needs it.
4. **Does `data` (legacy syntax) also mint spec values**, or only
   `new Inductive`? Recommendation: both — one code path, `data` is
   already the canonical decl.
