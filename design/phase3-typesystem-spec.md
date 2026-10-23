# Phase-3 implementation spec — unified type system

Normative design: ADR-0106 (kernel, as amended) + 0152 (`data`) +
0153 (`~[n]~>` dimension index) + 0154 (identity/equality/universes/
literals/erasure) — same as phases 1–2. This spec orders what remains:
the genuinely-hard items plus the polish that makes the dependent
surface a real language. Waves of ≤4 parallel workstreams. New
semantics that settle here get their own ADRs; several items are
parked behind an explicit **needs user decision** section below.

## Phase-2 end state (what exists)

Implicit `{…}` index binders (`f{a,b}`, `Vec{n,a}` index-app, erased
at exec), motive-inferred `match`, `forall` as explicit-`Pi` alias,
`IO(sig)` typed effects + handler typing (`synth_handle`, residual
`IO(S\H)` computation, closed-sig row-metas), `Hom(n)`/`>>>[k]`/`id_k`
checked n≤3 (interchange boundary-law verified; n≥4 stored), real
`>>>` composition on cell operands, `Path`-as-data, `eval.checked` +
`reify_ast`/`quote_ast` metaprogramming, seeded nominals + `type`/
`parent` links, `Type(i)` universes, ring-normal-form index solving
behind a `trait IndexSolver` seam (`Proven|Disproven|Unknown`),
exec-erasure + artifact lint, `__tag__`/`{tag}` bridge deleted
(runtime side), `stacker` growable-stack seams, conv zero-copy fast
paths. Verified at phase-2 close: lib 293 + dependent_structure 30/30
+ parity 46/46 + `vec_map{n,a,b}` typechecks and runs e2e.

## Workstream map

| WS | Scope | Repo(s) | Blocks |
|---|---|---|---|
| A | Instance resolution — implicit evidence args + dictionary search by nominal head | `engine-typecheck-nbe`, `kit-prelude-lang`, `lib-type`/stdlib | — |
| B | Higher coherence — `Hom(4)`, comp-tree normalizer for `conv`, HIT evaluation note | `engine-typecheck-nbe`, `kernel` (`handl-value`), `engine`, `lib-type` | — |
| C | Dependent pattern unification — split-tree coverage for indexed `match` | `engine-typecheck-nbe` | — (informs D) |
| D | `Succ`-arithmetic bridge — ctor-aware index equality | `engine-typecheck-nbe`, `kit-prelude-lang`, `kernel`, `engine-transform-rewrite`, `lib-desugar` | — |
| E | `hnl` e2e + compiled-backend parity (`__ctor__`/`__former__`/IO) + artifact sizes | `host-cli`, `host`, `engine-exec-*` | — |
| F | `.hnl` surface fixes + stdlib nominal/former sweep | `kit-prelude-lang`, `engine-typecheck-nbe`, lib-*, `engine` | — |
| G | Diagnostics (spans, expected-vs-actual via `quote`) + user docs | `engine-typecheck-nbe`, `plugin-ext-diag`, `host-lsp`, `host/docs` | — (last) |

Waves: **1: A ‖ B ‖ E** → **2: C ‖ D ‖ F** → **3: G** (G's docs half
drafts from wave 2; ≤4 concurrent throughout). C and D share the
index-equality core — same wave, different seams (C owns
`synth_match_arms`/coverage, D owns `eval_index`/`IndexSolver`);
sequence their dependent.rs landings to avoid the mid-session
overwrite collisions phase 2 hit.

## A — Instance resolution (implicit evidence args)

The phase-2 `{…}` binders solve *index* args by unification and erase
them at exec (`eval.index_app` projects the callee). The next step is
implicit *evidence* args — ADR-0106 §11 already fixes the semantics:
no interface/trait/impl; an algebra is an ordinary object, a "trait
instance" is a value you hold and pass. `Eq a`/`Semiring a` are
nominal formers; a dictionary is an object whose `type` link is the
constraint former-app. Resolution = search, not unification.

- **Constraint formers**: `Eq : Type ~> Type`-shaped declared formers;
  a dictionary value carries `{"type": Former{C, args}, ops…}`. First
  instances in `lib-type`: `Eq`/`Ord`/`Show` over the builtin nominals.
- **Evidence binders**: `{…}` binder whose domain is a constraint
  former-app is discharged by search, not by `unify_index`. Surface
  marker is a decision (below); the elaboration slot is the existing
  implicit-pi drain (`dependent.rs` ~L4199-4270, `dep_synth_call_type`
  ~L2267, `implicit_binder_head` ~L4547).
- **Search**: candidates = declared instances whose head nominal
  unifies with the constraint head; index args unify through
  `unify_index`/`ctx.vcheck.conv`; recursive goals (`Eq(List a)`
  needing `Eq a`) resolve depth-first under fuel; backtracking =
  `store_mark`/`store_restore` (`checker.rs` ~L1052). No candidate →
  honest stuck diagnostic (never silent `type.unknown`); ambiguous →
  decided by policy (decision 1).
- **Erasure class — the real fork**: index args erase; evidence args
  are *runtime* arguments (the body calls dictionary methods), so the
  resolved dictionary materializes as a real call argument at
  elaboration. `eval.index_app`'s callee-projection scheme needs a
  per-binder classification (Language data — e.g. the binder's
  declared domain marks evidence vs index), not blanket erasure.
- **Import story**: instances ride `Sig_m` exports or a module-level
  registry — decision 2. Importable instances are what makes
  `Ord` resolution work across module boundaries.
- Exit: `sort{a} ⦃Ord a⦄ : List(a) ~> List(a)` — call site writes
  `sort(xs)`, the `Ord(Nat)` dictionary is resolved and passed;
  `Eq(List(Nat))` composes; missing instance → diagnostic naming the
  constraint; ambiguity decided per policy; a dictionary-bearing call
  *runs* with the resolved instance (not erased); dependent_structure/
  parity stay green.

## B — Higher coherence (bounded slice: n=4 + comp-tree normalization)

Honest framing stays: this is research-shaped; the tractable slice is
one more dimension of the *existing* boundary law plus definitional
equality for cell composition — not the full globular structure.

- **`Hom(4)` checking**: `infer_hom` (checker.rs ~L578) +
  `cell_faces_at` (~L642) already recurse on dimension — move the
  stored/checked gate 4→5. `infer_comp_axis`/`infer_comp_axis_tys`
  (~L758/~L771) and `infer_id` (~L924) same; `judgments.rs`
  `cell_comp_axis`/`cell_id` arms (~L883/~L916) axis-4; kernel
  `cell_comp_axis` (`handl-value/src/lib.rs` ~L2505) runtime
  composition at n=4. n≥5 stays stored-unchecked — the honest bound
  moves, doesn't disappear.
- **Comp-tree normalizer for `conv`**: `cell.comp(k,·,·)`/`cell.id`
  trees currently compare structurally — a `comp`-tree normal form
  inside `conv` (the WS-D pattern: normal-first, solver-second):
  - associativity — reassociate nested `comp(k, comp(k,f,g),h)` to a
    canonical side;
  - units — `comp(k, id_k(x), f) → f`, `comp(k, f, id_k(y)) → f`;
  - interchange — `(a ∘₂ b) ∘₀ (c ∘₂ d) ≡ (a ∘₀ c) ∘₂ (b ∘₀ d)` as a
    grid/exchange normal form (the research edge — if no terminating
    normal form lands, interchange stays *stated* and the spec records
    why; boundary-law checking already verifies well-formedness).
  Bounded fuel; non-normalizing shapes → stuck (stored), consistent
  with the decided-vs-stuck contract.
- Effect: `e >>>[2] eInv ~[3]~> id(f)` and `dependent.hnl`'s
  `comp_assoc`/`id_left`/`id_right` move from boundary-law-checked to
  *definitionally* convertible where the normalizer discharges them;
  `interchange` depends on the grid normal form landing.
- **HIT/quotient evaluation — document, don't implement**: can a
  `data` ctor target `Hom(k≥1,·,·)`/`Path(a,b)` (a higher-inductive
  constructor)? The former machinery admits the *declaration*, but
  `F.rec` computes on 0-dim ctor values only — a path ctor needs the
  eliminator to specify its action on paths (an ap/map clause), which
  the fold machine has no rule for. Write the verdict as a short
  `design/` note or parked ADR (probably: needs a new eliminator
  clause + a story for whether quotient equations are definitional or
  propositional) — not a workstream.
- Exit: `Hom(4)` boundary law enforced by `conv`; comp assoc + unit
  laws convertible; interchange verdict recorded; HIT note filed;
  n≥5 stored bound restated in `dependent.hnl` comments.

## C — Dependent pattern unification (split-tree coverage)

Current: `unify_index` (dependent.rs ~L954) covers ctor-injective
`App` heads (~L984-993), ring normal forms, `IndexSolver` linear
solving; `synth_match_dependent`/`synth_match_with_motive` (~L3063+)
instantiate ctor sigs and unify indices *per arm* — decided-impossible
rejects the arm, `absurd` discharges it. The gap: unification is
arm-local; nothing computes the **split tree** — which ctors the
scrutinee's indices leave possible, and which are uncovered.

- Kőszegi-style coverage: unify the scrutinee's index term against
  *each* ctor's result indices → per-ctor {split under substitution |
  impossible | undecided}; the possible set is the coverage
  requirement; missing possible ctor → "non-exhaustive: `VCons` not
  covered" diagnostic; all-impossible → absurd-context match
  (legitimate). Undecided ctors → a named "coverage unknown" warning —
  stuck ≠ error, but never silent.
- Ctor-vs-numeral disjointness: `App{Succ}`-head vs `Const 0` must be
  decided-impossible both directions (verify and pin — likely already
  follows from injector-head mismatch; this is what makes
  `match (v : Vec(Succ n, a)) { VCons(h,t): … }` provably total).
- Ctor-form index patterns (`Vec(VCons…)`-shaped index terms in
  patterns) and nested patterns — extend `unify_index`'s App arm;
  record what stays stuck.
- Scope: `dependent.rs` (`synth_match_*`, ctor-sig instantiation,
  `unify_index`), a `coverage` judgment op if the split is
  op-shaped; `types.yaml` `type.match` rewiring if the check moves.
- Exit: `head{n,a} : Vec{Succ n,a} ~> a` checks as *total* on a
  VCons-only match; uncovered-ctor match errors naming the ctor;
  disjointness decided both directions; undecided coverage warns;
  dependent_structure/parity green.

## D — `Succ`-arithmetic bridge (ctor-aware index equality)

Flagged open in phase-2 WS-D: `Vec(n+1+0) ≡ Vec(Succ n)` is stuck —
`n+1` normalizes, the `App{Succ}` index term doesn't read as `+1`.
The ctor-result instantiation gives `?k+1` one direction; the bridge
is the other.

- **Design options (decision 3)**: (a) builtin rewrite — the index
  normalizer reads `App{Succ, x}` as `x+1`: needs the ctor table as
  `Language` data or it violates engine-genericity (a hardcoded
  `Succ` in Rust is a kind check); (b) `data`-declared index
  projection — the former's declaration carries a per-ctor index map
  (`Succ ↦ n~>n+1`), emitted by `desugar.data` and applied in
  `eval_index` before polynomial normalization — generic, the ctor's
  arithmetic is *declared*, not baked; (c) keep stuck — users write
  `n+1` (the `dependent.hnl` convention). Leaning (b): only declared
  ctors bridge, and any inductive can opt in.
- Scope: `eval_index`/`eval_index_fuel` (dependent.rs ~L370-374),
  `unify_index`, the `IndexSolver` boundary (~L905-949 — the bridge
  lives upstream of `prove`, in normalization), `desugar.data`
  emission in *both* impls (`engine-transform-rewrite` native +
  `lib-desugar` overlay — parity required), kit data for the
  declaration shape.
- Exit: `Vec(Succ n, a) ≡ Vec(n+1, a)` convertible both directions;
  `Zero`/`0` decided per the declared mapping; undeclared ctors stay
  stuck (no silent accept); decided-vs-stuck contract unchanged.

## E — `hnl` end-to-end + compiled-backend parity + artifact sizes

The new surface was verified through crate tests; the `hnl` binary has
no build on this box (`host-cli/target/` absent), and `hnl-gate.sh`
has been carried as un-runnable since phase 1.

- Build `hnl`: `(cd ../host-cli && cargo build -p handl-host-cli)` +
  `rebuild-exts.sh`; then run the phase-2 demos through the real CLI —
  `hnl typecheck`/`hnl run` on dependent.hnl, `vec_map{n,a,b}`/
  `head{n,a}` sketches, io.hnl `handle`, `forall`/`f{a}` sugar,
  `eval.checked`/`reify_ast` metaprograms — and `hnl-gate.sh` on
  migrated `.hnl` libs.
- Compiled-backend parity: `scope_value`/`global_value` exist in all
  three engines (tree-walk ~L4113, bytecode vm.rs ~L538, cranelift
  lib.rs ~L630), but `interpret_io_former` is tree-walk-only
  (tree-walk ~L2243 — bytecode/cranelift eliminate handlers statically
  and never interpret `IO` former objects). Verify `match` ctor-object
  dispatch + seeded-`Sym` handle round-trip under bytecode/cranelift;
  port `interpret_io_former` or document the parity bound.
- Artifact sizes: measure the erased artifact on stdlib-size modules
  (arrow_model.hnl precedent); the per-call O(module)
  `prepare_exec_artifact` walk on the `eval_shared_*` bridge is a
  carried perf edge.
- Exit: `hnl` exists; the demos run identically under all three exec
  engines (or the divergence is named and bounded); `hnl-gate.sh`
  green on WS-F's migrated libs; artifact-size baseline recorded.

## F — `.hnl` surface fixes + stdlib nominal/former sweep

- **Arrow-body object-literal parens**: `x ~> {…}` parses `{…}` as a
  *block* (type-annot children); objects need `x ~> ({…})` (phase-2
  WS-G note). Options (decision 5): grammar-level disambiguation
  (the entry-shaped lookahead `synth_block` already applies at the
  type level) vs keep parens + document — the block reading is
  load-bearing for multi-statement arrows.
- **`eval_body_node` lit-payload Null bug** (`judgments.rs` ~L182-190):
  a `lit` whose payload *is* an object (`{"__sym__"}`/`{"__former__"}`
  /`{"__type__"}` — exactly what `reify_ast` emits, quote_ast.rs ~L91)
  hits the `is_object → Value::Null` arm instead of evaluating the
  payload. Fix: the object payload *is* the literal (boundary-faithful
  payloads are the instantiate convention); keep the `"value"`-key
  read for `{"value": …}` envelopes.
- **Carried gaps**: `subst_term_` for dependent ctor domains (`P(x)` —
  phase-1 log, unimplemented); dead `bind_pattern_vars`/
  `FoldMode::Predicate`/`instantiate_ctor` (remove or document);
  `term_interpreter`'s dead `"tag.construct"` arm; serialized
  sub-terms inside `body_def`/quoted payloads not artifact-linted;
  `type Name body` contextual parse (carried — see open items).
- **Stdlib sweep**: `lib-option`/`lib-result` still mint `__tag__`
  via `hash_map.set` (option index.hnl ~L18/25, result ~L19/26);
  `lib-test` and other `tag.`-consuming `.hnl` files — migrate
  producers to `data`/`__ctor__` values (phase-2 WS-B landed the
  `sym`/`sym.seeded` term-env reachability this needs); remaining
  `v.tag`/`"tag"` *dispatch* consumers → ctor-sym identity. Boundary
  unchanged: `__tag__` on host-internal model records
  (Cell/Arrow/Handler/law) is a value-shape convention, out of scope.
- Exit: `x ~> {k:v}` resolves per decision 5 (or parens documented);
  a `reify_ast`-produced former payload evaluates through
  `apply_arrow`; lib-option/lib-result produce `__ctor__` values with
  `match` dispatching on sym identity; `subst_term_` either lands or
  is scoped out explicitly; dependent_structure/parity/handl_desugar
  green.

## G — Diagnostics + user docs

The checker reports kinds and paths but no positions: every
`TypecheckDiagnostic` emits `span: None` (lib.rs ~L308) even though
`AstNode.span: Option<Span>` rides the AST (kernel ast.rs ~L193), and
unify failures render internal encodings, not source-level types.

- **Spans**: thread `node.span` into synth/check/unify diagnostics;
  `plugin-ext-diag` already owns `Span`/`render_diagnostic` — the lint
  pipeline renders, the typecheck one doesn't yet.
- **Expected-vs-actual**: render `Val` through `quote`/`domain_quote`
  (dependent.rs ~L153) with zonking first — "expected `Vec(n+1, a)`,
  got `Vec(Succ n, a)`" at the failing node's span; meta solutions
  shown, not `?m` handles.
- **User docs**: `host/docs/site/` is Diátaxis-shaped and has nothing
  on the dependent surface — an `explanation/` page (data families,
  `F.rec`, match refinement, `Path`/`Hom`, `IO(sig)`, implicit
  binders) + `how-to/` pages (write a `data` family; handle an `IO`
  effect; migrate a tag-string consumer). ADRs stay the spec; docs are
  the user surface.
- Exit: a failing unify prints expected-vs-actual with a source span
  through `hnl typecheck` and the LSP path; docs pages landed and
  indexed in `site.yaml`.

## Cross-cutting invariants (unchanged)

- One value space: checker and exec share `Val`; arrow *values* stay
  self-contained serializable data (`body_def`+`binds`) —
  defunctionalized data, closures only as domain machinery (0106).
- Types-as-values; every type-level entity is an index on a nominal
  former; **nominal heads + structural bodies** (0154 §3). Instance
  dictionaries are ordinary objects with `type` links (0106 §11) —
  search is an elaboration rule, not a new kind of value.
- **Dimension ≠ mode ≠ effect axes** (0153): the `~[n]~>` bracket is
  cell dimension; evidence args enter the *implicit-binder* slot, not
  the bracket.
- Indices are type-level, erased at exec; **evidence args are not** —
  the implicit-binder slot has two erasure classes now, classified by
  declared domain, not ad hoc. The artifact lint stays the verifier
  (0154 §6).
- Honest bounds stay honest: n≥5 stored-unchecked, undecided coverage
  is a named warning, non-normalizing comp-trees store — never a
  silent accept.
- Engine-genericity: calculus vocabulary may be fixed in Rust;
  language constructs are `Language` data — the `Succ` bridge must be
  *declared* metadata, and new surface forms get `AstShapeDef` roles.

## Needs user decision

| # | Decision | Options | Lands in |
|---|---|---|---|
| 1 | Instance-search coherence/overlap policy | (a) no overlap — ambiguous candidates reject at search (Coq/Agda discipline); (b) ordered first-match with backtracking (Prolog discipline); (c) most-specific wins | WS-A — the `store_mark`/`store_restore` machinery supports any; pick before WS-A exit criteria bind |
| 2 | Instance declaration mechanism | (a) `instance` decl → per-module table, exported through `Sig_m` (importable); (b) scope convention — any in-scope binding typed `Former{C,·}` is a candidate (zero new surface); (c) `lang` data registry | WS-A — (a) is the only option that makes instances importable |
| 3 | `Succ`-arithmetic mechanism | (a) builtin rewrite w/ ctor table as `Language` data; (b) `data`-declared per-ctor index projection (recommended); (c) keep stuck, users write `n+1` | WS-D — (b) preserves engine-genericity and lets any inductive opt in |
| 4 | Coverage strictness | (a) full split-tree exhaustiveness — uncovered possible ctor is an error; (b) bounded split + "coverage unknown" warning class | WS-C — (b) matches the decided-vs-stuck contract |
| 5 | Arrow-body `{…}` disambiguation | (a) grammar lookahead — entry-shaped `{` after `~>` is an object (same shape test `synth_block` applies); (b) keep the parens requirement, document it | WS-F — (a) reparses existing code |
| 6 | `Hom(4)` scope | (a) extend the checked gate to n=4 (mechanical `cell_faces_at` recursion); (b) leave n≥4 stored, spend the budget on the comp-tree normalizer | WS-B — (b) is the honest cut: the normalizer is the semantic win, n=4 checking is more-of-the-same |

## Open items (tracked, not blocking)

- Open effect-row tails `{puts | r}` — phase-2 WS-F stretch; row-metas
  bind whole rows only, no surface syntax for row extension.
- `type Name body` contextual parse — carried from phases 1–2;
  `type.Name :=` covers the need. Decide keep/drop (folds into
  decision 5's grammar pass if kept).
- HIT/quotient follow-up — whatever WS-B's note concludes; if the
  eliminator clause is feasible it's a phase-4 ADR.
- `__tag__` payload discriminator on host-internal model records
  (Cell/Cell2/Arrow/op/Handler/law) — carried, separate cleanup.
- `sym` seeds for unexported formers; FFI boundary convention —
  carried ×2.
- ADR-0155/0156 (scope-bound arrow environments / `ModuleEnv`) —
  separate host track, out of scope here.
- SMT backend behind `IndexSolver` — the seam exists
  (dependent.rs ~L905); no driver yet.
- `match_recursor` desugar still requires annotated scrutinee + full
  coverage while the checker accepts less — `try_tag_pattern` covers
  the runtime gap; the lowering divergence is carried.
- Metacircular `.hnl` eval stack depth — the desugar overlay needs
  ~16–32 MiB (tests spawn 64 MiB threads); a real fix (re-threaded
  spine / explicit-continuation eval) is owed, not another bump.

## Progress log

- **WS-D landed** (`engine-typecheck-nbe`): `data`-declared ctor index
  projections — decision 3 resolved to **(b)**. `synth_data` recognizes
  the Nat-former shape: a unary ctor `S(arg : F)` building `F` registers
  `ctor_proj[S] ↦ λx.x+1`, and a family with a successor ctor plus
  exactly one bare ctor registers `Z ↦ 0`. `eval_index` applies the
  projection *before* ring normalization (`App{Succ,x} → x+1`,
  `var{Zero} → 0`), so `Vec(Succ n) ≡ Vec(n+1)` and
  `Vec(Succ(Succ n)) ≡ Vec(n+2)` are definitional both directions;
  `Succ ?k ≡ 2` solves `?k := 1` through the existing linear fragment.
  Two elaboration fixes ride along: ctor-`App` index args read through
  `node_to_index_term` (`Succ n`'s arg is the rigid atom `var{n}`, not
  the sort `Nat`), and a bare `var` naming a `type.ctor_sig` in index
  position is the atom `var{C}` (so `Zero` projects to `0`) instead of
  instantiating the ctor's signature type. Unregistered `App` heads stay
  honest atoms — `f n ≢ n+1` is `Unknown`, arity mismatches don't
  project; decided-vs-stuck contract unchanged (`Vec(Succ n)` vs
  `Vec(n+2)` still rejects; `?k+1 ≡ n+1` pointwise-Bin unification stays
  `Unknown` — the WS-C split tree is where arm-level coverage lands).
  The metacircular overlay (`lib-desugar`'s model) doesn't carry the
  table — noted under open items if overlay parity on `Succ`-indexed
  families is ever exercised. Tests: `dependent_structure` 33 (3 new),
  lib `index_solver_tests` +2, `dependent.hnl`/`indexed_family_*`/parity
  green. Generality: nothing `Succ`-shaped is in the solver — the table
  is `ctor_sym → (arity, template)` declared metadata any `data` with
  the shape populates.

- **WS-F landed** (decision 5 resolved to option (a) — grammar lookahead):
  - `eval_body_node` lit-payload bug fixed in
    `engine-typecheck-nbe/src/judgments.rs`: a `lit`/`value` node whose
    payload is a boundary-encoded object (`__sym__`/`__former__`/
    `__type__`/`__neutral__`/`__meta__`) now decodes through
    `TaggedValue::from` + `lift` instead of `Null` — desugar-minted
    `lit{__sym__}` (push_sym_lit in `data` blocks) and `reify_ast`
    products evaluate through `apply_arrow`. The `{"value": …}`
    envelope read is kept; ambient `Lvl` is threaded through
    `eval_arrow_body`/`eval_body_node`/`apply_body_value` for
    `__neutral__` rigid restore. Repro tests in judgments.rs.
  - `x ~> {k: v}` parses as a *record* now: `brace_is_record` in
    `engine-parse-pratt` scans the brace at token level — non-empty and
    every top-level element entry-shaped (`key :`, or the shape's rest
    token) — and the infix `block_children` arm falls through to the
    `{` prefix row. Multi-statement blocks whose first statement is a
    type-annot keep the block reading (a non-entry element fails the
    shape test); empty braces stay blocks. Reparse tradeoff (decision
    5): a brace that is all-entries is now a record — `{x: Int}`
    single-statement blocks change meaning (accepted).
  - `lib-option`/`lib-result` mint `__ctor__`+`type` objects (seeds
    `data.Option.{Some,None}`/`data.Result.{ok,err}` — display names
    keep the old tag spellings, so `match` patterns are unchanged).
    Manual mint kept over `data` decls: `data`'s `__splice__` assigns
    bind module scope but produce no `export` record entry (the export
    transform is text-level) — `data Option` is the follow-up once
    `export` understands `data`. `option` exports `Some`/`None` under
    ctor names (`None` = the nullary ctor object `match` compares
    against). Consumers migrated to ctor-sym dispatch:
    `option.test.hnl`/`result.test.hnl` assert via `match`;
    `lib-ws-frame-handl` projects the ctor with an `opt_tag` `match`
    helper; `engine-test` `is_err_result` reads `__ctor__` name `err`
    alongside the host `__tag__` assert channel; stdlib tests assert
    ctor sym names.
  - Verified: engine-typecheck-nbe lib tests 297/297,
    dependent_structure 33/33, engine-parse-pratt 62/62 +
    parse_conformance, engine-test `cargo check` clean. **Blocked**:
    parity 46/46 and the stdlib lib suite could not build —
    `hnl-engine-exec-bytecode` lacks `handler_push`/`handler_pop`/
    `handler_lookup` impls for a `NativeOpCtx` trait extension landed
    by the in-flight handler workstream (sibling-repo drift, unrelated
    to this change).

- **WS-E landed** (`host-cli` + `engine-exec` + `engine-exec-bytecode` +
  `engine-exec-cranelift` + `engine-exec-tree-walk`): `hnl` rebuilt and
  the exec-backend parity question is resolved — **bytecode and
  cranelift now run `handle`/IO dispatch**, they do not merely drop it.
  - *`hnl` build*: `cargo build -p handl-host-cli` →
    `~/.cargo/shared-target/debug/hnl` (shared `CARGO_TARGET_DIR`);
    `libhnl_ext_{typecheck,eval,lint,compile,sdkgen}.so` rebuilt from
    the ext repos. `hnl ext list` shows all five; `hnl typecheck` +
    `hnl run` green on `/tmp/hnl-verify/{nat,vec,sugar,io,path_j,
    alias}.hnl`.
  - *The gap found was bigger than IO*: `eval.handle` is a
    `!Handle` scheme whose `body`/`impl_` are `!Child{role}`
    placeholders — `Term::Child` was `NotImplemented` on both compiled
    backends, so **any** `handle` node escaping the
    `interpret_with_handlers` fast path crashed (`handle str.upper("x")
    with h`, any host-call program — not just IO). Fix: `Term::Child`
    now compiles on bytecode (`EvalChild` via
    `lang.child_role_index_for` — also fixes the other `!Child`
    schemes); cranelift's compile arm has no `lang`/`node` context so
    `Term::Child` stays `NotImplemented` there (honest bound — only
    reachable via *named*-target `Handle` elimination, which surface
    `.hnl` never produces: `eval.handle` fixes `target: ""`).
  - *The port*: unnamed-target `Handle` (`""`, i.e. all surface
    `handle … with …`) no longer statically eliminates — it compiles to
    the new shared **`handle_dyn`** op in `engine-exec`'s `native_ops`
    (same pattern as `match_shell`), behind a new
    `OpService::HandlerStack` + `NativeOpCtx::{handler_push,
    handler_pop, handler_lookup}` (3 impls: VmCtx/ClCtx/TwCtx). The op
    runs the tree-walk `Handle` semantics: eval impl child → push
    runtime handler stack → eval body child → `interpret_io_former`
    (shared port — sig fields dispatch through `handler_lookup`,
    Arrow impls via `ctx.apply_arrow`, serialized-Term impls via the
    `{__term__}` bound-arrow convention) → `attach_carried_handler`
    (moved to shared) → pop on every exit. `handler_impl` deduplicated
    into `native_ops`. `arr_dispatch` on both compiled backends now
    consults the stack innermost-first before operand-carried impls;
    `apply_arrow_body` (bytecode) reinstalls carried `Arrow.handlers`
    (cranelift gets it free via `apply_value_treewalk`). Named-target
    `Handle` keeps `eliminate_handlers` (unchanged).
  - *Verdict*: `handle IO({puts:"hello"}) with {puts: s ~> 42}` →
    `puts: 42` identically on all three engines; uncovered `gets` stays
    residual data; `match` ctor-object dispatch + seeded-`Sym` round
    trip (handle `16969724874574109231`) verified identical;
    `handle 1 with {}`/`handle str.upper("hi") with {…}` run on
    bytecode (previously crashed). **Residual gap, pre-existing**:
    `.hnl` module calls on cranelift panic (`HandlFunction::call`
    builds a runtime lacking the `cranelift` eval-engine name —
    `plugin-lib-handl` ~L2231; `str.upper("hi")` alone crashes on
    cranelift, unrelated to `handle`).
  - *Artifact sanity*: `hnl compile sexp` on `data`-using files shows
    `__former__`/`__ctor__` only as live runtime `record` values
    (sym handles are concrete data — the permitted shape); exec-erasure
    + artifact lint (`prepare_exec_artifact`) runs on the eval path.
  - *Tests*: engine-exec 9/9, bytecode 91, cranelift 82 + conformance
    2/2 (the `child_role` bytecode cell re-pinned PASS — intentional
    fix), tree-walk ~137 — all `--test-threads=1`. Commits landed per
    repo; the WS-F log's noted bytecode-parity blocker
    (`handler_push/pop/lookup` trait extension) is now unblocked.
