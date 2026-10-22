# Phase-1 implementation spec — unified type system

Normative design: ADR-0106 (kernel, as amended) + 0152 (`data`) +
0153 (`~[n]~>` dimension index) + 0154 (identity/equality/universes/
literals/erasure). This spec is the work breakdown; it implements the
phases, in waves of ≤4 parallel workstreams.

## Workstream map

| WS | Scope | Repo(s) | Blocks |
|---|---|---|---|
| A | `Val` NbE domain + shared evaluator | `kernel` | everything |
| B | Checker: `infer`/`check`/`conv`/metas/imports | `engine-typecheck-nbe` | D, F |
| C | `data` → former/ctor syms + `F.rec` + `match` lowering | `engine-transform-rewrite`, `kit-prelude-lang`, `engine-typecheck-nbe` | E |
| D | `Hom(n,·,·)` + `>>>[k]` typing, n≤2 | `kit-prelude-lang`, `engine-typecheck-nbe` | — |
| E | `type.hnl` sym migration; ctor-sym values | `lib-type`, prelude | — |
| F | exec-transform erasure + final-artifact lint | `host` (`handl-runtime`, transform passes) | — |

Waves: **0: A** → **1: B ‖ C** → **2: D ‖ E ‖ F** (4 max concurrent).

## A — NbE domain (kernel)

- `handl-value`: add `Val` variants — `VRigid{lvl,spine}`,
  `VFlex{meta,spine}`, `VFormer{sym, args}`, `VArrow{params,
  body_def, binds}` (defunctionalized — data, no host closures),
  `VSym`, `VConst`. JSON stays the boundary encoding only.
- Conventions: de Bruijn indices in terms, **levels** in values;
  `Env` = persistent cons list; meta store `Vec<Option<Val>>` with
  a constraint field (literal metas, ADR-0154 §5) + occurs check;
  `VUnfold{name,spine,lazy}` for controlled unfolding (Kovács).
- `sym` gains deterministic seeding for declared names (0154 §1).
- Value gains the `type` link; types the `parent` link (0154 §2).
- Exit: domain round-trips JSON; `eval`/`quote`/`conv` compile and
  pass a smoke suite (identity conversion, meta solve, former apps).

## B — Checker on the domain

- `infer`/`check`/`conv` over `Val`; `conv` = sym identity → spine
  walk → meta solve → symbolic index solver (ring/linear — retained,
  scoped to `conv` only).
- `types.yaml` demoted to dispatch table: `node_kind → op name`;
  judgment ops are the spec (0154 §7).
- **Consolidation**: delete `typecheck_model.hnl`; port its unique
  coverage first — `synth_dep_former_` (type exprs just evaluate),
  `bind_imports_`, `dependent_structure`/`axis_flow_boundaries` tests
  → NBE tests. Delete `engine-typecheck-handl` once green;
  `Kit::typecheck_handl*` + `ext-typecheck` rewire to `typecheck`.
- `hnl typecheck` runs NBE; bootstrap `check_module` scope rules
  (`unbound-wire`, `unbound-export`, unused-import) move to
  `lints.yaml` as lint data.
- **Imports**: module typechecks to a signature record `Sig_m`;
  `stmt.import` binds `name : Sig_m` (named = fields, `*` = splice,
  `* as m` = record). `resolve`/dynamic → `type.unknown`.
- **Universe levels**: `Type(i) : Type(i+1)`, `Pi` level = max
  (0154 §4); literals as constrained metas (§5).
- Exit: `hnl typecheck` green on stdlib-size files at interactive
  speed; former `.hnl` fixtures pass as NBE tests.

## C — `data` per ADR-0152

- `desugar.data` emits: `F` = seeded sym former; ctor arrows →
  `{"__ctor__": cSym, …}` objects carrying `type: F(i⃗)` link;
  `F.rec` = dependent eliminator (motive `Pi(i⃗, F(i⃗) ~> Type)`).
- `match` lowers to `F.rec` application; index refinement = ordinary
  unification of ctor result indices; `absurd` unchanged.
- Strict positivity moves into the checker (it has the ctor sigs).
- Iota: `F.rec(m, …, Cⱼ x⃗) ⟶ caseⱼ x⃗ rec-results` — definitional,
  so `Vec(1+1) ≡ Vec(2)` by normalization.
- Exit: `dependent_structure` parity on the new pipeline; `{"tag"}`
  strings gone from ctor values.

## D — `Hom(n,·,·)` + `>>>[k]`, n≤2

- `Hom` nominal former, dimension index; `~[n]~>` sugar → it.
- `>>>[k]` typing rules + `id_k` for k≤2 (arrows + equations);
  `Path` defined as data (Hom + inverse + coherence).
- `>>>` fixed to real composition first (0106 amendment).
- Honest bound: n≥3 parses, stores, does not check (aspirational).
- Exit: `e : f ~[2]~> g`, `e >>>[2] eInv ~[3]~> id(f)` *parse & store*;
  1/2-cell checking real.

## E — `type.hnl` sym migration

- `Int := sym("type.Int", parent…)`-style seeded mints; `__tag__`
  bridge kept until consumers migrated; ctor tags become ctor syms.
- `Nat <: Int <: Number` via `parent` links; `type.is`/`instanceof`
  read the new links.
- Exit: `lib-type` passes its gate (`hnl-gate.sh`); consumers green.

## F — Erasure + artifact lint

- `exec-transform` pass: erase all type-level content from runnable
  form — former applications, metas, indices (generalizes ADR-0151's
  annotation erasure), then DCE.
- **Artifact lint**: final artifact = only 1-arrows + runtime values;
  a stuck/meta in a runnable position = error diagnostic (0154 §6).
  Runtime-minted cells allowed (laws-as-values).
- Exit: lint rejects a crafted stuck-meta artifact; stdlib green.

## Cross-cutting invariants

- One value space: checker and exec share `Val`; arrow *values* stay
  self-contained serializable data (`body_def`+`binds`) — closures
  exist only as domain machinery, never as language objects (0106).
- No kind-dispatch in Rust beyond the calculus vocabulary
  (engine-genericity rule); roles via `AstShapeDef`.
- Every type-level entity is an index on a nominal former; indices
  erase. `data`/`Hom`/`Arr`(future)/`Type(i)` — one mechanism.

## Open items (tracked, not blocking)

- `IO(sig)` typed-effect signature objects — decide scope (D/E
  follow-up).
- `sym` seeds for unexported formers; FFI boundary convention.
- `j` keyword for the J eliminator (surface sugar only).
- `type Name body` contextual parse.

## Progress log

- **2026-10-22 — WS-A first slice landed** (`kernel` repo):
  `handl-value/src/nbe.rs` — `Val` domain (`Rigid`/`Flex`/`Former`/
  `Nominal`/`Arrow`(defunctionalized `ArrowVal`)/`Universe`/`Const`/
  `Unfold`), `Spine`, persistent `Env`, `MetaStore` with
  `MetaConstraint::DescendantOf` (literal metas) + `occurs` +
  `force_head`, and **deterministic sym seeding**
  (`Sym::seeded`/`seeded_with_parent`/`is_seeded` — content-hash
  handle, high-bit marked, stable across processes, `thread_local`
  memo) + `Sym::from_parts`. 6 unit tests green.
  Next: `eval`/`quote`/`conv` over the domain (B), `data` lowering (C).
- **2026-10-22 (cont.)** — `conv()` + `quote()` landed in `nbe.rs`
  (commits `330ee1a`, `<this>`). `conv` = nominal heads + structural
  args + meta solving (occurs, mark/truncate, lazy Unfold). `quote`
  reifies `Val` → `TaggedValue` (`__neutral__`/`__former__`/`__type__`
  encodings). Domain complete for WS-A; next: `eval` over `Term`
  (`engine/` repo holds `Term`/`term_interpreter`) then WS-B.
- **2026-10-22 (cont.)** — `eval` over `Term` landed
  (`engine` repo, commit `338bd21`): `hnl-engine/src/nbe.rs` —
  `eval(term, input, &dyn OpHandler)` mirrors `term_interpreter`
  arrow semantics (input-threaded `Seq` axis 0, `First`/`Choice` on
  `Pair`/`Either`, `App` → `apply`, `RecordProj`/`RecordWith`);
  `>>>`[k≥1 or dim≥2] delegates to the handler as `cell_comp_axis`;
  `apply` = spine push (Rigid/Flex/Unfold) + former index push +
  residual closure for `Val::Arrow`/`TaggedValue::Arrow`
  (`OpHandler::apply_arrow` hook for engine-owned body reduction).
  Out-of-fragment variants → `NotImplemented`. 10 tests green.
- **2026-10-22 (cont.)** — WS-B slices 1–3 landed
  (`engine-typecheck-nbe` commits `880b6f6`, `95e265a`, `dbf9b0d`;
  `kernel` commit `c1187b2`):
  - `checker.rs` — `Checker`: `infer`/`check`/`conv`/`subtype` over
    `Val` (Γ level-indexed; nominal parent-walk subtyping; cumulative
    universes `Type(i):Type(i+1)`, `Pi` level = max; literals =
    `DescendantOf` metas + `default_unsolved`; `Telescope` former sigs
    with dependent index subst; `lift` = inverse of `quote` incl. the
    `__tag__` legacy bridge).
  - `judgments.rs` — `JudgmentOps` (`OpHandler`): constructors
    `nominal`/`former`/`universe`/`pi`/`hom`/`meta`/`literal`/`lift`/
    `quote`/`decl_parent`, judges `infer`/`check`/`unify`/`subtype`/
    `bind`/`lookup`, and pure index arithmetic (`add`/`sub`/`mul`/`div`
    — `Vec(1+1) ≡ Vec(2)` by evaluation).
  - Engine seam — `TypecheckContext.vcheck` + `vconv`/`vsubtype`/
    `vinfer`/`vbind`/`vlookup`/`veval` judgment ops wired into
    `eval_judgment`; `veval` runs sub-schemes through
    `hnl_engine::nbe::eval`.
  - kernel: `Sym::seeded_with_parent` upgrades a parentless memoized
    sym (same handle — mint order can no longer orphan a parent).
  Next: port `dependent_structure`/`axis_flow_boundaries` `.hnl`-model
  tests to NBE coverage; then imports (`Sig_m`), `types.yaml` demotion,
  `hnl typecheck` NBE, scope lints.
- **2026-10-22 (cont.)** — WS-B coverage-port *probe* (not landed):
  `engine-typecheck-handl/tests/dependent_structure.rs` was
  mechanically ported to `NbeTypecheckEngine` (20 tests): **1/20
  pass**. The failure inventory is the real WS-B remainder and is
  mostly WS-C-shaped: `synth_data` judgment op missing in NBE;
  `interpret_type_expr` has no `call` kind (dependent former
  application `Vec(n, a)` as a type expr); match index-refinement /
  `absurd` / `transport` / `J` are `.hnl`-model-only.
  Recommendation: port these against the ADR-0152 encoding (ctor-sym
  formers + `F.rec`), not the legacy `{"tag"}` pipeline — i.e. land
  WS-C (`desugar.data` emits former/ctor syms, match lowers to `F.rec`,
  positivity in the checker), then re-run the ported suite as the
  parity gate before deleting `engine-typecheck-handl`. The ported
  file is preserved at the working-tree level only (failing until
  WS-C); `axis_flow_boundaries.rs` not yet attempted.
- **2026-10-22 (cont.)** — WS-D first slice (`engine-typecheck-nbe`
  `9c0469c`): former signatures are `FormerSig::{Tel, Pi, Hom}` —
  judgment rules as data. `Checker::new()` auto-registers `type.pi` +
  `type.hom`. `infer_hom`: `Hom(1,A,B) : Type(max(levels))`;
  `Hom(2,f,g)` requires both cells' types `≡ Hom(1,a,b)` with
  convertible boundaries; `n ≥ 3`/non-literal `n` store unchecked
  (ADR-0153 honest bound). `Pi` = universe-max; `Val::Arrow` codomain
  → `CannotInfer` pending `apply_arrow` engine hook. 12 checker tests
  green (278 lib). Remaining WS-D: `>>>[k]` *term* typing (the
  `cell_comp_axis` op the eval delegates to), `id_k`, `Path`-as-data.
- **2026-10-22 (cont.)** — WS-F landed (`host` `bf8de09`, `kit-prelude-lang`
  `88a5c31`): the exec-transform erasure pass + final-artifact lint per
  ADR-0154 §6.
  - `handl_runtime::exec_erasure_pass` — `apply_exec_erasure` runs the
    named `"exec-erasure"` `RewriteEngine` pass (rules are
    `lang.transforms` data; the kit ships
    `transform.exec-erasure.type-annot`: `expr : T` → subject via
    `Replace [0]`, the AST-level generalization of ADR-0151). Output is
    compacted through `Ast::subtree` — orphaned type-level subtrees are
    dropped (the DCE half) and the root-is-last convention survives
    root-eliminating `Replace`s. `prepare_exec_artifact` = erasure → lint.
  - `handl_runtime::artifact_lint` — `lint_exec_artifact` /
    `check_exec_artifact` walk reachability (children +
    `ARENA_REF_PAYLOAD_KEYS`) and reject: `__former__`/`__meta__`/
    `__neutral__`/`__type__` encodings embedded in payloads, and
    undecided type-level node kinds (`lang.types` rule ∧ no `lang.eval`
    scheme ∧ carries child refs — `pi`/`sigma`/`forall`/`family`/`j`)
    with a `PairShape` clause-kind exemption (`arm`/`entry` are parent-
    interpreted structural data, not stuck nodes). `__sym__`/`__arrow__`/
    `__ctor__`/`__tag__` values are legal; runtime-minted cells
    unaffected (laws-as-values).
  - Wired via `prepare_exec_artifact` into `eval`, `eval_with_globals`,
    `eval_shared_with_globals`, `eval_shared_with_call_args`, `compile`
    — uniform across exec + compile engines; runs after
    `runtime_assert_pass`/`erase_pass` in `run_checked`.
  - Tests: 11 unit + 11 integration (`exec_artifact.rs`) green;
    `arrow_model.hnl` (stdlib-size) lints clean. Open edges: `j` and
    other grammar-produced surface forms with no eval scheme flag under
    the kind check (correct — stuck), but `absurd`/`fibrant`/`glue`/
    `univalence`/`alt` own no `type.*` rule so they still fall back to
    payload-junk eval (capability gap, not leakage); serialized
    sub-terms inside `body_def`/quoted payloads aren't linted; per-call
    prep on the `eval_shared_*` bridge adds one O(module) walk.
- **2026-10-22 (cont.)** — WS-C landed (rewrite `e8379a1`, nbe `fcf5fe3`,
  prelude-lang `c50af5d`): `desugar.data` and the NBE checker's
  dependent-data fragment per ADR-0152.
  - `desugar.data`: `F` binds a seeded-sym nominal former — `F(i⃗)` is
    `{"__former__": fSym, "indices": i⃗}` (parameter-free `F` is the
    object itself, no predicate arrow). Each ctor is a nominal arrow
    producing `{"__ctor__": cSym, fields…, "type": {"__former__": fSym,
    "indices": j⃗}}` — `{"tag"}` dispatch is gone; the recursor fold
    dispatches on `v.__ctor__` sym identity (`Sym::seeded("data.F.C")`).
    Result-index params are recovered at runtime from recursive fields'
    own `type` links (`xs.type.indices[j]`); implicit indices
    (`MkDbl`'s `2*n`) lower to `null` (type-level only). Positivity
    stays as an early mirror; the checker owns the check.
  - `engine-typecheck-nbe/src/dependent.rs` (new, ~2400 lines): the
    tagged-value port of the model's `synth_data`/`instantiate_ctor_`/
    `refine_scrutinee_`/`synth_match_arms`/`synth_transport`/`synth_j`
    fragment. `synth_data` binds `F : type.family{name, base}` + each
    ctor a `type.ctor_sig` telescope (params as index metas in Γ *and*
    `results`, ctor args self-bound to `var{name}` terms, strict
    positivity checked here). Index terms are `var`/`Bin`/`Un`/meta
    values: `eval_index` resolves `var` through `results` (self-binding
    is the rigid fixed point) with canonicalization (`x+0→x`, `2*?n`
    commutes to `?n*2`); `solve_index` covers linear `X±c`, exact
    division (`X*c ≡ t`), `X*X`/`X**k` integer roots — decided failures
    mark arms unreachable. `synth_match_arms` is now dependent
    elimination: per arm, instantiate the ctor sig → `family_app`,
    unify each index (rigid `var` scrutinee indices bind `name ↦ c_i`
    in `results` + `eq_<name> ↦ Path{name, c_i}`), bind pattern fields
    to the zonked domains, check the body (against `apply_motive` when
    a `return … with` clause is present); `absurd` arms are discharged
    on decided-impossible equations and rejected when reachable.
    `synth_transport`/`synth_j`/`synth_dim_arrow`/`synth_flow`/
    `synth_axis_flow`/`synth_call`/`synth_assign`/`synth_data` judgment
    ops wired; annotations and `assign` values route through
    `dep_synth_type` (previously `x : T = v` in last position was never
    checked — `type.assign` was a bare `construct type.unit`).
    `dep_check_intro`: `family_app` checks through index unification,
    arrows against `type.pi`/`type.family`/`type.sigma` via dependent
    introduction, `refl`/witnesses against `Path`.
  - `types.yaml` (kit-prelude-lang): `call → synth_call` (curried),
    `assign → synth_assign` (binds + checks the value — the legacy
    `construct type.unit` never evaluated children). `type.data` was
    already `synth_data` on main; now it exists.
  - **Parity: `tests/dependent_structure.rs` ported to
    `NbeTypecheckEngine` (fixture fixed to `../../lib-type/src/
    dependent.hnl`; `Err(LangError)` maps to an `error` diagnostic in
    the harness) — 15/20.** Passing: surface formers, reindexing,
    adjunction 2-cells, ctor checks/rejects, definitional index
    equality, family-in-codomain, both absurd arm cases, decided
    unreachable arms, non-path transport rejects, `Path(2,3)`/`refl`
    reject, `dependent.hnl`. Failing (5): `match_refines_scrutinee_
    index_per_branch`, `transport_consumes_branch_path_witness`,
    `j_eliminates_branch_path_through_motive`, `dependent_family_
    unifies_inside_function`, `absurd_discharges` — one root cause:
    `results` bindings (rigid `var` rewrites + `eq_<name>` witnesses
    installed by `dep_intro_arrow`/refinement) are empty by the time
    arm bodies evaluate, so `transport`/`j`/`absurd`-Dbl can't resolve
    paths; `synth_assign` also needs the 3-child `assign(target, annot,
    value)` lhs form.
  - Not done: WS-B `FormerSig`/`Telescope` `Val`-domain path is unused
    by this slice (tagged port, honest note); `bind_pattern_vars`/`FoldMode::
    Predicate`/`instantiate_ctor` (lang/ast-less) are now dead code —
    `#[allow(dead_code)]` or removal pending; `subst_term_` for
    dependent ctor domains (`P(x)`) unimplemented; `host` data eval
    tests still assume the predicate encoding (ADR-0154 §2 `type` links
    change ctor values — a follow-up sweep is owed).
- **2026-10-23 — WS-D second slice landed** (`engine-typecheck-nbe`
  commit `038e65a`; `lib-type` commit `e754221`):
  - `FormerSig::{Comp, Id}` — `cell.comp(k, a, b)` / `cell.id(k, x)`
    are term-level former applications (canonical data), typed by new
    `Checker::infer_comp_axis` / `infer_id`. `>>>[k]` law: `k ∈ {0,1}`
    on 1-cells → `Hom(1,a,b) ∘ Hom(1,b,c) → Hom(1,a,c)`; `k = 2` on
    2-cells → vertical composition `Hom(2,f,g) ∘ Hom(2,g,h) →
    Hom(2,f,h)`; boundary law `tgt(fst) ≡ src(snd)` enforced by `conv`;
    lower-dim operands promote degenerately; `k > m` (m ≤ 2) is a
    `Dim` error. `id_k(x) : Hom(k,x,x)` — `id₁` objects, `id₂` arrows,
    `id₂` on an object is `refl` (`Hom(2,a,a)`); `Hom(2,·,·)` now
    admits object endpoints under the degenerate-cell reading.
    `JudgmentOps` gained `cell_comp_axis`/`cell_id`/`refl` arms and
    `id` at dim ≥ 2 — the `engine/src/nbe.rs` `Seq` delegation seam is
    unchanged. Deep-axis (horizontal) comp types `Hom(2,f,g) ∘₀
    Hom(2,f',g') → Hom(2, f∘₀f', g∘₀g')`. 5 new checker tests green.
  - `lib-type/src/dependent.hnl` — `Path` declared as data:
    `Sigma(e : a ~[2]~> b, eInv : b ~[2]~> a, cohere₃)`; `id`/`Sigma`
    exports; assoc/unit/interchange laws stated as `~[3]~>` equations.
    `dependent_hnl_parses_and_typechecks` green.
  - Checked: dims ≤ 2 (Hom formation, comp boundary law, id levels).
    Stored unchecked: `Hom(n≥3)`, `>>>[k≥3]`, `id_k≥3`, symbolic axes,
    all `~[3]~>` coherence data. Open edges: `>>>` (axis 0, dim ≤ 1)
    still evaluates as application under `nbe::eval` — comp typing only
    materializes through the `cell_comp_axis` delegation (axis ≥ 1 /
    dim ≥ 2); `Val::Arrow` codomain inference still blocked on
    `apply_arrow`; the legacy tagged `synth_axis_flow`/`synth_dim_arrow`
    in `lib.rs` do not emit `Former{type.hom}` — the `veval` → `v*`
    bridge to route `~[n]~>`/`>>>[k]` through judgment ops is owed
    (WS-B seam).
- **2026-10-22 (cont.)** — WS-C follow-up: the 5 remaining
  `dependent_structure.rs` failures fixed (parity now **20/20**);
  `engine-typecheck-nbe` uncommitted→committed.
  - Root cause was *ordering*, not results scoping per se:
    `synth_assign_judgment` (and the duplicated assign paths in
    `synth_block`/`check_block`) synthesized the value *before*
    checking it against the annotation. `synth_function` then bound
    `match` scrutinee params at fresh metas, so the arms were checked
    under an unrefined meta scrutinee — `results` stayed empty and
    `eq_<name>`/`absurd`/motive refinement never ran. Fix mirrors the
    model's `assign` op: elaborate the annotation first and
    `check_node(value, ann)` directly for dependent formers
    (`type.pi`/`family`/`sigma`/`forall`/`Path`/`family_app`); plain
    annotations keep synth-then-`dep_unify`. `synth_block`/`check_block`
    now delegate to `synth_assign_judgment` (single implementation).
  - `synth_assign` learned the arrow-annotated lhs form: `f : A ~> B`
    parses as `assign[function{type-annot(f,A), B}, v]` — the whole lhs
    is the `A ~> B` type expr, name = annot subject. Also the 3-child
    `assign(target, annot, value)` desugared form.
  - `synth_match_arms` motive child: `child_role_index_for` returns a
    *child position* — it was used as an arena index (`var n` →
    `type.number`). Now mapped through `children[pos]`.
  - `dep_unify` `type.function`×`type.function` recurses via
    `dep_unify` (param/body) so `family_app` codomains index-unify;
    `effects` still structural.
  - `synth_call` `type.family` arm accumulates args onto `family_app`
    (term-position `Vec(i, Bool)` was "callee is not applicable" on the
    2nd arg — `_motive := (i2,w2) ~> Vec(i2,Bool)` in the `j` test).
  - `check_node` records the dependent-intro's *returned* type (keeps
    the transport node's `via` Path witness) instead of `expected`.
  - `dep_builtin_or_tag` bug: unknown type names minted `{"__tag__":""}`
    (the `""` err arm was unreachable). Now errors — which also
    repaired the 9 pre-existing `synth_type_annot` predicate-test
    failures on `fcf5fe3` (term-level `(n) -> n % 3 == 0` predicates
    now fail `dep_synth_type` → fall back to the eval-closure path).
  - Green: dependent_structure 20/20, lib 283/283 (+9 restored),
    parity 46/46, grade_type_dispatch, with_handle_type_e2e.
  - Open: `host` `data_constructor_eval` expects `{"tag"}` encoding —
    untouched here (`handl_runtime::desugar_pass`, not the rewrite
    native); the §2 sweep is still owed.

### 2026-10-22 — WS-B checker consolidation landed: `.hnl` model deleted, Sig_m imports, NBE canonical

- **engine-typecheck-handl deleted.** The interpreted `.hnl` adapter
  (`HandlTypecheckEngine` + the 6,162-line `models/typecheck_model.hnl`)
  is gone; the model is archived at `host/project/design/typecheck_model.hnl`
  as the record of the reference semantics (kept rather than deleted
  outright — it is the only prose-level statement of the intended
  semantics outside this spec). Unique coverage was already subsumed by
  the NBE suites (dependent_structure 20/20 on `e775ed3`, parity 46/46,
  lib 283): the `axis_flow_boundaries`/`block_inference`/
  `type_synthesized_basics` assertions either exist in `tests/parity.rs`
  or are superseded by the `synth_axis_flow`/`synth_call` op tests — no
  unique assertions needed porting beyond the import/signature suite.
- **`import` binds `Sig_m`.** `typecheck` accumulates `stmt.export` pairs
  into `ctx.exports` → `TypecheckResult.signature` (a `type.record`
  `fields` map). `stmt.import` → `synth_import` op: `import m` binds
  `m : type.module_sig{module, fields}`; named imports are field
  projections; `* as m` binds the record; `*` splices fields (+ bare-name
  qualified alias); `resolve`/dynamic specs → `type.unknown`.
  `type.module_sig` is an *open* record: `record_field_type` probes
  `<module>.<f>` through `ModuleLookup` (declared signature, else opaque
  meta-function when the op resolves, else error); `type.unknown`
  projects to `type.unknown`. `ModuleLookup::lookup_module_exports` is
  the host seam (`ModuleRegistry`/`ModuleRuntime` impls).
- **`types.yaml` stays a dispatch table.** `type.stmt.import` →
  `synth_import`, `type.stmt.export` → `synth_export` — named judgment
  ops alongside the existing `synth_*`/`check_*` ops; the semantic core
  lives in Rust, not combinator schemes.
- **`hnl typecheck`/`hnl run` → NBE.** `prelude_engine_registry`
  registers `"nbe"` (`NbeTypecheckEngine`); the kit's declared
  `runtime.typecheck: nbe` now takes effect — the old `.with_typecheck(
  "handl")` overrides are gone. `Kit::typecheck_handl*` removed in favor
  of `typecheck_with_prelude` (fresh NBE engine + module chain +
  wire/splice names seeded as `type.unknown` globals) and
  `typecheck_source`. `kit.canonical().typecheck` unchanged.
- **Scope diagnostics → `hnl lint`.** `lint.scope.unbound-export` joins
  `unbound-wire`/`unused-import`/`shadowed-binding` in `lints.yaml`;
  `plugin-ext-lint` delegates `unbound-wire` + `unbound-export` to the
  `check_module` scope pass (kept as the compatibility seam — the lint
  plugin and benchmarks still consume it).
- **`data_constructor_eval` → `__ctor__`.** The test asserted the retired
  `{"tag"}` template internals; it now asserts the ADR-0152 encoding
  (object body producing a `__ctor__` value with the arg *value*
  substituted) — a test fix, no runtime change.
- **Verified:** `import_sig` 10/10; `data_constructor_eval` 1/1;
  kit-prelude compiles clean. `Cargo.toml` `[patch]` sections repointed
  `engine-typecheck-handl` → `engine-typecheck-nbe` across the workspace.
- **Note:** parallel WS-E work (canonical `type.<Name>` seeds, `Sym.ty`
  links) landed in `kernel`/`engine-typecheck-nbe` during this slice and
  is committed under its own messages; commit `9e4f6b6` carries both the
  WS-E seed work and this slice's import/export machinery.
- **2026-10-23 — WS-E landed** (`kernel` `572491d`+reseal commit,
  `engine-typecheck-nbe` `9e4f6b6`, `lib-type` `ac439755`; host/kit-core/
  lib-type-handl changes swept into parallel WS-B commits `1f6ae8b4`/
  `e2b8723`/`5f43372` — content identical, attribution shared):
  - `handl-value`: `Sym` gains a `ty` link (membership; `parent` stays
    `<:` — 0154 §2); `__sym__` wire encoding carries both links.
    `Sym::seeded_with_links(seed, parent, ty)` set-once upgrade;
    `BUILTIN_TYPE_GRAPH` (data) + `ensure_builtin_type_nominals()`;
    `type_link` (x : T read — Sym.ty / object `type` field / `__tag__`
    bridge / literal defaults `1→type.Nat`, `-1→type.Int`, `1.5→type.Float`);
    `instance_of` = link head then parent-walk (nominal arg resolves via
    `nominal_head` — sym/former/`__type__`); `Sym::resealed` — stale bare
    mints re-resolve through the memo on link walks.
  - `handl-runtime`: `sym("seed")`/`sym("seed",parent[,ty])` seeded mints
    (+ `sym.seeded` alias); `instance_of` delegates to kernel; legacy
    name-role bridge kept for unseeded nominals.
  - `kit-core`: `type.<Name>` ops mint seeded nominals (replacing
    `__tag__` records); new ops `type.is_nominal`/`type.instanceof`/
    `type.ty`/`type.Unit`; FUNCS += `Type`/`Unit`.
  - `lib-type-handl`: `kind_is` nominal arm — `type.is(v, T)` =
    instanceof; record gains `Type`/`Unit`; tests assert the `__sym__`
    shape + both-link reads.
  - `engine-typecheck-nbe`: canonical `type.<Name>` seeds (capitalized,
    matching `data.<DeclName>` convention) in `infer_const`/`infer_literal`;
    `Checker::new` ensures the graph; `default_unsolved` → `type.Nat`
    for the `type.Number` anchor (0154 §5); `lift` `__tag__` arm routes
    through `canonical_tag_seed` (legacy `int`/`type.number` spellings
    lift to the same nominals).
  - `lib-type`: `Int`/`Nat`/`Float`/`Number`/`Str`/`Bool`/`Null`/`Obj`/
    `Array`/`Value`/`Type`/`Unit` are `type.<Name>()` seeded mints;
    `*N` prototypes deleted. Gates: handl-value 110 tests, nbe 283 lib
    tests, sym_module 11 tests — all green. `hnl-gate.sh` not runnable
    (no built `hnl` + ext .so's on this box).
  - Left over (bridge edges): `type.of` still returns kind strings;
    `__tag__` producers (`tag.tag`/`tag.construct`, dependent.hnl's
    tagged-model encodings) untouched — deletion is the later cleanup;
    `type.pi`/`type.hom`/`cell.*`/tagged-model tags keep lowercase
    seeds (machinery, not builtin-type names); a `{"__tag__":"custom"}`
    datum does not satisfy `type.Obj` (matches legacy roles-only
    behavior).
- **2026-10-23 (cont.) — integration pass: ordered value annotations
  landed** (`engine-typecheck-nbe` commit `44b18f9`). Fixes the WS-B
  regression where `f : inc(4) := v` was not checked against the
  evaluated annotation: `dep_synth_type`/`interpret_type_expr` rejected
  `call`-shaped annotations outright. Ported the model's
  `node_term_`/`eval_term_`/`apply_val_`/`eval_node_` fragment into
  `dependent.rs` (`node_to_ann_term`/`eval_ann_term`/`apply_ann_val`/
  `eval_ann_node`, fuel-bounded at 4096): every `synth_assign` now
  records `name ↦ <value term>` in `ctx.results` (the model's
  `tc_bind_result`), so `inc(4)` in annotation position lowers to
  `App{var inc, [4]}` (the `dep_synth_call_type` results arm — already
  present, previously unreachable), evaluates to `5`, and classifies:
  plain value → equality (`6` fails, `5` accepts); `LamVal`/`CompVal`/
  `__arrow__` → predicate application (`false` rejects, undecidable →
  `runtime_assert` diagnostic); type tag → the existing dependent/dep_unify
  path. Nested calls (`dbl(inc(4))`) and literal annotations
  (`x : 5 := 5`) work; unbound callees stay errors (stuck `App` fails
  `dep_unify`). New test `ordered_call_annotation_checks_equality`.
  Verified: lib 283/283, parity 46/46, import_sig 10/10,
  dependent_structure 20/21, handl-value lib 31, hnl-engine lib 13.
  **Pre-existing red (verified red on HEAD via `git stash`, NOT caused
  by this change):** `dependent_structure::
  dependent_hnl_parses_and_typechecks` — `transport: operand is not a
  Path (kind 'var')` on `export transport := (p, v) ~> (p # v)`: `p`
  is a function param bound at a fresh meta, and `resolve_path` only
  accepts `Path`-tagged results/Γ bindings — the `(p, v) ~> p # v`
  export added in `e754221` needs a `p : Path`-typed binder or an
  `is_meta → stuck-not-error` arm in `resolve_path`/synth_transport.
  Owed fix belongs to WS-C/D territory.
  **Still open from the gap list:** (2) the `veval`→`v*` bridge —
  `synth_dim_arrow`/`synth_axis_flow` still emit legacy `Arrow{inputs,
  grade, output, dim}` records rather than `Former{type.hom}`/
  `Former{cell.comp}` canonical data, and `unify`/`dep_unify` have no
  `__former__`/`__sym__` conv bridge (sketch: `lift_cell_ty` mapping
  `type.function`/`type.pi`/`Arrow` records → `Hom(1…)`/`Hom(dim,…)` +
  a meta-free domain-encoded arm in `dep_unify` → `ctx.vcheck.conv`);
  (3) `OpHandler::apply_arrow` still returns `None` (residual closure
  only) — `Checker::infer` on `Val::Arrow` is `CannotInfer("arrow")`
  and `FormerSig::Pi` with an arrow codomain is rejected; smallest
  correct version: `JudgmentOps::apply_arrow` fires when the last param
  binds, evaluates the `{"__ast__"}` `body_def` arena under
  `binds ∪ {param ↦ arg}` (var/call/binary/if/array/paren/function
  fragment), stuck shapes → `Former{stuck.*}` neutrals so `infer`
  reports `CannotInfer` honestly.
- kernel chore check: `kernel d526b3f` is a clean 2-line
  `[patch]` repoint (`engine-typecheck-handl` → `-nbe`), no WS-E WIP
  folded in; tree clean, no rebase needed.

- **2025-01-XX (WS-C/D: open transport + `veval`→`v*` bridge + `apply_arrow`)**
  `engine-typecheck-nbe 12f6e8b`, `kit-prelude-lang 524adf3`:
  - `resolve_path`: a metavariable-bound operand (`p` in
    `transport := (p, v) ~> (p # v)`) synthesizes an open
    `Path(?a, ?b)` — stuck, not a decided non-path — with fresh tagged
    endpoint metas; `via` is recorded only on non-meta results so an
    open transport stays an atomic `{"__meta__"}`. `refl` on
    `Path(2,3)` and decided non-path operands still reject.
  - `synth_dim_arrow`/`synth_axis_flow` now emit canonical
    `Former{type.hom}` data (`Hom(n, src, tgt)`, `Const` dim for
    `~[n]~>`, `Dim`-typed meta for symbolic dims) via a new domain
    bridge — `domain_lift` (tagged JSON → `Val` through
    `checker::lift`, with `__meta__` leaves remapped to fresh domain
    metas so the two stores never alias) and `domain_quote` (`Val` →
    `quote` boundary spellings). `flow_of` routes domain-encoded
    operands through the same law; `infer_comp_axis_tys` applies the
    boundary law on operand *types* (non-Hom types read as degenerate
    objects) with decided errors propagating and stuck shapes storing
    `Hom(d, …)` unchecked — the honest bound.
  - `unify` gained a domain-encoded arm (`__former__`/`__sym__`/
    `__neutral__`/`__type__`) → lift both sides, `ctx.vcheck.conv`,
    `store_mark`/`store_restore` on failure (top-level `__meta__`s
    still solve in the tagged store first — ordering matters).
  - `JudgmentOps::apply_arrow` evaluates a saturated arrow's
    `{"__ast__"}` `body_def` under `binds ∪ {param ↦ arg}` over a
    fuel-bounded tagged fragment (var/call/binary/unary/if/array/
    object/paren/function + dim_arrow/axis_flow/flow/cell_* forms
    evaluating to their canonical formers); open subterms produce
    `Former{stuck.*}` neutrals rather than errors. `Checker::infer`
    on `Val::Arrow` saturates params with rigid binders (fresh
    domain metas as domains) and infers the body — a `type.pi` tower;
    `FormerSig::Pi` evaluates an `Arrow` codomain on the domain's
    rigid. `JudgmentOps` carries the ambient `Language` (role lookups,
    `Ast::instantiate` for nested `function` bodies).
  - `types.yaml`: `type.cell_id` (`id2 x` → `Hom(2, x, x)`),
    `type.cell_comp_v` (`=>>` → axis-2 law), `type.cell_comp_h`
    (`=>|` → axis-0 deep law) → new `synth_cell_id`/
    `synth_cell_comp_axis` ops sharing `comp_axis_typed`.
  - `synth_object_record` now mints the tagged type when an object
    literal carries a `__tag__` key (the reserved key names the tag —
    the literal denotes `{"__tag__": t, …}` not a `type.record` field
    of `type.string`).
  - Verified: lib 283/283, parity 46/46, dependent_structure 20/21.
    **Remaining red — same test, deeper edge:**
    `dependent_hnl_parses_and_typechecks` — `transport`/`cast`/`cong`
    now clear; the failure moved to `Sigma := (A,F) ~> {"__tag__":
    "type.sigma", "param": A, "body": F}` — a `type.string` vs raw
    `"type.sigma"` unify where the tagged-literal object's *field
    types* are read as field *values* (`Sigma(A,B)` should produce
    `type.sigma{param: A, body: B}` as term data — the term-vs-type
    gap in the tagged surface, not a `__tag__` mint issue).

- **2025-01-XX (WS-C/D closeout: dependent.hnl green — 21/21)**
  `engine-typecheck-nbe` (single commit): `dependent.hnl` now
  typechecks cleanly end-to-end — `Sigma`/`Path`/`id`/`transport`/
  `comp_assoc`/`id_left`/`id_right`/`interchange` all synth to their
  canonical term-level type data. Four small edges closed, all inside
  existing machinery:
  - **Object-literal blocks (ADR-0079):** `{ "__tag__": "type.sigma",
    "param": A, "body": F }` in a `block_children` position (arrow
    bodies — `grammar.arrow.infix`) parses as a `block` of `type-annot`
    children, not an `object` of `entry` pairs, so `synth_object_record`
    (which already mints the tagged type) never ran. `synth_block` now
    detects the all-entry-shaped case and delegates — a `type-annot`
    child is an *entry* when its `name` echoes a literal-leaf subject
    (`pattern_role` absent — `var`/`arr` are pattern leaves and stay
    annotations; `value`/`lit` declare none). Role-driven
    (`subject`/`annot` roles), no kind hardcoding.
  - **Positional tuple domains in `synth_call`:** `(x, y, …) ~> body`
    mints `type.function{param: type.array{elements:[d1..dn]}}` but the
    call op checked every arg against the whole array — `Sigma(X, Y)`
    could never apply. Now each argument checks against `elements[i]`
    and the tuple shrinks until exhausted; `type.array{element: t}`
    (uniform/rest) still checks the argument as a whole array.
    (Pre-existing gap — `f(1,2)` on `(x,y) ~> x` failed identically on
    HEAD.)
  - **`check_node` vs unsolved metas:** the directed `check` schemes
    run `type_field(expected)` which yields `null` on `{"__meta__"}` —
    a `function` literal checked against `?t` never bound its params
    (`lookup: unbound variable`). Unsolved-meta expected now
    synth-then-unifies; zonking first (and again after synth — nested
    applications share callee metas) keeps solved cells on the
    directed path instead of re-solving stale cells (occurs-check).
  - **`type.function` intro via `dep_check_intro`:** a `function` node
    checked against `type.function{param, body}` goes through
    `dep_intro_arrow` (bind the name at the domain, check the body
    against the codomain) — gated on the params spelling a real binder
    (`pattern_role`/`subject`-role/variadic shape + `param_names`
    non-empty), so the structural `check` scheme still owns
    non-binder/synthetic params.
  - **Honest bound on open dims:** `comp_axis_of` treated a
    `Hom(?d, ·, ·)` operand as dim-1 (the `hom_dim` default), making
    `e >>>[2] eInv` on open cells a decided `Dim{2,1}` — but a
    meta-dimmed `Hom` is *open*, not dim-1. The composite now stores
    `Hom(?d, sa, tb)` unchecked. And `unify`'s domain arm treats a
    `Hom(m≥3, ·, ·)` mismatch as stuck (the `~[3]~>` coherence terms
    are stored-not-checked), not a decided conv failure.
  - `zonk_index` gained the same `visited` cycle discipline as
    `Checker::zonk` (mutual `?x:=?y`,`?y:=?x` solutions are reachable
    now that shared callee metas unify in nested applications).
  - Verified: lib 283/283, dependent_structure **21/21**, parity
    46/46, import_sig 10/10, grade_type_dispatch 1/1,
    with_handle_type_e2e 1/1, handl-value 31/31, hnl-engine 13/13,
    kit-prelude ordered_annotations 2/2. `handl-runtime`: all 44 test
    binaries — 41 green; `indexed_family_{data,fin,match}` overflow
    the 8 MiB test-thread stack inside `engine_exec_tree_walk` eval
    (deep `Vec.rec` recursor — zero typecheck frames in the
    backtrace; all pass at 64 MiB; pre-existing env limit, unrelated
    to this change) and `nominal_kernel` has 2 pre-existing
    `vm_sym_*` arity-validation failures (runtime `sym` op, untouched
    crate). `dependent_hnl_parses_and_typechecks` runs its check on a
    64 MiB thread — `Path`'s nesting legitimately needs ~10 MiB of
    check stack (every judgment hop is several `eval_judgment`
    frames).

- **2025-01-XX (phase-1 cleanup pass: stale tests + `__tag__` sweep)**
  `host 50cad5b5`, `kit-core 1c20216`, `lib-type 572d5e0`:
  - **`nominal_kernel` — fixed (was 17/19):** the two `vm_sym_*`
    failures were stale test expectations, not a broken primitive.
    `vm_sym_non_nominal_parent_errors` listed `json!("str")` as a bad
    parent — strings are now the ADR-0154 §1 seeded mint, not errors.
    `vm_sym_too_many_args_errors` asserted the old 0–1-arity message;
    `sym(root, 1)` now correctly errors "…-argument form takes a seed
    string first" and 4+ args error "expected 0–3 arguments". Added
    `vm_sym_string_arg_is_seeded_mint` pinning `sym("seed")`
    determinism (same seed → same handle; builtin `type.*` seeds
    excluded — `ensure_builtin_type_nominals` upgrades their links
    in place, so null-link assertions must use non-builtin seeds).
    **20/20 green.**
  - **`indexed_family_{data,fin,match}` — fixed via harness, root
    cause diagnosed (not the fold machine):** the 8 MiB SIGABRT was
    *not* deep `Vec.rec` eval — the fold machine is `Term::Loop`
    feedback, Rust-iterative (`fold_step` in
    `engine-exec-tree-walk::eval_inner_td`, MAX_ITERS guard). GDB on
    the crashing thread shows the overflow inside
    `Runtime::parse` → `apply_desugar` → `fire_native` →
    `handl:desugar.data` → `plugin-lib-handl`'s
    `eval_shared_with_call_args`: the ADR-0094 `.hnl` desugar overlay
    (`lib-desugar/src/index.hnl`) is a ~1250-line metacircular program
    evaluated by the tree-walk engine, nesting ~20–30 `apply_app`
    levels at ~600 KiB of unoptimized stack each (measured: fails at
    16 MiB, passes at 32 MiB). Legitimately deep metacircular eval —
    no re-threaded spine, no quadratic fold. Each test body now runs
    on a 64 MiB spawned thread (the `dependent_structure` pattern).
    All three green at default stack.
  - **`type.of` — migrated:** `kit-core`'s `TypeOf` now prefers the
    explicit `type` link (obj `"type"` field; nominal `ty` link) —
    the two *declared-membership* arms of `handl_value::type_link` —
    and keeps the kind-string read for unlinked values (literals,
    plain objects, `__tag__` data). The `__tag__` bridge stays in
    `type_link`/`instance_of` where it already lives — `type.of` does
    not duplicate it. Verified: `type_model_parity` 23/23,
    `value_model_parity` 24/24, `meta_model_test` 30/30,
    `data_constructor_eval` green (no consumer read `type`-keyed
    records as `"object"`).
  - **Stale test fixed (same class, found en route):**
    `kit-core::type_const_ops_mint_builtin_type_values` still expected
    the superseded `{"__tag__": t, "__registry__": "type"}` mint —
    updated to the `__sym__` nominal shape (name + `type.Type`
    membership link).
  - **`dependent.hnl` `Sigma`/`Path` — kept, documented (flagged, not
    deleted):** the `{"__tag__": "type.sigma"}` literal is the
    *term-level* mint — `synth_object_record` constructs the real
    `type.sigma{param, body}` data from the reserved key. The nominal
    `{"__former__": s, "indices"}` shape is not `.hnl`-expressible
    (`sym()` is not in the checker's term env), so the reserved-key
    literal stays as the bridge until `__tag__` deletion; doc comment
    now says so. `dependent_structure` **21/21 green**.
  - **Remaining edges:** (a) the `.hnl` desugar overlay emits the
    legacy `{tag: name}` ctor encoding (`fold_chain` dispatches on
    `v.tag`) while the Rust `desugar.data` emits `__ctor__`/`type`-
    link data — self-consistent within each path but worth syncing
    the port to ADR-0152 before the `__tag__` bridge deletes;
    (b) `type.of` on any object with a plain `"type"` field now
    returns that field — consistent with `type_link` but a wider
    semantic than a kind tag; flag if a non-type consumer relied on
    `"object"`.
- **2026-10-23 — WS-C follow-up: `.hnl` desugar overlay synced to
  ADR-0152** (`lib-desugar`, `engine-transform-rewrite`, `host`):
  remaining edge (a) from the `type.of` entry above is closed.
  - `lib-desugar/src/index.hnl`: `data_rewrite` now emits the nominal
    encoding — `F` binds a seeded-sym former (`{"__former__": fSym,
    "indices": i⃗}` object; `(i⃗) ~>` arrow when indexed), each ctor is
    a nominal arrow producing `{"__ctor__": cSym, fields…, "type":
    F(j⃗)}`, `F.rec` keeps the bound-arrow fold machine. New helpers
    mirror the Rust clause-for-clause: `former_seed`/`ctor_seed`
    (`"data.F[.C]"`), `sym_lit` (`lit` carrying the `{"__sym__"}` mint
    via `sym("seed")`), `family_app`, `param_extractors` (recursive
    fields' `type.indices[j]` links), `mentions_unres`/`ie2ast`
    (index_expr_to_ast; unresolved params → `lit null`). `fold_chain`
    dispatches on `v.__ctor__` sym identity **or-ed with `v.tag ==
    name`** — the legacy-string bridge arm for `{tag}` values in
    serialized artifacts, kept until `__tag__`/`{tag}` deletion. The
    Rust `build_fold_chain` gained the identical `binary.or` bridge
    arm so the overlay stays structurally identical (the `if`-count
    tests don't see `binary.or`). Predicate fold mode is
    retained-but-dead, mirroring `FoldMode::Predicate`.
  - `host/plugin-lib-handl`: `desugar_module()` gains a `SymModule`
    dependency so `sym("seed")` resolves inside the overlay (flat
    `call_host` lookup); the parity test's module gains it too.
  - Verified: `handl_desugar` parity **6/6 green** (was 5 failing);
    `hnl-engine-transform-rewrite` 41+21+35 green; `indexed_family_
    match` green end-to-end through the `.hnl` overlay.
  - **Stale tests swept (the "follow-up sweep is owed" item):**
    `indexed_family_data`/`indexed_family_fin` still asserted the
    predicate encoding (`Vec(0,a)(v0)` call) — updated to
    `sym.instanceof(v, Vec(i⃗))` former-head membership plus a
    `v.type.indices[0]` declared-index check; renamed
    `*_predicate_*` → `*_membership_*`.
  - **Bridge left in place:** (a) fold-dispatch `v.tag` arm (both
    impls) for `{tag}` artifact values; (b) `FoldMode::Predicate`
    codegen kept dead on both sides; (c) `dependent.hnl` `__tag__`
    literal formers still stand (checker env has no `sym()`);
    (d) `match_recursor` stays Rust-served (`DESUGAR_FUNCS=["data"]`)
    — its `.hnl` port exists but isn't the production path.
  - Not run: `lib-desugar` hnl-gate (no `hnl` binary on this box —
    build `host-cli` to gate the `.hnl` typecheck/lint pass).

- [2026-03-18] **Surface open-items: `J` eliminator, `type.Name` aliases, `IO(sig)`** —
  - **`J` eliminator surface (`J p, motive, base`)**: `lex.keyword.j`
    emits `token.keyword.j` for the CAPITAL `J` spelling (lowercase `j`
    stays an identifier — ~250 live uses in lib-desugar/lib-noise/
    lib-str/…); `grammar.j.prefix` + `types.yaml` `type.j -> synth_j`
    were already landed. `apply_motive` gained a var arm so a
    results-bound `Lam`/`LamVal` motive (`_motive := (i,w) ~> T`)
    applies like a literal arrow. `dependent_structure`'s
    `j_eliminates_branch_path_through_motive` now parses the real
    surface form; `j_surface_literal_motive` covers literal motives.
    22/22 green at commit; kit-prelude-lang `7824a7c`,
    engine-typecheck-nbe `98a1a53`, lib-type `079629d` (doc).
  - **`type.Name := body` / `type.Name(params) := body` aliases** —
    the chosen surface reuses `assign` (no grammar changes): the
    `type.` namespace head on an `assign` lhs (field-access or call on
    `var "type"`) is the transparent alias form per ADR-0154 §3.
    `desugar_generic_assign` rewrites it to `Name := body` /
    `Name := (params) ~> body`; `synth_assign` binds a
    `type.alias{params, body}` expansion record;
    `dep_synth_var_type`/`dep_synth_call_type`/`type.Name` field-access
    expand it by plain substitution (params bound to arg types or
    fresh metas, body re-synthesized) — structural, no former mint.
    Also landed: `dep_synth_type` `paren` multi-child reads as
    `type.array{elements}` (the tuple convention `synth_function`
    already mints), `dep_check_intro` positional `type.array{elements}`
    check, `type.array` added to the dependent `check_node` list.
    `type_alias_expands_transparently` covers `Score`/`Id`/`Pair`
    accept + reject. engine-transform-rewrite `4b253a9`,
    engine-typecheck-nbe `1d23fb7`.
  - **`IO(sig)` indexed typed effects (ADR-0153 §4)**: `FormerSig::Io`
    registered at `type_sym("type.io")` (`IO(sig) : Type(0)`, arity 0
    = `IO(⊤)` compat). `Checker::subtype` record-width arm —
    `IO(sa) <: IO(sb)` iff `sb` names ⊇ the ops of `sa`.
    `dep_synth_call_type` `Some("IO")` + `dep_builtin_or_tag` `"IO"`
    emit canonical `Former{type.io}`; `io_sig` reads sig records from
    `entries` payloads, `entry` children, or object-literal-block
    `type-annot` children; `dep_unify` routes IO-headed pairs through
    `vcheck.subtype`. `dep_synth_type` `function` arm now only
    descends into `paren` params for the domain — call-headed domains
    (`IO(s) ~> T`, `Vec(n,a) ~> T`) previously read their last arg.
    lib-type `ec07203` exports `IO := (sig) ~> {"__former__":
    sym.seeded("type.io"), "indices": [sig]}`.
    engine-typecheck-nbe `48d9690`.
  - **Open edge (IO)**: `io_signature_subtyping`'s negative case —
    `IO({puts,gets}) ~> IO({puts}) = x ~> x` — still accepts. The
    positive cases (`IO({puts}) <: IO({puts,gets})`, `IO({puts}) <: IO`
    bare) pass, and the domain `FormerSig::Io`/`sig_width_subtype` arm
    is landed; the widen-rejection needs the `var`-check path (the
    `rule.check`/`check_subtype` scheme that runs before
    `dep_unify`'s IO arm) routed through `vcheck.subtype` — one
    follow-up arm, not a new subsystem.
  - Test: `cargo test -p hnl-engine-typecheck-nbe --test
    dependent_structure` — 23/24 green (the IO negative case the one
    expected failure); `cargo test -p hnl-engine-transform-rewrite`
    green (41/21/35).
