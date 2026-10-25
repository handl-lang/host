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

## B — Higher coherence (generic `Hom(n)` + comp-tree normalization)

Honest framing stays: this is research-shaped; the tractable slice is
the *existing* boundary law made dimension-generic plus definitional
equality for cell composition — not the full globular structure.
(User direction superseded decision 6's n=4-then-stop scoping: the
checking went *generic*, not "4 then stop".)

- **`Hom(n)` checking — generic**: `infer_hom` (checker.rs) checks
  every literal `n ≥ 1`: `cell_faces_at`'s boundary walk was already
  level-generic, so the `n ≤ 3` gate was the only artificial cap —
  `Hom(n, c₁, c₂)` checks `infer(c₁)`/`infer(c₂)`'s `(n-2)`-faces by
  `conv` at any `n`. `>>>[k]` (`infer_comp_axis`/
  `infer_comp_axis_tys`) and `id_k` (`infer_id`) got the same
  treatment — the vertical/horizontal laws are level-uniform —
  `k > dim` keeps its `Dim` error at any level; `k ≤ 0`, open
  dimension indices, and open endpoint types stay stored (the
  open-dimension bound, unchanged).
- **Comp-tree normalizer for `conv`** (landed in kernel `nbe.rs`):
  `cell.comp(k,·,·)`/`cell.id` applications normalize to a canonical
  `CellNf` form — flatten same-axis chains into ordered `Axis(k,·)`
  nodes (associativity), drop `∘ₖ` units (`id_j(x)` absorbs at
  `j ≤ k+1` or recursively through its payload; objects are
  degenerate `id` towers and absorb at every axis), and transpose
  uniform `j`-grids at `j < k` (interchange — smaller axes sort
  outward, a terminating canonicalization: each transpose strictly
  decreases the preorder axis sequence). Bounded fuel; non-uniform
  grids, open axes, and non-unit leaves stay stuck — structural
  compare, never wrong-equal.
- Effect: `e >>>[2] eInv ~[3]~> id(f)` (and its dim-4+ counterparts)
  checks under the generic boundary law; `dependent.hnl`'s
  `comp_assoc`/`id_left`/`id_right`/`interchange` are *definitionally*
  convertible where the normalizer discharges them; the tagged-side
  `Hom(m ≥ 3)` unification escape stays as the proof-relevance
  stuck-contract.
- **HIT/quotient evaluation — document, don't implement**: can a
  `data` ctor target `Hom(k≥1,·,·)`/`Path(a,b)` (a higher-inductive
  constructor)? The former machinery admits the *declaration*, but
  `F.rec` computes on 0-dim ctor values only — a path ctor needs the
  eliminator to specify its action on paths (an ap/map clause), which
  the fold machine has no rule for. Write the verdict as a short
  `design/` note or parked ADR (probably: needs a new eliminator
  clause + a story for whether quotient equations are definitional or
  propositional) — not a workstream.
- Exit: `Hom(n)` boundary law enforced by `conv` at every literal `n`;
  comp assoc + unit laws convertible; interchange convertible on
  uniform grids (verdict recorded); HIT note filed; the stored
  fragments restated in `dependent.hnl` comments (open dims, open
  endpoint types, non-literal axes — no dimension cap).

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
| 6 | `Hom(4)` scope | (a) extend the checked gate to n=4 (mechanical `cell_faces_at` recursion); (b) leave n≥4 stored, spend the budget on the comp-tree normalizer | WS-B — superseded by user direction: **generic `Hom(n)`** — the recursion was already level-generic, so the cap lifted entirely (all literal `n`, all `>>>[k]` axes, all `id_k` levels) AND the normalizer landed |

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

- **Dogfood: real `.hnl` collections lib** (`lib-collections/
  src/collections.hnl` — full repros in `/tmp/dogfood-report.md`).
  Writing `Maybe`/`Nat`/`Vec`/`List`, implicit-binder `vmap`/`vhead`/
  `filter`/`zipWith`/`append`, `IO(sig)` ops, `instance` evidence
  exposed every surface friction point:
  - *Clean*: `data` families (sorted + unsorted params), implicit
    `{…}` index binders + call-site inference, `Vec{Succ n}` bridge,
    single-arm dependent `match` (provably total on `Vec{Succ n}`),
    `absurd` discharge + unreachable-arm rejection, `IO({puts})` sig
    records + `handle` coverage rules, `instance` search discharging
    `{ev : Eq(a)}` at `eqv(Zero)`, `Forall(K, x ~> B)`, `export` on
    implicit-binder fns and `data`, `Sigma(Nat, m ~> Vec(m,a))` as the
    existential encoding.
  - *Broken* (silent-wrong or unwritable): **runtime `match` on a
    parameterized family** misdispatches without an inline
    `(v : Fam(i⃗))` annotation + all-ctor arms — bare `Nope:` patterns
    act as binders (`match Jx(9) { Nope: Nope }` → `Jx(9)`), and the
    annotation can't be written inside a `{…}`-bound body (binders
    appear as unsolved metas in the domain: `expected [?m1,?m0], got
    [n,a]`; bare index names in paren annotations read as SORTS —
    `Vec(n,a)` ≡ `Vec(Nat,Type_0)`, only `n + 0` keeps `var`). A
    `Vec{n,a}`-polymorphic fn can be checked OR run, not both.
    **Self-recursion typechecks but runs wrong**: `lower_ast` rejects
    self-calls, the AST-eval fallback returns the base arm / input
    unchanged (`vmap`, `vsum`, `len` all produce wrong values, all
    three engines). **`T.rec` is invisible to the checker**
    (`record_field_type` on the family) — `F.rec` folds unwritable in
    checked code. **`append` fails**: `?k+1+m ≡ m+?k+1` doesn't
    canonicalize through metas/rigids (and the error renders both
    sides identically — meta-name collision in zonk display).
    **`zipWith` catch-22**: inner `VNil` is "unreachable arm" to the
    arm checker (n≡?k+1 propagates) but "can occur" to the coverage
    pass — `absurd`/catchall/real-arm all fail differently.
    **Un-annotated ctor results are unsound**: `VCons(k : Nat, x : a,
    xs : Vec(k,a))` (no `: Vec(…)` result) synthesizes `Vec(?m,a)` —
    `VCons(Zero,true,VNil) : Vec(5,Bool)` checks. **No existential
    index syntax** (`?m` fails to lex); `Sigma` works as a return type
    but a computing `filter` can't rebuild it (bare-name→sort again).
    **Dict member projection missing** (`ev.eq` errors on the
    former-app type; `match ev { mkEq(f): … }` is the idiom).
    DX: `J` is a reserved keyword (ctor named `J` gives a bare
    "expected comma" error), no `forall a.` sugar, no `extern`
    effect-op decl form, `IO(sig)` can't be minted in term position,
    `data Nat` values aren't `Number`-arithmetic-able.
  - Phase-4 candidates (ranked): runtime parameterized-match fix or
    loud decline; `F.rec` as a checked elimination form; letrec/`fix`
    surface or honest self-call lowering; implicit binders as rigid
    vars in bodies (fix bare-name→sort for bound names); commuted-sum
    canonicalization through metas; reconcile coverage vs
    arm-reachability judgments; decl-time error for unrecoverable
    ctor result indices; existential index former; `extern` op decls.

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

- **WS-G landed** (`engine-typecheck-nbe` + `kit-prelude` + `host` +
  `ext-typecheck`): judgment errors carry source spans and render
  expected-vs-actual in surface syntax; `hnl typecheck` prints
  `file:line:col` diagnostics through `plugin-ext-diag`.
  - *Spans*: `synth_node`/`check_node` attach `node.span` to propagated
    `LangError`s via `with_node` (fills an empty span only, so the
    innermost node's position wins); the no-type-rule warning records
    `node.span` instead of `None`. `typecheck_with_prelude` keeps the
    `Engine` span on `typecheck errors:` wraps — shifted back by the
    `do {` wrapper prefix so byte offsets stay source-relative — and
    adopts the bootstrap scope-pass diagnostic's span (already
    source-relative). `plugin-ext-typecheck` puts `span` + the pure
    `message` on the capability response; `ext-typecheck` renders via
    `render_diagnostic` (the lint shape), stripping the internal
    `typecheck errors:`/`typecheck:` prefixes; spanless responses keep
    the legacy `{path}: {msg}` line.
  - *Expected-vs-actual*: `unify`/`dep_unify` failures render
    `cannot unify — expected <T>, got <U>` through the new
    `display_type` (dependent.rs) — `type.function` → `A ~> B`,
    `family_app` → `Vec(n + 1, Bool)`, `Pi`/`Sigma`/`Forall`/`Vau`
    print binders, quote spellings by head, index terms infix
    (`n + 1`), record/effects rows spelled, solved tagged metas zonked.
    The `cannot unify` phrase is kept for consumers matching on it.
  - *Docs*: `host/docs/site/tutorials/dependent-types.md` — `data`
    families, indexed `Vec`, `match` refinement, `Path`/`refl`/`#`/`J`,
    `IO(sig)` handlers, implicit `{…}` binders; every snippet verified
    on the rebuilt `hnl`, with a typecheck-vs-run honesty table;
    indexed in `tutorials/index.md`.
  - *Verified output*: `vec.hnl:2:20: error [typecheck]: cannot unify
    — expected Vec(3, Bool), got Vec(1 + 1, Bool)`; scope errors
    `scope.hnl:1:5: error [typecheck]: unbound wire: `y` is not in
    scope`. Tests: engine-typecheck-nbe all suites green
    (lib 297, dependent_structure 33, parity 46, …),
    plugin-ext-typecheck 10/10, hnl-ext-typecheck 4/4.
  - *Bytecode-parity re-check* (the WS-F blocker): `handler_push/pop/
    lookup` impls have landed on `engine-exec` — bytecode suite 91
    (86 unit + 5 conformance) green, `handl_desugar` parity 6/6 green,
    kit-prelude `fp_integration`/`ordered_annotations`/`tagged_values`
    green. Pre-existing, unrelated: kit-prelude `intl_c` (13/13 fail —
    the `from "../../../lib-intl-c/src" import *` module resolves to
    `null` on this box: `field_access` on null; environment/ICU,
    not the parity path).

- **WS-C landed** (`engine-typecheck-nbe`): Kőszegi-style split-tree
  coverage for indexed `match` — the "what do we do about
  exhaustiveness" carry pinned to decision 4b.
  - *Mechanism*: `synth_match_with_motive` now runs a coverage pass
    after the arm loop. At each split-tree leaf every ctor of the
    scrutinee's family is probed as `Possible`/`Impossible`/`Undecided`
    against the leaf-refined scrutinee indices (`ctor_leaf_status`,
    full-ctx snapshot — probes never leak). An undecided-uncovered ctor
    case-splits a splittable index `var` and recurses: `Nat` sorts go
    `v ↦ 0` / `v ↦ m + 1`; a `data`-declared index family goes
    `v ↦ App{K, f⃗}` per ctor (`eval_index` projects `App{Succ,m}` to
    `m + 1`, so both spellings coincide). Fresh split vars get their
    own index sorts for deeper splits. Decided-reachable + uncovered ⇒
    `non-exhaustive` error naming the ctor and the leaf substitution
    (`under n ↦ 0, m ↦ 0`); still-undecided at a final leaf ⇒
    `coverage unknown` warning. Bounds: `SPLIT_MAX_DEPTH = 3`,
    `SPLIT_MAX_NODES = 64` — over-budget degrades to the warning, never
    a silent accept.
  - *Supporting solver work*: the census lives in the ctx
    (`family_ctors`, `ctor_family`, `family_param_nat` from `data`
    decls; `nonneg_index_vars`, `nonneg_metas` for Nat-sorted rigids).
    `solve_index` gained the nonnegativity-gap rule — `c + Σ` with all
    atoms known-nonneg and monotone coefficients decides `m + 1 ≡ 0`
    where the linear fragment stayed stuck — and `Nat`-sorted metas
    reject negative solutions (`?k + 1 ≡ 0` decides `VCons` impossible
    on `Vec(0)`). `unify_index` decides declared-ctor disjointness:
    two different `data` ctor heads, or a ctor `App` vs a numeral, are
    decided-impossible; unknown heads stay stuck. `absurd` contract
    unchanged — decided-impossible leaves still need it.
  - *Scope*: single-scrutinee matches on `data`-declared families.
    Nested/multi-scrutinee splitting is out of scope (the bound is the
    honest cut). The canonical `match_on_succ_indexed_scrutinee_refines`
    test — `VCons`-only on `Vec(Succ n)`, no catchall — is the payoff;
    `Vec(n + m)` decides through `n ↦ 0`/`n ↦ Succ m` splits.
  - *Verified*: `dependent_structure` 43 tests — the 5 new coverage
    tests pass; the 5 failures are the in-flight WS-A `instance_*`
    tests (parse-level, `instance` syntax not yet landed — sibling
    workstream). `parity` 46/46. `cargo test --lib`: 301 pass, 1 fail —
    `checker::tests::hom_dim_index_n_le_2` is the WS-B sibling's
    in-flight test. `hnl typecheck` green on a split-tree file
    (`VCons`-only on `Vec(Succ n)` + `{VNil, VCons}` on `Vec(n + m)`).
    Note: the commit sits atop uncommitted WS-A/WS-B sibling edits in
    this working tree — reconcile at their landings.

- **WS-B landed** (`engine-typecheck-nbe` + `kernel/handl-value` +
  `lib-type`): **generic `Hom(n)` checking + a comp-tree normalizer in
  `conv`** — decision 6 superseded by user direction ("generic, not 4
  then stop"; both halves landed).
  - *Genericity verdict*: `cell_faces_at` was already a level-agnostic
    boundary walk (`j+1 = m` reads the faces, `j+1 < m` recurses
    through the source boundary, `j+1 > m` is the degenerate
    self-face) — the `n ≤ 3` cap was artificial. `infer_hom` now
    checks every literal `n ≥ 2` via the same face law
    (`Hom(4, Int, Int)` checks as iterated refl, `Hom(4, Int, Nat)`
    rejects; `x ~[4]~> y` on 2-cells requires `x ≡ y`);
    `comp_axis_of` dropped the `k ≥ 4 || d ≥ 4` stored gate —
    top-boundary and deep-axis (`tgt_k ≡ src_k`) composition decide at
    any literal axis/dimension, `k > dim` keeps `Dim` at any level
    (`>>>[k≥2]` on objects is now a decided error, not stored);
    `infer_id` dropped the `lvl ≤ 3` gate (`id_7` on a 4-cell →
    `Hom(7, w, w)`; `id_k` of a k-cell is `Dim` at any level).
  - *Normalizer* (`handl-value/src/nbe.rs`): `Former{cell.comp}`/
    `cell.id` applications compare via `CellNf` — assoc flattens
    same-axis chains to ordered `Axis(k,·)` nodes; `id_j(x)` absorbs
    under `∘ₖ` when `j ≤ k+1` or recursively through its payload
    (objects — `Nominal`/`Const`/`Universe`/non-cell formers — are
    degenerate `id` towers and absorb at every axis, the
    `cell_boundaries` reading); uniform `j`-grids (`j < k`, same
    inner axis + arity) transpose — interchange is *definitional*:
    `(a ∘₀ c) ∘₂ (b ∘₀ d) ≡ (a ∘₂ b) ∘₀ (c ∘₂ d)` and the n-ary
    generalization. Canonical order is smaller-axis-outward; each
    transpose strictly decreases the preorder axis sequence
    (terminating), plus a 4096-step fuel bound. Stuck shapes
    (non-uniform grids, open axes, non-unit leaves, `id_?` levels)
    compare structurally — never wrong-equal. `conv` fires the
    normalizer whenever either side is cell-headed, so `comp` trees
    nested under `Hom` args or in spines convert too.
  - *Verifier shapes*: `x >>>[3] xInv ~[4]~> id(p)` checks; deep-axis
    `∘₂` on 4-cells pastes `(d-1)`-faces pointwise; assoc/unit trees
    nested under `Hom(4,·,·)` endpoints convert.
  - *Still stored-vs-checked*: `n ≤ 0`, non-literal dims/axes, open
    `Hom(?m,·,·)` endpoints, `id` with unresolvable payload level —
    and the *tagged* `Hom(m ≥ 3)` unification escape
    (`beyond_checked_dim`, a conv-failure → stuck contract, kept).
    Stretch not landed: suspension/globular unit coherences — the
    unit rule is the flat `Hom(j,x,x)` encoding's `id`-tower
    absorption; a separate suspension former isn't part of the
    former set.
  - *Tests*: checker +5 (generic `Hom(4)`/`Hom(5)` laws, `>>>[4]`/
    `>>>[3]`/`∘₂` at dim 4, `id_4`–`id_7`, object `Dim` at any axis),
    the two stale assertions updated (`Hom(4,Int,Nat)` now rejects;
    interchange composites now conv); nbe +4 (assoc, unit absorb
    incl. `id`-towers and objects, 2×2 + 3×2 interchange grids +
    non-uniform stuck, `Hom`-nested trees). engine-typecheck-nbe lib
    302/302, dependent_structure 43/43, parity 46/46, handl-value
    37/37 — all `--test-threads=1`.
  - *Note for the sibling WS-A/WS-C in flight*: `dependent.rs`'s
    instance-coherence scan needed `ctx.instances.get(&head).cloned()`
    (borrow overlap with `unify_index`/`store_restore`) — one-line fix
    left in the working tree; `src/dependent.rs`/`src/lib.rs` carry
    their uncommitted work, so only `src/checker.rs` lands in this
    commit.

- **Dogfood soundness fixes (session 2)** — three of the ranked
  phase-4 candidates landed:
  - *Runtime parameterized-match misdispatch (F2)*: `match Jx(9)
    { Jx(x): 111, Nope: 222 }` returned `222` because
    `handl-kit-arrow-lower`'s `compile_pattern` discriminated on
    `__tag__` only — canonical ctor objects (`{"__ctor__": sym, …}`,
    parameterized families mint no `__tag__`) fell through to the
    bare-var arm's unconditional bind. `compile_pattern` now emits
    `tag_pred ∨ eq(ctor_name(v), name)` with binds chosen at match
    time (`Choice` re-runs the tag discriminant; ctor path uses the
    callee's declared param order via `EnvEntry.param_names`, else
    sorted `ctor_fields`); bare `var` patterns bound to ctor objects
    check `eq(ctor_name(binding), null)` to decide bind-vs-eq at
    match time. New term ops `ctor_name`/`ctor_fields` in
    `hnl-engine`'s `term_interpreter`. Commits `8e03b40` (engine),
    `29491416` (host).
  - *Arrow `binds` capture gap (nested-arm case)*: a nullary-ctor
    `var` pattern inside an arrow body matched unconditionally —
    `Ast::instantiate` bakes only term-position `var`s, so `subst`-
    routed ctor objects left pattern `var`s unbound and
    `scope_value` missed → catch-all. `native_ops` capture now
    routes `is_ctor_object` values to residual `binds`.
    `vsum(v1) = Succ(Zero)` end-to-end on the AST-eval path.
    Commit `8af864a` (engine-exec), plus `as_num`'s Nat-tower
    coercion (`Succ(Zero) → 1`) so `ie2ast`'s `k+1` index eval and
    `binary.add` on `data Nat` values compute instead of erroring.
  - *Un-annotated ctor result unsoundness (F1)*:
    `VCons(k,x,xs)` (no `: Vec(i⃗)`) synthesized `Vec(?m, ?a)` and
    `?m` unified with any expected index — `VCons(0,true,VNil) :
    Vec(5,Bool)` checked. Now: `type.function` application performs
    the model's `subst_term_` dependent-app (param_name arg index
    into the codomain — declared `Vec(k+1,a)` computes), and a
    `nat` index parameter no argument domain mentions mints
    `lit null` (the desugar's unrecoverable-index convention)
    instead of a solvable meta — `Vec(null,a) ≢ Vec(5,Bool)`.
    Commit `81a184e` (engine-typecheck-nbe).
  - *Still open* (ranked, next pass): `?k+1+m ≡ m+?k+1` atom
    ordering (F5), `F.rec` visibility to the checker (F6-adjacent),
    implicit-binder name scoping into body annotations (F3),
    coverage-vs-reachability disagreement (F6), existential index
    former (F7), dict member projection (F8). `lower_ast` self-call
    decline + AST-eval recursion now works for `vsum`; `zipWith`
    catch-22 untested post-fix.

- **Dogfood gap fixes (session 3)** — the three open P2s from the
  session-2 log landed in `engine-typecheck-nbe`, plus regression
  tests for the P1 runtime fixes:
  - *Commuted-sum canonicalization (F5)*: `append{n,m,a}` typechecks.
    Root cause was narrower than "atom ordering" — `poly_of` bailed on
    a bare `{"__meta__": id}` atom (`tag_of(t)?` → `None`), so any
    `Bin` containing an unsolved meta stayed unnormalized:
    `(m + ?k) + 1` vs `m + (?k + 1)` never canonicalized. Metas are
    now honest ring atoms (keyed `m:{id}` like `index_atom_key`
    already promised). Regression:
    `commuted_sum_index_unifies_through_metas`.
  - *`F.rec` checker visibility*: `synth_data` registers a `<F>.rec`
    Γ binding minted from the ctor telescopes — a nested
    `type.function` `?mm ~> c0.. ~> i0.. ~> F(?u) ~> ?mo` where each
    case is the ctor's instantiated domain chain plus one `?mo`
    hypothesis per recursive arg, and `?mo` is shared across cases
    and the result (the non-dependent fold contract; `null` motive
    stays legal). `record_field_type`'s new `type.family` arm
    resolves `F.rec` through that scope binding, and `check_node`
    accepts type expressions for `type.universe` (`Bool` as a
    second-round index arg). `Vec.rec(null, 0, step)(2, Bool, v)`
    typechecks. Regression: `family_rec_resolves_eliminator_type`.
  - *Implicit binders in body annotations (F3)*: `dep_intro_implicit`
    bound `Γ[binder] := param` (the binder's *sort* — an unsolved meta
    for `{n, a}`), which shadowed `results[binder] = var{b}` for the
    Γ-first `dep_synth_var_type` lookup — `(v : Vec(n, a))` read
    `Vec(?m1, ?m0)`. Non-evidence binders now bind `var{b}` in Γ
    (the `synth_assign` convention); evidence binders
    (`{ev : Eq(a)}`, `evidence: true`) keep their former-app type.
    Regression: `implicit_binder_names_scope_into_body_annotation`.
  - *Runtime regression tests* (`host/crates/handl-runtime/tests/
    engine_parity.rs`): `ctor_object_match_dispatches_on_sym`
    (`match Jx(9) { Jx: 111, Nope: 222 }` → `111`, all three
    engines), `nat_tower_binary_add` (`Succ(Zero) + Succ(Zero)` → 2),
    `self_recursive_arrow_evals_via_ast_fallback` (`vsum` folds
    correctly — **tree-walk only: bytecode/cranelift still diverge
    on self-recursive closures** — recorded, not fixed).
    `unannotated_ctor_result_index_is_decided_mismatch` pins F1.
  - *Still open*: ~~`VCons(x, xs)` *reconstruction* inside an annotated
    `match` arm hits a var-shadowing wrinkle~~ **fixed** (session 5
    below — `__fam_<name>` telescope bindings); the `zipWith`
    catch-22 (F6); coverage-vs-reachability; existential index
    former (F7); dict member projection (F8); `hnl run` on
    `export f : T = …` files fails (`call_host: function 'f' not
    found` — the `export` op is a no-op and the do-wrap path never
    binds the name; pre-existing, not a P1 regression);
    `export f{…} : T = body` in module-prep yields name
    `f{…}` (unbindable).

- **Stdlib nominal migration — the rest of lib-\*** (WS-F continuation;
  per-repo commits on `main`, no push). Survey: `grep '__tag__'` +
  `tag.*`-consumer census over `lib-*/**/*.hnl`. Findings: the only
  remaining `.hnl`-side `__tag__` *mints* were lib-test's
  `hash_map.set(..., "__tag__", "ok"/"err")` workaround; every other hit
  was a *consumer* of ok/err results — and since lib-result moved to
  `__ctor__` while `tag.tag_of`/`tag.untag` read only `__tag__`, those
  consumers were silently broken (tag_of → null, untag → null). The
  consumer fix is `match`-based ctor-sym dispatch: the lowered
  `Name(args)` pattern's `tag_pred ∨ eq(ctor_name(v), name)`
  discriminant covers **both** the canonical `__ctor__` mint and the
  host-minted `{"__tag__": ...}` envelope (`ffi.load`/`ffi.sym`/
  `error.try`), so `match r { err(e): …, ok(v): …, _: … }` is safe
  regardless of producer; `tag.untag(r).value` reads became `r.value`
  (both shapes carry the payload under `.value`). Migration table:

  | Lib | Verdict | Test status |
  |---|---|---|
  | `lib-test` | **migrated**: assert/snapshot/benchmark/fuzz/property mint `{__ctor__: sym.seeded("data.Result.{ok,err}"), value, type:{__former__:"data.Result"}}` (manual mint — these are `HandlModule`s whose binds carry only declared imports; `sym` is a floor native); test/benchmark/fuzz/property consumers → `match` | compiles into engine-test; lib unit suite green except 7 pre-existing `pred.*` failures (unrelated: `char.chars`/`serialize` return `{"children":[…]}` term nodes on this box — fails identically with the migration stashed) |
  | `lib-cli` | **migrated**: cli-c `argv`/`run` → `match` ctor dispatch + exported `is_err`; `argv_head` law → `.value[0]`; cli-c.test.hnl → `res_tag`/`is_err` + `.value` (~40 sites) | `hnl test` 32/50 (baseline pre-migration 0/50 — the `tag.*` reads were dead on `__ctor__` records); residual fails are the same `{"children"}` env bug + stale-`hnl` match lowering |
  | `lib-cli-gen` | **migrated**: 5 test files → `.value`/`res_tag`; generated adapters' `json-capable` doc comment → `parse(r.value.stdout)` | test files need cli-c backend (the same env caveats) |
  | `sdkgen-cli` | **migrated**: `emit_hnl.rs` doc-comment template + `emit_hnl_test.rs` expectation updated so regenerated adapters don't reintroduce `tag.untag` | `cargo test -p` not run (env-time limit); one-line string change |
  | `lib-compress-gzip-c`, `lib-compress-zlib-c` | **migrated**: `unwrap` → `match r { err(_): error.raise(…), _: r.value }` | `hnl test` probe file compiles; natives absent on this box (gate tolerates `call_host` fails) |
  | `lib-concurrent-epoll` | **migrated**: `is_err`/`err_of`/`val_of` → match/`.value` | natives absent; no local suite |
  | `lib-fetch-c`, `lib-fetch-js` | **migrated**: same helpers + 4 fetch-js sites | fetch-c.test.hnl → helpers migrated; natives absent |
  | `lib-gh` | **migrated**: test file → `.value` | needs `gh` binary |
  | `lib-intl-c`, `lib-unicode-c` | **migrated**: `is_ok`/`is_err`/`res_tag` now `match`-based (covers host `__tag__` ffi envelopes *and* `__ctor__`); `payload`/`or_raise`/`statused` + ~130 test sites → `match`/`.value`; conformance `generate.mjs` template updated | kit-prelude `intl_c` suite can't run here (ICU absent — pre-existing, WS-G noted) |
  | `lib-serve-epoll` | **migrated**: `tag.tag_of(d) == "None"` → `match d { Some(v): …, _: read-more }` | natives absent |
  | `lib-ws-c` | **migrated**: `val_of` → `.value`; test → helpers | natives absent |
  | `lib-ws-frame` | **migrated**: `tag.tag_of(decode h) == "None"` → `match … { Some(_): false, _: true }` | contract file (sig-only) |
  | `lib-ws-whatwg-c` | **migrated**: http.test.hnl → `res_tag` | natives absent |
  | `lib-tag`, `lib-loader`, `lib-desugar` | **left alone**: bootstrap/overlay internals; `tag.*` ops are the intentional inspection surface over host `__tag__` model records (ADR-0152 §Migration) | — |
  | `lib-type`, `lib-type-handl` | **left alone**: already nominal; `dependent.hnl`'s `{"__tag__":"type.sigma"}` mint is the documented `.hnl`-expressible bridge `synth_object_record` reads — stays until `__tag__` deletion lands | — |
  | `lib-ffi` | **left alone**: `__tag__` mentions are doc comments describing the host `ffi.load`/`ffi.sym` envelope — accurate | — |
  | `lib-json`, `lib-url-rfc`, `lib-url-whatwg`, `lib-base64`, `lib-hex`, `lib-serialize` | **left alone**: `{"type":"codec"/"parser"/"serializer"}` records are duck-typed module descriptors — dispatch is field-call/`typeof.is`, the string is *data*, not a type tag | — |
  | `lib-pred`, `lib-cli` flag `kind`, `lib-concurrent*` `{name: op}` step vocab | **left alone**: tag-is-data (case-record fields, scheduler op selectors) | — |
  | host `__tag__` mints (`error.try`, `ffi.load`/`ffi.sym`, `TestAssertHandler`, Cell/Arrow/Handler/law model records, engine's `{"__tag__":"type.*"}` term values, `docs/`+fixtures `*_model.hnl`) | **left alone**: the carried FFI-boundary/host-internal convention (open items: "`sym` seeds for unexported formers; FFI boundary convention — carried ×2") — consumers now read them through `match`, which accepts both shapes | — |
  | `lib-fs`, `lib-event`, `lib-serve`, `lib-kv*`, `lib-fetch`, contract `export X := A ~[1]~> B` | **NOT migrated — phase-4 flag**: `IO(sig)` doesn't fit yet. `op "x" : Sig` tails are inert text (verified: `op "demo.op" : String -> GARBAGE(((` typechecks clean — the loader strips decl lines before any type pass). And `IO(sig)` models a *suspended* computation (`interpret_io_former` dispatches the sig record as data), while these ops are eager FFI calls — writing `~> IO({…})` codomains would misrepresent the contract and is unverifiable anyway (all these files are arrow-dialect → `hnl-gate`'s `is_canonical` skips them). This is lib-collections' own KNOWN-GAPS item: "no `extern` effect-op decl form — a `builtin`/`extern` surface for op declarations is a phase-4 item". Contract `~[n]~>` stays — post-ADR-0153 it is the *dimension* index, already correct | — |

  Verified: `match r { ok(v): …, err(e): … }` dispatches `__ctor__` and
  host `__tag__` mints identically; `.value` projects both; bare
  nullary-ctor names (`None:`) are catch-all binders unless scope-resolved
  — consumers use `Some(_)`/`_` splits instead. `dependent_structure`/
  `parity`/`handl-runtime` untouched by these `.hnl`-only edits
  (compile-time `include_str!`s — no Rust diff); `engine-test` lib suite:
  31 pass / 7 pre-existing `pred.*` fails (identical stashed vs migrated).

- **`hnl lsp` first cut landed** (`host-lsp` + `kit-prelude` +
  `kit-data-prelude` + `engine-transform` + `engine-typecheck-nbe`):
  `hnl lsp` now boots the language server on stdio via a new
  `hnl-ext-lsp` cdylib (ADR-0149 packaging — `libhnl_ext_lsp.so`
  discovered next to the `hnl` binary, running the same
  `serve_stdio()` loop as `handl-host-lsp`, which split into lib +
  thin bin to share it).
  - *Diagnostics*: `textDocument/publishDiagnostics` runs the full
    checker pipeline — the bootstrap scope pass
    (`check_module_with_prelude`, all its spanned diagnostics:
    `unbound-wire`, `duplicate-binding`, …) plus the NBE checker's
    spanned errors (`cannot unify — expected Bool, got Number`) mapped
    to LSP ranges. `x : Bool := 3` squiggles 0:0–0:13 severity 1.
  - *Hover*: identifiers get the checker's per-node synthesized types
    — new `Kit::inspect_with_prelude` returns `node_types` keyed by
    spans in caller-source coordinates (the `do {}` wrapper stripped),
    rendered via `display_type_unzonked` (new `pub` ctx-free export).
    Hover on `x` in `y := x` shows `x : Bool`.
  - *Goto-definition*: new generic `kind: binding` LSP provider —
    a scope index walked off `AstShapeDef` roles (target+value →
    module bindings; pattern/params+body → scoped binders), innermost
    scope wins; `lsp.def.var` runs first in `textDocument/definition`
    so a `var` reference resolves to its binder before the export
    rules' enclosing-declaration fallback. Def on `x` in `y := x`
    jumps to the `x` in `x : Bool := true` (0:0).
  - *Smoke*: 20/20 `lsp_conformance` tests pass incl. new
    type-error/hover-type/def-binding cases; scripted stdio client
    verified `hnl lsp` end-to-end.
  - *Next-version scope*: completion/references/rename beyond the
    existing token-level implementations; richer hover (literals,
    keywords — currently identifiers only, `?m` for unsolved metas);
    `data` ctor bindings + sequential-`do`-block scope edges in the
    binding index; multi-diagnostic NBE errors (first span only
    today); per-workspace kits via the dormant `KitResolver`.

- **`hnl lsp` v2 landed** (`host-lsp` + `host-tooling` +
  `kit-prelude` + `kit-data-prelude` + `engine-typecheck-nbe`): the
  useful, data-driven language server — every feature still dispatched
  through `lsp.yaml` rule kinds, checker semantics untouched.
  - *Meta-zonked hover*: `NbeTypecheckEngine::typecheck` now zonks
    every *reported* type before `TypecheckResult` leaves the engine —
    `typ`, `node_types`, and each `Sig_m` field — via a new
    `dependent::zonk_reported`: tagged `{"__meta__"}` leaves resolve
    through the tagged store (`zonk_index`) and each domain-encoded
    subtree round-trips lift→quote so `Val::Flex` metas solved after
    quoting force through `ctx.vcheck`. Solved metas render their
    solution (hover shows `{n: Number, s: Str}`), unsolved keep `?m`.
  - *Non-identifier hover*: `type_hover_at` no longer gates on
    identifier tokens — any non-trivia token takes the tightest
    `node_types` span, so `+` shows `1 + 2 : Number`, `match`/`data`
    keywords and formers (`Vec`/`Succ`) show the enclosing node's
    type; comment tokens still take the classification hover.
  - *Completion*: `lsp.completion.scope_name` (new `scope_name`
    provider kind) reads the binding index in reverse — Γ at the
    cursor: module bindings, imports, `data` ctors, pattern/lambda
    binders, sequential block binds — deduped by label in
    `lsp.yaml` rule order. After `r.` the host asks the checker:
    the tightest node ending at the dot's `type.record`/`Sig_m`
    `fields` map becomes Field items with type details.
  - *References + rename*: `textDocument/references` and
    `textDocument/rename` entries in `lsp.yaml` map to the `binding`
    provider, which now resolves *every* `var` node's def back to its
    binder (innermost-scope wins) — scope-aware, so shadowed names
    don't conflate. `ReferencesPlugin`/`RenamePlugin` extend the
    `ToolingHost` seam; handlers fall back to the token-level
    implementations when no binder resolves.
  - *Scope index edges*: sequential `do`-block binds (each binding
    scopes `(end, block.end)`), `x = v in body` optional-body scope,
    `stmt.import` bound names, `stmt.export` signature names, `data`
    family name + ctor names at module scope + family params scoped
    over the constructors, and `field`/`callee`/`source` target
    resolution (`type.Score := …` binds `Score`). The node kinds are
    declared under a new `scope:` config on `lsp.def.var` — the
    vocabulary stays data, the walk stays role-driven.
  - *Multi-diagnostic NBE errors*: new `Kit::diagnose_with_prelude`
    keeps the engine's diagnostics in-band on `TypeInspection` instead
    of aggregating to `typecheck errors:` with the first span —
    `publishDiagnostics` maps every structured diagnostic to its own
    LSP range (`cannot unify — expected Number, got Str` squiggles the
    failing node, not the document).
  - *Smoke*: 26/26 `lsp_conformance` tests pass (the 20 v1 cases plus
    new scripted stdio cases for scope-aware references/rename,
    operator hover, scope-name + record-field completion, and
    node-spanned type diagnostics); `libhnl_ext_lsp.so` rebuilt and
    `hnl lsp` verified end-to-end over stdio.
  - *v3 scope*: per-workspace kits via the dormant `KitResolver`;
    UTF-16-exact positions (current column math is byte/character
    approximation); richer record-hover/keyword docs.

- **WS-H perf pass (session 4)** — measurement-driven; bytecode/cranelift
  parity gap found and closed.
  - *Parity fix (real gap)*: `tag_patterns_on_compiled_engines` in
    `handl-runtime::match_arrow_lower` failed with `unknown v2 native
    op: ctor_name` — `compile_pattern`'s ctor-object discriminant/binder
    ops (engine `term_interpreter`, session-2) were never added to the
    shared `native_ops` table, so every lowered ctor `match` crashed on
    bytecode + cranelift. `ctor_name`/`ctor_fields` now live in
    `engine-exec`'s `apply_native_op` (TaggedValue twin of the
    term-interpreter ops: `__ctor__` sym → its name, else `null` —
    total probe; sorted non-`__ctor__`/`type` fields — errors on
    non-object). Verified: `match_arrow_lower` 24/24 (both compiled
    engines), full `handl-runtime` suite green, including
    `ctor_object_match_dispatches_on_sym` on all three engines.
    Bytecode/cranelift already run `handle`/IO via `handle_dyn`
    (WS-E); this closes the `__ctor__` half of the parity question —
    verdict is now *real*, not documented.
  - *Instrumentation (kept, env-gated)*: `HNL_TC_STATS` /
    `HNL_CONV_STATS` counters — `stats.rs` in
    `engine-typecheck-nbe` (dup-pair + node-revisit probes + inclusive
    timers, one relaxed atomic read when off), `conv_stats()` in
    `handl-value::nbe`. `dependent_structure`'s `zzz_tc_stats_dump`
    prints the report.
  - *Measured* (`dependent_structure`, 48 tests, ~0.8 s): `dep_unify`
    205 calls/55.6 % structural dup pairs, `unify` 127/48.0 %,
    `unify_index` 364/28.3 %, `dep_synth_type` 1767/5.7 % node
    revisits, `instantiate_ctor` 379/99.7 % re-instantiation (by
    design — fresh metas per site), `nbe::conv` 22/4.5 %.
  - *Landed — `TermData` borrow views + scheme cache*
    (`engine` + `engine-typecheck-nbe`): `TermData` is now
    `repr(transparent)` with `TermData::view`/`get_term_view`/
    `get_term_view_list` — `eval_type_scheme_td` recursion borrows
    sub-schemes instead of deep-cloning a `serde_json::Value` per
    step (`get_term`/`get_term_list` cloned every level); `Op`
    judgment args pass `&[&TermData]` views. `TypecheckContext::
    scheme_td` caches `Term::from_term` per lang-owned `&Term`
    pointer (synth/check rule schemes convert once per kind, not
    once per node). Wall time on `dependent_structure` is
    parse/setup-bound (~0.8 s both ways; check-side
    `eval_type_scheme_td` inclusive ≈ 236 ms) — the win is
    structural, showing on larger type-rule-heavy inputs.
  - *Not landed (measured, upside too small)*: `conv`/`dep_unify`
    memoization — hit rates clear the >5 % bar, but the functions
    total ≤6 ms of ~800 ms suite time (≈1 % bound) and caching is
    unsafe around meta-solving side effects and the `results`
    environment; `instantiate_ctor` re-elaboration — the 99.7 % dup
    is *required* fresh-meta minting (the `ctx.results.clone()`
    snapshot is ~part of 10.6 ms total). Documented, not built.
  - *Microbenches*: `conv_bench_repeat_pairs`/`conv_bench_ptr_shared`
    extend `conv_bench_*` in `handl-value::nbe` — 500 iters of a
    64-deep pair: 3.60 ms structural vs 1.97 µs ptr-eq floor — the
    ceiling a `(a,b)`-keyed memo could recover.
  - *Suites*: 302 lib + 48 dependent + 46 parity +
    `handl-runtime` all green (`--test-threads=1`). Commits per repo.

- **Annotated-arm `VCons` reconstruction fixed (session 5)** — the
  session-3 still-open wrinkle. Two shadowings inside
  `instantiate_ctor_full`'s telescope elaboration, same fix shape:
  - *Family params*: `data Vec(n : Nat, a : Type)` bound `n`/`a`
    directly in Γ + `results` while the telescope elaborated, so an
    arm's own same-named rigids (`f{n, a}`) rewrote to the fresh
    metas. Params now bind under `__fam_<name>` internals and a new
    `ctx.fam_scope` rename map redirects the telescope's own
    `var{n}`/`var{a}` occurrences to them (`dep_synth_var_type`,
    `dep_synth_call_type` callees, `node_to_index_term` leaves/callees,
    `node_index_or_type` ctor probes); restored on every exit.
  - *Arg names* (the actual `Vec(Number, …)` mismatch): ctor arg
    names only self-bound in `results`, so a pattern-bound field
    (`VCons(k, x, xs)`'s `k : Nat` in Γ) shadowed the telescope's
    `var{k}` atom and `xs : Vec(k, a)` elaborated `Vec(Number, ?a)`.
    Args now self-bind `var{name}` in Γ *and* `results`, keeping the
    `var{aname}` atoms `param_name` substitution keys on.
  - Verified: `push{n, a} : a ~> Vec{n, a} ~> Vec{n+1, a}` with a
    `VCons(k+1, x, v)` arm typechecks; `hnl run` evaluates
    `f{1,Bool}(true)(VCons(0,true,VNil))` →
    `VCons{1, true, VCons{0, true, VNil}}`; regression tests
    `annotated_arm_family_params_dont_shadow_rigids` +
    `annotated_arm_elem_param_scopes_cleanly`; 302 lib + 51 dependent
    + 46 parity green; `lib-collections` typechecks. Commit
    `582d4e9` (engine-typecheck-nbe — folded into the auto-discharge
    commit).

### 2025 — Dogfood round 2: `lib-json-handl` (JSON parser/serializer in `.hnl`) + `State`/`IO`

Second gap-finding pass (report: `/tmp/dogfood2-report.md`). Wrote
`lib-json-handl/src/json.hnl` — `data Json`/`PErr`/`Result(e,a)`
families, checked `jfirst_scalar : Json ~> Json` (match-homogeneous
self-recursion), `toStr` worklist serializer, `parse` explicit-stack
descent producing `Result` ctor objects, `mk_*` manual `__ctor__`
mints (import-safe lib-option idiom), and **two `runState`s**:
a `.hnl` fold over the produced `IO` former's sig record (option A
literal) and a free-monad chain of single-op IO objects
(`IO({get:k})`/`IO({put:[s',k]})`/`IO({ret:a})`). File typechecks
clean; `hnl run` evaluates all 21 smoke assertions green, including
`runState_c` folding `get→put(s+1)→get→ret` to `(6,6)` — get returns
s, put replaces it, and the put's payload is computed inside the fold.

- **State verdict**: option A's record-fold is sound only as an
  op-*summary* (`get`→s0, `put`→final s, ≤1 entry/op, record order);
  real sequencing needs the chained encoding. `handle`/
  `interpret_io_former` dispatches each sig entry independently —
  there is no carrier for the state thread, so `handle` cannot
  express `runState`. The `.hnl` fold over `io.indices[0]` works
  because the ops ARE value-level `{opname:arg}` fields.
- **Ranked gaps** (new this round): `type.unknown` doesn't unify
  (poisons `+`/`if`/`&&`/`>=`/`==` on every host call — parser lives
  in `while` bodies); `while` bodies are a checking black hole (the
  escape hatch that makes everything writable); `IO(sig)` can't be
  *produced* in checked code (record mint synthesizes a plain record
  — `Former{type.io}` needs a declared stub or a real `perform`
  surface); `data` ctors are file-local — `__splice__` binds don't
  reach imported-arrow envs (ADR-0155) and `hnl test` can't desugar
  `data`; `:=` arrows are monomorphic across call sites (shared param
  metas — `{get}` vs `{put}` record calls collide); `Unknown` in an
  annotation → `LamVal` uncallable; `export` dead inside modules;
  `()`/`J`-keyword/`kv[1]`-tuple/record-in-arg DX.
- **Phase-4 candidates**: `type.unknown` consistent-equality (or
  declared `Sig`s for the ambient floor); typed `while` protocol;
  `IO`/`perform` value surface + reconcile `handle`'s impl contract
  with `interpret_io_former`; module-scope `data` envs +
  `desugar.data` overlay parity; let-generalization/`rec`;
  `export`-binds-locally (or drop for module-record); Unit `()`,
  tuple-index, reserved-keyword diagnostics, `hnl run --unchecked`.
- Carried: F6 (`zipWith`), F7 (`?m`), F8 (dict member). F1–F5
  verified fixed/adequate this session.

### 2026 — Three design decisions landed

- **Explicit `Sigma`, no `exists` sugar.** `filter{n,a} : (a ~> Bool)
  ~> Vec{n,a} ~> Sigma(Nat, m ~> Vec(m,a))` written in
  `lib-collections/src/collections.hnl` — `hnl typecheck` clean,
  `hnl run` end-to-end. Sigma intro is the `[witness, value]` pair.
  Friction recorded: a Sigma-destructured `m` reads as the `Nat` sort
  inside `Vec(m, a)` (bare names resolve through gamma to their sort;
  only `m + 0`-shaped compounds stay `var`), so the recursive repack
  `Vec(m+1,a)` never unifies with `Vec(Nat, ?m)` — the computing
  filter stays unwritable. `exists m. T` sugar →
  `host/project/BACKLOG.md`.
- **Auto-discharge for decided-impossible match arms.** In
  `synth_match_with_motive` (engine-typecheck-nbe): an arm whose ctor
  index unification is DECIDED-unsatisfiable is auto-discharged —
  skipped silently, body unchecked-but-allowed, no arm-type
  contribution; coverage never required it anyway. `absurd` stays
  legal as documentation; `absurd` on a reachable arm still errors. A
  ctor building a *different* family remains an error (nominal
  impossibility ≠ index decision — almost always a typo). Fixes the
  `zipWith` catch-22 (inner `VNil` under `n ≡ ?k+1`). Tests:
  `match_auto_discharges_decided_impossible_arm`,
  `match_auto_discharge_zipwith_shape`; dependent_structure 51, lib
  302, parity 46 all green. `typecheck_model.hnl` (fixture + design
  copy) updated to the same contract.
- **`export` binds + `hnl run --name`.** The shared `export` op
  (engine-exec `native_ops`) `env_assign`s the declared name —
  `export x := v` binds `x` via the child's `target` role, self-ref
  exports get `with_self_ref_bind`; engines without Env keep the
  pass-through. `hnl run` on an export file evaluates to the block
  tail; `--name f` splices `f` as the tail (a `do`-leading source gets
  it before the leading block's matching close). Verified:
  `export x := 41; export y := x+1; y` → 42, `--name y` → 42,
  collections.hnl `hnl run` → `smoke_tail` evaluates. Remaining gap:
  `export` inside a `do {…}` block is still an unbound wire at
  typecheck (scope pass doesn't bind it) — pre-existing.

### 2026 — CI migrated from Dagger to the bupha shell runner

- `bupha` (github.com/9aia/bupha — headless CI: webhook → queue →
  detached-worktree → runner → status) now drives host CI via
  `runner.kind = "script"` → `bash scripts/ci.sh`. The Dagger module
  (`dagger/src/ci/main.py`) is deprecated on disk; the container engine,
  image pulls, and engine round-trip are gone from the gate.
- `scripts/ci.sh` carries the stage surface 1:1 — `doctor`, `test`
  (serial nextest partitions + doctests), `test-shard`,
  `coverage-report`, `coverage-gate` (tooling ≥99% lines), `dogfood`,
  `ci` — with the module's env block (`CARGO_INCREMENTAL=0`, `debug=0`)
  and the `-j 2` memory guards (`CI_JOBS`/`CI_DOGFOOD_JOBS`).
- Worktree plumbing replaces container mounts: `[patch]` tables resolve
  through a symlink farm in the worktree's parent; the `../host`
  self-patch is rewritten to the worktree's absolute path (a symlinked
  `../host` spells each self-patched crate twice and collides in the
  lockfile — the module's absolute `/work/host` self-patch avoided the
  same trap); `HANDL_TOKEN`/`BUPHA_TOKEN` become an env-scoped
  `insteadOf` rewrite (`GIT_CONFIG_*`, nothing lands in `~/.gitconfig`);
  cargo/sccache caches become shared `.ci-target/{host,dogfood}` dirs.
- Post-ADR-0149 dogfood: `hnl` is a dumb host — `fmt`/`lint`/`typecheck`/
  `test` are `libhnl_ext_*` cdylibs from the ext-* repos, discovered via
  `HANDL_EXT_DIR`. `ci.sh dogfood` builds the four exts + `hnl` into
  `.ci-target/dogfood` — on a detached worktree, from scratch-parent git
  worktrees whose `../host` is the sha under test (the dagger module
  mounted a cli source and dev-linked it; same effect, no containers).
- `scripts/ci-watch.sh` generates a one-repo bupha.toml per run and calls
  `bupha --config … run`; statuses post as `ci/bupha`. `host/bupha.toml`
  is the committed standalone manifest for the shared watcher merge.
- Pre-existing breakage surfaced, left on the dagger side: `dogfood`
  still mounts the cli source as `name="cli"` while the gate looks for
  `../host-cli` (stale since the cli → host-cli rename, ADR-0149), and
  dev-link on a marker-less Cargo.toml duplicates existing `[patch]`
  tables.

### 2026 — Index-position syntax `{…}` → `<…>` (dimension IS an index)

- Parser: the `<` special production (`dimension_applies`/`parse_dimension`,
  engine-parse-pratt) now carries the former `{…}` index-application and
  implicit-binder surface — `Vec<n, a>`, `f<a, ev : Eq(a)>`, `IO<{ puts: T }>`
  all mint `dimension` `[target, args…]`. `parse_dimension` gained the
  whitespace-application fold (`Vec<Succ n, a>` ⇒ `call(Succ, n)`) and a
  post-arg `:` wrap for sorted binders (the `type-annot` kind is read from
  the `:` infix row's child shape — engine-generic). The `{` index-app
  production (`index_app_applies`/`parse_index_app`) is deleted; `{…}`
  everywhere else stays records/blocks/match arms. Old `{` forms now fail
  to bind (verified: `/tmp/hnl-verify/instances*.hnl` in `{…}` spelling
  error out; the `<>` spellings typecheck).
- Typecheck (nbe): `dimension` joins the `"call" | "index_app"` arms —
  `parse_family_head`, `dep_synth_type`, ctor-telescope index terms,
  `node_to_index_term`, `node_to_ann_term`; `synth_call`'s implicit-pi
  index fill covers it (`is_index_app` → `index_app | dimension`).
  `implicit_binder_head` already accepted `dimension`. Bootstrap scope
  pass: `implicit_binder_names` accepts `dimension` targets.
- Data: `type.dimension` (synth_call + check, mirrors `type.index_app`)
  added in kit-prelude-lang/types.yaml — fragment count 86→87 (88 with the
  parked `instance` WIP in-tree). kit-data-prelude assembly asserts bumped
  to the merged counts (tokens 107, lexer 105, grammar 87, eval 91, ast 97).
- Semantics notes (approved): `Vec<n,a>` in eval position now applies via
  `transform.desugar.dimension` → `call` (vs `{…}` erasure) — no `.hnl`
  source puts a former application in eval position. Unannotated
  `f<n,a> := body` binds runtime params (`f := n ~> a ~> body`,
  ADR-0106 §12) — annotated `f<n,a> : T = body` keeps implicit-erasure.
  `index_app` node kind remains for pre-desugar/hand-built trees.
- Sources: `lib-collections` collections.hnl fully migrated
  (`vsingleton<a>`, `Vec<Succ n, a>`, `eqv<a, ev : Eq(a)>`,
  `IO<{ puts: Unit }>`, `handle printLn with { puts: h }` — record braces
  stay `{ }` spaced). lib-json-handl/lib-type doc spelling updated.
  Tutorial: `IO<sig>`/`handle <arrow> with <h>` (deep-carry idiomatic)/
  implicit `<…>` binders.
- Verified: engine-parse-pratt 62+conformance, dependent_structure 51/51
  (incl. the parked `instance` WIP's tests — `synth_instance` is committed),
  lang_merge 7/7, kit_assembly 5/5, `hnl typecheck` clean on
  collections.hnl + json.hnl + `/tmp/hnl-verify/*_angles.hnl`,
  `hnl run` collections.hnl evaluates, `1 < 2` still parses as comparison.
- Parked: kit-prelude-lang carries an unrelated uncommitted `instance`
  WIP (WS-A fragments: token/lexer/grammar/ast/eval/types + count bump) —
  its committed `synth_instance` + tests need those fragments; left in
  tree, flagged for its owner.


### 2026 — hnl-level smoke suite wired into bupha CI (`tests/smoke/` + `ci.sh smoke`)

- New `host/tests/smoke/` suite — one `.hnl` file per phase-3 surface
  area, driven by the real `hnl` binary + `libhnl_ext_{typecheck,eval,
  test}` cdylibs (no Rust changes):
  - `types.hnl` (`hnl typecheck`): `data Nat`, `data Vec(n,a)` indexed
    family, dependent `match` refinement (`vhead`/`vtail` VCons-only
    exhaustive; two-arm `is_nil`), `Vec.rec` eliminator call, `Path`/
    `refl`, `p # v` transport, and `J` inside a `match … return (i,w) ~>
    T with` motive arm.
  - `types_err.hnl` (expect failure): `x : Bool := 3` →
    `cannot unify — expected Bool, got Number`.
  - `effects.hnl` (`hnl typecheck`): `IO<{ puts }>`/`IO<{ fs }>` sig
    annotations, `handle` covered (`puts: Unit` replaces the op entry)
    and partial (`{puts: Unit, fs}` residual), widening accepted
    (`IO<{puts}>` where `IO<{puts, fs}>` required).
  - `effects_err.hnl` (expect failure): `IO<{puts, fs}>` used where
    `IO<{puts}>` required → `cannot unify` (effect narrowing rejected).
  - `instance.hnl` (`hnl typecheck`): `instance Eq(Nat) := dict` +
    `<a, ev : Eq(a)>` sorted implicit binder; `eqv(Zero)(Zero)`
    discharges by dictionary search.
  - `alias_forall.hnl` (`hnl typecheck`): `type.NatAlias`/`type.BoolVec`/
    `type.Hom2(a)` transparent aliases, `forall n : Nat, T` (explicit-Pi
    sugar — the forall's first arg IS the index; `idn(Succ(Zero))`
    instantiates, a bare `2` is `Number` not `Nat`), `~[2]~>`/`~[3]~>`
    cell terms (`>>>[2]`/`id` laws).
  - `eval.hnl` (`hnl run` + `hnl test`): `eval.checked(self, "1 + 2")`
    → `{value,type}`; `sym.seeded`/`__ctor__` manual mints (lib-option
    idiom), `type.instanceof`, `match` on minted ctors; `test_*` +
    `assert` for the test driver, a `while`-body `chk` block for the
    run driver (`==` on `type.unknown` host results is a checker error
    — the while body is the typed black hole).
- All files are spelled in the new `<>` index surface (`Vec<n, a>`,
  `f<a, ev : Eq(a)>`, `IO<{ puts }>`) — the `{…}` `index_app` spelling
  is gone in-tree (see the `{…}` → `<…>` entry above).
- `scripts/ci.sh`: new `smoke` stage; the worktree scratch-parent +
  ext-build plumbing is factored into `setup_hnl_toolchain`/
  `cleanup_hnl_toolchain`/`build_toolchain_exts` shared with `dogfood`.
  `SMOKE_EXTS=(ext-typecheck ext-eval ext-test)` — everything lands in
  the shared `.ci-target/dogfood` target dir, so `smoke` after
  `dogfood` is incremental. `smoke` is appended to the `ci` stage list
  (per-push) with a `CI_SKIP_SMOKE` guard; `ci-watch.sh`/`bupha run`
  pick it up free.
- Verified: `bash scripts/ci.sh smoke` green end-to-end on a cold
  `.ci-target/dogfood` (release exts + `hnl` built from siblings,
  ~5 min) — 4 typecheck files `ok`, `hnl run eval.hnl` → `[true×4]`,
  `hnl test eval.hnl` → 4/4 passed, both `*_err.hnl` files fail with
  the expected diagnostics.
- Notes / gaps surfaced while writing the suite: `hnl test` cannot
  desugar `data` (known — eval.hnl is data-free on purpose);
  `match ev { mkEq(f): … }` on an implicit-evidence binder typechecks
  but can't run (`ev` has no runtime dict — erasure class), so
  `instance.hnl` stays check-side; `assert X = n` parses under
  `hnl test` but not under `hnl run`'s checker (use nested literal
  `match`); `hnl run` typechecks first, so a file carrying Path/J-only
  bindings can't run — `types.hnl` is typecheck-only for that reason.

### 2026 — `perform` op emission (`io` module + `synth_perform`/`check_call`)

- *Where it landed*: `perform` is a **host function on the new `io`
  module** — `hnl_engine_exec::io::{IoModule, PerformFn}` in
  `engine-exec` (new `src/io.rs`, next to `interpret_io_former`'s
  semantics). No AST node, no grammar/eval-scheme change: the surface
  form is an ordinary `call` whose callee resolves the `sym.seeded` way
  — unbound `var` → name string → `call_host` → `ModuleRegistry` —
  so tree-walk/bytecode/cranelift all reach it identically. Both the
  flat `perform(puts, "hi")` and qualified `io.perform(puts, "hi")`
  spellings resolve. Registered in `kit-prelude`'s `PRELUDE_MODULES`
  (native row) + `io` in `kit-data-prelude`'s `prelude.yaml`
  `base_modules` (which also seeds `io`/`perform` as checker wires via
  `prelude_wire_names`).
- *Runtime mint*: `perform(op, arg)` returns the existing IO former
  shape — `{"__former__": <Sym seeded "type.io">, "indices": [{op:
  arg}]}` — exactly what `interpret_io_former` dispatches under
  `handle`. `op` accepts a string, or a `__sym__` value (its display
  name is the op name); `arg` defaults to `null` (`perform(op)` →
  `{op: null}`).
- *Type rule* (`type.perform` isn't a rule — the typing lives on the
  generic `type.call`): `synth_call_judgment` intercepts a
  `perform`/`io.perform` callee (unbound-in-Γ — source bindings shadow)
  → `synth_perform` mints `Former{type.io, [sig{op: T}]}` where `op` is
  read as a *name* (`var` → its name, `value`/`lit` → string payload,
  `sym("n")`/`sym.seeded("n")` → seed) and `T` is the arg's synthesized
  type (`type.unit` if omitted). The new `check_call` judgment
  (`type.call`'s `check:`) defers `perform` calls to synth+`dep_unify`
  (record-width sig subtyping: `IO{puts:Str}` checks against
  `IO{puts,fs}`); every other call reproduces the old YAML check scheme
  verbatim (synth callee → check arg vs `param` → yield `body`).
- *Scope walk* (`engine-typecheck/src/bootstrap.rs` `check_expr`): a
  `perform`/`io.perform` call's bare-`var` op operand is a name
  position (like an object `key:` or `field-access` field), not an
  unbound wire — exempted; callee/args still walk.
- *Op-name representation*: strings — `perform(puts, …)`'s bare `puts`
  arrives as `"puts"` via the unbound-var→name rule, and handler
  records dispatch on string field names. `sym("puts")` operands map to
  the same `"puts"` name.
- *Merge semantics (v1, decided)*: each `perform` mints a fresh
  single-entry sig — `do { perform(puts,"a"); perform(puts,"b") }`
  produces two `IO` values (the first is discarded as a statement
  value; there is no ambient sig accumulation). No merge protocol —
  `interpret_io_former` interprets whatever sig record arrives.
- *Verified*: `hnl run` `x := perform(puts,"hello"); handle x with
  {puts: s ~> s}` → IO object `{puts:"hello"}` (former handle
  `16969724874574109231`, the `type.io` seeded nominal). `hnl
  typecheck` `y : IO<{puts: Str}> = perform(puts,"hi")` → ok.
  `engine_parity.rs` `io_perform_mints_io_former` +
  `io_perform_through_handle` green on all 3 engines;
  `dependent_structure.rs` 5 `perform` tests (mint/both spellings,
  wrong-sig rejection, handle coverage + spurious/empty diagnostics,
  computed-op rejection, shadowing) green; `effects.hnl` smoke extended
  with `perform` bindings; `effects_err.hnl` still fails as required.
- *Open issues*: (1) the `hnl-wire-prelude.json` cache is keyed on the
  `hnl` binary's identity — a `.so`-only rebuild (ext cdylib change,
  `hnl` untouched) serves a stale wire set; observed during this work
  (delete `~/.cargo/shared-target/hnl-wire-prelude.json` to bust).
  (2) Handler coverage `check_subtype` does not solve metas — an
  anonymous `s ~> s` impl against a `Str` sig entry leaves `?m`
  unsolved (`IO{puts:?m}` ≠ `IO{puts:Str}`); declared impls work, smoke
  uses `puts_payload_impl : Str ~> Str`. (3) `perform`'s sig entry is
  the *payload* type (`Str`), not the op's declared arrow type — the
  two sig shapes coexist intentionally (declaration stubs vs minted
  requests) but a shared op-type discipline is a follow-up.
  (4) `puts` bound to a different value passes that value through —
  documented; users should bind op names to their own names or spell
  them as strings.

### 2026 — typed `while` bodies (engine-typecheck-nbe `73673c9`, kit-prelude-lang `38c76ed`)
- `type.while` → `synth_while` judgment (was `type.unknown` — 100% unchecked): statement-form checks cond→Bool + body under `ctx.mark()/restore` (binds don't escape, rebinding shadows locally); state-form reads `{state}` head per `build_loop_step_args`, body checked against internal `type.__loose_top__` sentinel so `{state}`/`{out}` records never join but internals report.
- `loose_synth_node` dispatches by `AstShapeDef` roles only; `ctx.loose` yields fresh metas for record/array access on unsolved metas.
- 7 new `tests/while_typing.rs` tests; lib 302/302. Fallout noted: `lib-desugar/src/index.hnl:2584` `+`-on-`Str` bug inside a while body (pre-existing, now detected); `if cond` on `type.unknown` in while bodies still fails (pending `type.unknown`-top workstream).

### 2026 — `perform` primitive (engine-exec `1f7976d`, nbe `7e36925`, kit `a06f992`/`ec09d43`, host `7ebe493e`)
- `perform(op, arg)` / `io.perform(...)` — host fn on new `io` prelude module (no AST/grammar change; resolves via `call_host` like `sym.seeded`): mints `{"__former__": type.io, "indices":[{op:arg}]}` — dispatchable through `handle`/`interpret_io_former` on all 3 engines.
- Checker: `synth_perform` — `perform(op,arg)` synthesizes `Former{type.io,[{op:typeof(arg)}]}`; bare-`var` op operand exempt from unbound-wire (engine-typecheck `4b3f579`).
- v1 merge semantics: each `perform` mints a fresh single-entry sig — no ambient accumulation; `do { perform(a); perform(b) }` yields two IO values.
- Open: `check_subtype` doesn't solve metas in anonymous handler impls (use declared impls); sig entry = payload type not op arrow type (both sig shapes coexist intentionally); `hnl-wire-prelude.json` cache is binary-keyed — `rm` after `.so` rebuilds.
