# Phase-2 implementation spec — unified type system

Normative design: ADR-0106 (kernel, as amended) + 0152 (`data`) +
0153 (`~[n]~>` dimension index) + 0154 (identity/equality/universes/
literals/erasure) — same as phase 1. This spec continues the work
breakdown from phase-1's open items; waves of ≤4 parallel workstreams.

## Phase-1 end state (what exists)

Dependent `data` families with `F.rec`, `match` refinement,
`transport`/`J`, `Path`-as-data, `Hom(n)`/`>>>[k]`/`id_k` (n≤2
checked, n≥3 stored), `Type(i)` universes, seeded nominals +
`type`/`parent` links, imports→`Sig_m`, `type.Name` aliases, `J`
sugar, `IO(sig)` typed effects (type-level only — runtime op-dispatch
is a parallel in-flight task), exec-erasure + artifact lint, `.hnl`
checker deleted, `.hnl` desugar overlay synced. Verified: 283 lib +
24/24 dependent + 46/46 parity + 45/45 runtime + 6/6 overlay parity.

## Workstream map

| WS | Scope | Repo(s) | Blocks |
|---|---|---|---|
| A | Runtime `IO` flow — verify + finish (in-flight task) | `engine-exec*`, `host`, `kit-prelude-lang`, `lib-type` | F |
| B | `__tag__`/`{tag}` full deletion | `kernel`, `kit-core`, `kit-prelude`, `engine-transform-rewrite`, `lib-desugar`, `lib-type`, `engine-typecheck-nbe` | — |
| C | Implicit args + surface sugar (`f{A}`, `match` w/o motive, `Path` notation, `forall` shorthand) | `kit-prelude-lang`, `engine-transform-rewrite`, `engine-typecheck-nbe`, `kernel` (`handl-language`) | — |
| D | Numeric/index solver hardening (`conv`-integrated normalization + bounded solver iface) | `engine-typecheck-nbe`, `kernel` (`handl-value`) | — |
| E | `Hom` n≥3 + coherence; `>>>` real composition (0106 amendment) | `engine`, `engine-typecheck-nbe`, `kernel` (`handl-value`), `lib-type` | — |
| F | Typed-effect signatures — handler typing + effect-row polymorphism | `engine-typecheck-nbe`, `kit-prelude-lang` | (needs A) |
| G | `eval`/metaprogramming — checked terms from runtime mints, `Val`→AST quoting | `kernel`, `engine`, `engine-typecheck-nbe`, `host` | — |
| H | Perf — conv/normalization, `eval_judgment` stack depth, `Env` walks | `engine-typecheck-nbe`, `kernel`, `engine` | — (continuous) |

Waves: **1: A ‖ B ‖ C** → **2: D ‖ E** → **3: F ‖ G ‖ H** (4 max
concurrent; H slots wherever capacity frees).

## A — Runtime `IO` flow (verify + finish)

`IO(sig)` is type-level only since phase 1 (`FormerSig::Io`,
`type_sym("type.io")`, record-width subtyping, `dep_unify` IO arm).
Runtime op-dispatch is a parallel in-flight task — this WS verifies
what landed and finishes the remainder.

- `handle f with h` at runtime = `Term::Handle` dynamic-extent
  install + `Term::Op` handler-stack dispatch (tree-walk
  `handler_stack`; bytecode/cranelift `eliminate_handlers`). Verify
  ops declared in `IO(sig)` dispatch to the pushed handler and the
  handled result flows out.
- `print`/`read` (or the `io` module's op surface) produce/consume
  `IO` values — an `IO(sig)` codomain on the op's declared
  signature, not just a host return.
- Untyped ops keep working unchanged (record-width is opt-in).
- Scope: `engine-exec/src/native_ops.rs` (`handle` shell +
  op-dispatch arms ~L966-990, ~L3029-3164),
  `engine-exec-tree-walk/src/lib.rs` (`handler_stack`, `Term::Op`),
  `engine-exec-bytecode/src/vm.rs`,
  `engine-exec-cranelift/src/lib.rs`,
  `kit-prelude-lang/data/eval.yaml` (`eval.handle` ~L1216),
  `host` (`Language::register_op`/`register_handler`, module chain).
- Exit: e2e `handle` on an `IO({puts})`-typed arrow dispatches
  `puts` to the handler and returns its result; unhandled op on a
  typed sig → diagnostic; tree-walk/bytecode/cranelift parity.

## B — `__tag__`/`{tag}` full deletion

All bridge arms kept for migration in phase 1 — remove them and the
producers. Boundary: `__tag__` as a *payload discriminator* on
`Cell`/`Cell2`/`Arrow`/`op`/`Handler` model data is a value-shape
convention, not type semantics — **out of scope** here (separate
cleanup, see open items).

- `kernel/crates/handl-value/src/lib.rs`: `legacy_type_seed`,
  `legacy_tag_roles`, `__tag__` arms in `nominal_head` (~L1916),
  `type_link` (~L1943), `instance_of` (~L1987-2003).
- `kit-core`: `type.of`'s remaining kind-string semantics for
  `__tag__` data (the explicit-`type`-link read already landed);
  `lift`'s `canonical_tag_seed` bridge in `checker.rs` ~L1116.
- `tag.tag`/`tag.construct` producers: `TagModule`
  (`kit-prelude/crates/kit-prelude/src/lib.rs` PRELUDE_MODULES
  `"tag"` row ~L177) + `eval.yaml` schemes (`rewrite`/`law` mint via
  `tag.construct` ~L1231/L1256) — replace mints with canonical value
  shapes or keep ops emitting non-tag data; sweep consumers first.
- `fold_chain`/`build_fold_chain` `v.tag` disjuncts — **both
  impls**: `lib-desugar/src/index.hnl` `fold_chain` (~L1059, the
  `binary.or` bridge arm) and
  `engine-transform-rewrite/src/native/desugar.rs`
  `build_fold_chain` (~L1709, identical `or` arm) — dispatch only on
  `__ctor__` sym identity.
- `lib-type/src/dependent.hnl` `{"__tag__": "type.sigma"}` mint
  (~L170): `sym.seeded(...)` is **not reachable in the checker's
  term env** (confirmed — `dep_synth_call_type` evaluates only the
  annotation fragment; `sym` calls in annotation position fail
  `dep_synth_type`). This item therefore becomes: **make `sym`/
  `sym.seeded` reachable in `.hnl` term env** — a `nominal`/`sym`
  judgment op (`judgments.rs` already has a `"nominal" | "sym"` arm
  ~L684 — wire it into `dep_synth_call_type`/`eval_ann_term`) or a
  host-op seam; then `Sigma`/`Path` mints become
  `{"__former__": sym.seeded("type.sigma"), "indices": [A, F]}`.
- Exit: `type_link`/`instance_of`/`type.of`/`lift`/`dep_*` carry no
  `__tag__`/`{tag}`-string reads; fold chains dispatch only on
  `__ctor__`; `dependent.hnl` mints real former values; a legacy
  `{"__tag__": "int"}` datum fails membership checks **honestly**
  (diagnostic, not silent pass). Suites: nbe lib/parity/
  dependent_structure, handl_desugar 6/6, `tagged_values`,
  `type_model_parity`, `value_model_parity` — all green.

## C — Implicit arguments + surface sugar

The kernel is elegant but `Nat.rec` motives are verbose — phase 2
makes the surface match.

- `f{A}` implicit former-binders — index params inferred from call
  sites (the same mechanism `List<T>` uses for `T`, now for `Dim`/
  family indices); `forall`-bound param shorthand desugars to a
  `Pi`-bound index (`id<d : Dim>` per ADR-0153 §3).
- `match` inference without explicit motive — `return … with`
  optional: motive defaults to a fresh meta solved from the expected
  type / arm bodies; `synth_match_arms`'s `apply_motive` path keeps
  the explicit form working.
- `Path` notation — consistent surface for `a ~Path~ b` /
  `Path(a, b)` in type-expression position.
- Scope: `kit-prelude-lang` (lexer/grammar/`types.yaml`/`ast.yaml`
  + transforms), `engine-transform-rewrite` (desugar natives),
  `engine-typecheck-nbe` (`dependent.rs` `synth_match_arms`,
  `dep_synth_call_type`, implicit-index binding),
  `kernel/crates/handl-language` (`AstShapeDef` roles for new node
  kinds — role-driven, no `kind ==` checks).
- Exit: `match n { Zero: 0, Succ(p): p }` typechecks without
  `return … with`; `f{A}`/`<d : Dim>` shorthand infers indices;
  dependent_structure + parity green.

## D — Numeric/index solver hardening

`solve_index` is linear + exact division + integer roots today
(`dependent.rs` ~L457-545) with bespoke canonicalization in
`eval_index` (`x+0→x`, `2*?n` commutation ~L369-405). Generalize to
the ring-identities domain via `conv`-integrated normalization + a
bounded solver interface.

- Move index normalization to the domain: `Bin`/`Un` index terms
  evaluate to canonical form inside `conv` (semiring normal form —
  `n+0`/`n*1`/`n*0` identities, commutativity, associativity,
  constant folding) — normal-first, solver-second.
- `IndexSolver` interface: `prove(a ≡ b) -> Proven | Disproven |
  Unknown` — bounded (fuel + term-size caps), deterministic;
  non-linear two-unknown stays `Unknown` (stuck ≠ error, unchanged).
- Interface sized so an SMT backend (Z3/CVC class) can slot behind
  the same `OpHandler`/service seam later — no call-site changes.
- Scope: `engine-typecheck-nbe/src/dependent.rs`
  (`eval_index`/`solve_index`/`subst_index`/`zonk_index`,
  `unify_index` ~L549), `checker.rs` `conv` integration,
  `kernel/crates/handl-value/src/nbe.rs` (if normalization lands on
  `Val` index terms).
- Exit: ring identities prove by normalization without solver
  calls; `Vec(n+0) ≡ Vec(n)`, `Vec(n*m) ≡ Vec(m*n)` convertible;
  two-symbolic-factors still honestly stuck; solver iface isolated.

## E — `Hom` n≥3 + coherence (n=3 first, n≥4 aspirational)

- **`>>>` real composition first** (0106 amendment, promoted
  blocker): in `engine/src/nbe.rs` `Term::Seq` `k==0 && d<=1` still
  *applies* (`eval(snd, mid)` ~L126-128) instead of forming a
  composite `Val` — cell-law typing and reindexing (`f >>> P`)
  assume `>>>` composes. The `cell_comp_axis` delegation seam exists
  (`k>0 || d>1` ~L129-140): route `k==0` on cell operands through it
  too, or materialize a composite `Val::Arrow`/cell value (kernel's
  `cell_comp_axis(0,·)` already mints `Arrow{first, second}` data).
- `checker.rs` `infer_hom` n=3 checking: `Hom(3, e, e')` requires
  both operands `≡ Hom(2, f, g)` with convertible boundaries — same
  boundary law one level up; `infer_comp_axis`/`infer_id` for k=3.
- `judgments.rs` `cell_comp_axis`/`cell_id` arms: axis-3 typing.
- `kernel` `cell_comp_axis` runtime n=3 composition.
- `dependent.hnl` `~[3]~>` coherence terms (`comp_assoc`,
  `id_left`, `id_right`, `interchange`) become **checked**, not
  stored; globular interchange for n=3.
- Exit: `e >>>[2] eInv ~[3]~> id(f)` *checked* (boundary law
  enforced by `conv`); n≥4 stays stored-unchecked — honest bound
  kept explicit.

## F — Typed-effect signatures (handler typing)

`IO(sig)` record-width subtyping landed; extend to handler typing +
effect-row polymorphism. Depends on A (runtime flow verified).

- `handle f with h` typing: `f : A ~> IO(S) ~> A'`, `h` supplies
  impls for `S`'s ops — missing impl → diagnostic; result type =
  handler's declared return; residual unhandled ops → `IO(S\H)`.
- Effect-row polymorphism: `S` as meta/sig-variable binds from the
  annotation (`IO(?s)` solves like other indices).
- Scope: `dependent.rs` (`synth` for `handle` nodes),
  `judgments.rs`, `checker.rs` `subtype`/`conv`, `types.yaml`
  `type.handle` op.
- Exit: `handle read>>>print with Console` checks impls against the
  declared sig; missing impl rejects; polymorphic sig binds.

## G — `eval`/metaprogramming

Runtime-minted types/laws/cells are already legal values (0154 §6 —
laws-as-values, lint-exempt). Add the *checked* direction:
`eval` producing checked terms + quoting `Val` → AST for
metaprograms.

- `quote` exists (`Val` → boundary JSON); need `Val`/`TaggedValue`
  → `Term`/`Ast` materialization so a metaprogram's product re-enters
  the pipeline (parse → typecheck → eval) instead of only being data.
- `eval` primitive on a quoted artifact runs the full check path —
  a runtime-minted term is *checked*, not trusted.
- Scope: `kernel/crates/handl-value/src/nbe.rs` (quote side),
  `engine/src/nbe.rs`, `engine-typecheck-nbe` (`synth` entry
  accepting materialized AST), `host` (`eval` primitive wiring).
- Exit: `Val → AST → infer` round-trip covered by tests;
  runtime-minted cells still pass artifact lint unchanged.

## H — Perf (continuous)

- Conversion/normalization hot paths on `Val` — profile `conv` on
  stdlib-size files first (the `dependent_structure` suite is the
  heaviest consumer: `eval_judgment` hops are several frames each).
- `dependent_structure` 64 MiB-stack case
  (`dependent_hnl_parses_and_typechecks` legitimately needs ~10 MiB
  check stack; `indexed_family_*` needed 32 MiB through the `.hnl`
  desugar overlay): stack-growth fix or explicit-continuation eval
  for `eval_judgment`/overlay eval — a real fix beats per-test
  `spawn(64MiB)` workarounds.
- `Env` persistent cons list — repeated `len()` O(n) walks; cache
  or share. `MetaStore` mark/truncate is fine.
- Exit: `dependent_hnl_parses_and_typechecks` runs on default
  8 MiB stack; conv microbench baseline recorded; `Env::len` not
  O(n) on hot paths.

## Cross-cutting invariants (unchanged)

- One value space: checker and exec share `Val`; arrow *values* stay
  self-contained serializable data (`body_def`+`binds`) —
  **defunctionalized data**, closures only as domain machinery,
  never as language objects (0106).
- Types-as-values; every type-level entity is an index on a nominal
  former; **nominal heads + structural bodies** (0154 §3).
- **Dimension ≠ mode ≠ effect axes** (0153): the `~[n]~>` bracket is
  cell dimension; `IO(sig)` is an object in the codomain;
  usage/erasure is a mode axis — none shares the bracket.
- Indices are type-level, erased at exec; the artifact lint stays
  the verifier (0154 §6).
- Engine-genericity: calculus vocabulary may be fixed in Rust;
  language constructs are `Language` data (`AstShapeDef` child
  roles, `types.yaml` dispatch) — no `kind ==` shape checks, child
  positions resolved by named role.

## Open items (tracked, not blocking)

- `__tag__` as payload discriminator on `Cell`/`Cell2`/`Arrow`/
  `op`/`Handler` model data — separate cleanup after WS-B (owner TBD).
- `sym` seeds for unexported formers; FFI boundary convention
  (carried from phase 1).
- `type Name body` contextual parse (carried; the `type.Name :=`
  assign form landed instead — decide if the dedicated form is still
  wanted).
- ADR-0155/0156 (scope-bound arrow environments / `ModuleEnv`) —
  separate host track, out of this spec's scope.
- `hnl-gate.sh` not runnable on the dev box (no built `hnl` + ext
  `.so`s) — `.hnl` lib changes gated by crate tests until a build
  exists (carried).

## Progress log

- **WS-E wave-2 (landed) — `>>>` composition + `Hom(3)` checking.**
  - *`>>>` verdict*: the flag was **half-stale**. Runtime `f >>> g`
    already composes (exec `flow` arm → `cell_comp_axis(0,1)` →
    `Arrow{first,second}`; `eval.axis_flow` k=0 routes through it;
    `Seq{0}` on body-arrow schemes is correct composite semantics —
    `(g∘f)(x) = g(f(x))`; `Op{flow}` value-operand dispatch landed
    with `d8f1e0b7`). The residual wrong reading was `nbe::eval` /
    `term_interpreter` `Seq{0}` on *operand terms denoting cell
    values* — `eval(snd, mid)` treated `snd` as an arrow on `mid`, so
    an input-free `snd` returned its own value with `fst` silently
    dropped. Fixed: input-free `snd` (`Arr`/`Const`) + both evaluated
    operands cell/morphism-valued → `cell_comp_axis` seam (`Const`
    payloads → kernel `Arrow{first,second}` mint; domain cells → the
    checker's `infer_comp_axis` — `Hom(1,a,b) ∘ Hom(1,b,c)`). Also
    added the missing `flow` arm to `JudgmentOps` (was
    unknown-op error under `veval`). Commits: `engine 308ef2c`,
    `engine-typecheck-nbe b2ab3e7`.
  - *`Hom(3)` checked* (was stored): `infer_hom` n=3 via
    `cell_faces_at` (`tgt_j = src_j` recursion — deeper faces through
    the source boundary's type; lower cells promote to iterated
    identities; `Hom(?d)`/meta endpoints stuck → stored);
    `infer_comp_axis` k=3 (gate moved to ≥4; deep-axis law corrected
    to `tgt_k ≡ src_k` via `cell_face_at` — previously compared top
    faces); `infer_id` id_3 checked, id_4 stored.
  - *Interchange*: `Hom(3, LHS, RHS)` *checks* — both sides infer to
    convertible `Hom(2)` boundaries, so `=>|`/`=>>` interchange is
    verified as a well-formed coherence cell (boundary law by conv).
    The strict `cell.comp`-tree equality is **stated, not
    definitional** — a comp-tree normalizer for interchange is a
    follow-up, not landed.
  - Verify: dependent_structure 24/24, parity 46/46, lib 285 —
    all green.

- *(spec drafted — no work landed yet)*

- **WS-C wave-1 (implicit index binders + surface sugar) — landed.**
  `f{a, b} : T` implicit index binders (adjacent `{` brace on an
  `assign`/`type-annot` subject desugars to implicit `type.pi` binders —
  never lambda-consuming, erased at runtime); `F{i, …}` index-argument
  application in annotation position (`Vec{n, a}` ≡ `Vec(n, a)` former
  index-app; `Vec{Succ n, a}` — whitespace inside braces is application,
  ctor callees become `App` index terms); `forall n : Nat, T` ≡ `Pi(Nat,
  n ~> T)` parse-level alias (`forall` keyword minted — free, only used
  in comments before); `match` may drop `return … with` when an expected
  type exists — motive inferred from the expected type's family-app
  index pattern (constant motive otherwise). Implicit app-site solving =
  ordinary meta + `unify_index` (plus `App` ctor-injective arg unification).
  Parse: `{` index-app is a special production in the Pratt engine
  (adjacency-gated like `dimension` — `Vec{n}` vs `v { … }`); kit data
  adds `token.keyword.forall`/`lex.keyword.forall`, `grammar.forall.prefix`,
  `ast.index_app` (callee+args roles — payload_extract/binding resolution
  work via `callee`), `eval.index_app` (= `eval.call` scheme),
  `type.index_app` (= `synth_call`). Blocker: sibling `engine` repo has
  uncommitted WS-E WIP (`compose_cell_vals`/`is_cell_val` — nbe.rs fails
  to compile), so `hnl-engine-typecheck-nbe` lib check +
  `dependent_structure` + `hnl typecheck`/`hnl run` e2e verification are
  pending on that branch finishing (dependent.rs itself: zero errors,
  warnings only; engine-parse-pratt 57/57 green).

- **WS-C wave-2 (e2e verify — deferred from wave-1, now green).** The
  wave-1 machinery was committed unexercised (`engine` was mid-edit);
  first `hnl typecheck`/`hnl run` of the sugar sketch surfaced **six
  real defects**, all fixed and committed (no push):
  1. `synth_assign` `function`-lhs and `dim_arrow`/`axis_flow`/`flow`
     branches resolved the bound name through `node_name` only — an
     `index_app` subject (`id_vec{n,a} : Vec{n,a} ~> _`) fell through
     to "annot var". Both branches now try `implicit_binder_head`
     first (`engine-typecheck-nbe dc604d8`).
  2. `synth_call` drained implicit `type.pi`s with fresh metas for
     *every* argument — `head{2,Bool}` checked `2` against
     `Vec{Succ ?n, ?a}` ("function is not a family member" → the
     number-vs-family_app unify error). `index_app` arguments now
     *fill* the next implicit pi as index terms; exhausted pis fall
     through to the ordinary domain check (`dc604d8`).
  3. `grammar.forall.prefix` parsed its body at rbp 0, capturing a
     trailing `:=`/`=` inside the forall — the sugar could never
     annotate a definition. Body now parses at rbp 6
     (`child_binding_powers`, the `data` convention) — assign stays
     outside, `~>`/`:` still bind inside (`kit-prelude-lang ac4ed55`).
  4. `eval.index_app` applied brace args as call arguments —
     `head{2,Bool}` bound `v := 2` and the match failed on `Int(2)`.
     Implicit index args have no runtime counterpart: the scheme now
     projects the callee only (same erasure class as
     `eval.type-annot`) — implicit args fully erased at runtime
     (`ac4ed55`).
  5. The bootstrap scope pass bound `f` but not the `{n,a}` binder
     names — `vec_map`'s own `a`/`b` reported as unbound wires.
     `implicit_binder_names` descends the binding target through
     `subject`/`params`/`source` to the `index_app` head and scopes
     the binder vars over the value only (`engine-typecheck 35bf9cb`).
  6. `synth_assign` bound the name *after* checking the value, so
     annotated recursion (`vec_map` calling itself) was unbound. The
     name is now Γ-visible at the annotation's type before the value
     checks — matching the scope pass's documented self-recursive
     semantics (`dc604d8`).
  *Verdict*: the sketch typechecks and runs end-to-end —
  `data Nat`/`data Vec(n : Nat, a : Type)` indexed families;
  `head{n,a} : Vec{Succ n, a} ~> a` (implicit binders + ctor-app
  index term) with a motive-inferred `match` (no `return` clause);
  `id_vec{n,a}`; `forall n : Nat, Vec(n+0,Bool) ~> Vec(n+0,Bool)` —
  `forall_demo(Succ(Zero))(VNil)` is correctly **rejected** (index
  `0` vs `Succ(Zero)` — a real dependent length check), the
  `VCons` tail accepted; motive-inferred `match` on `Nat`
  (`is_zero` runs `true`/`false`). Headline: **`vec_map{n,a,b}`
  typechecks and runs** — recursive calls `vec_map{k,a,b}(f, xs)`
  inside the `match` arm resolve through the implicit pis; `hnl run`
  returns the mapped `VCons` structure with all index args erased.
  Notes: `forall` is the *explicit* `Pi` alias — `x : forall n, T :=
  n ~> (v ~> v)`; a bare index name in type position still reads as
  its sort (`Vec(n,a)` → `Nat`), so length-indexed types write
  `Vec(n + 0, a)` per the dependent.hnl convention; `data Nat`
  ctor args need `Succ`-numerals (literal `1 : Nat` ≠ the inductive
  family — `Succ(3)` still rejected by design). Verify:
  `engine-typecheck-nbe` lib 285 + dependent_structure 24/24 +
  parity 46/46 green; `engine-typecheck` 15+43 green;
  `ext-typecheck`/`ext-eval` `.so`s rebuilt.

- **WS-D wave-2 (index-solver hardening) — landed.** `eval_index` now
  canonicalizes `Bin`/`Un` `add`/`sub`/`mul`/`neg` index terms into a
  bounded **polynomial normal form**: sorted monomials keyed by canonical
  atom string (`var` by name, metas by id, `App`/opaque `div`/`mod`/
  `pow`/`eq` nodes by serialized term), like terms collected
  (`a+b+a → a*2+b`), constants pooled (`(n+2)+3 → n+5`), products
  distributed (`n*(m+k) → n*m+n*k`), all under `POLY_*` caps —
  over-bounded products keep the factored `canon_bin` shape rather than
  expanding. Because normalization runs inside `eval_index`, every
  consumer (`unify_index`, `dep_unify` family_app/`Path` arms, match
  refinement, `closed_num`, `eval_ann_term`) sees normal forms **before**
  meta-solving: `Vec(n+m) ≡ Vec(m+n)`, `Vec(2*n) ≡ Vec(n*2)`,
  `Vec(n+1+0) ≡ Vec(n+1)`, `n+0/0+n/n*1/1*n/n*0` identities — all hold
  definitionally, including under rigid Pi binders. `unify_index`'s
  numeric tail now routes through a **`trait IndexSolver`**
  (`prove(a,b) -> Proven|Disproven|Unknown`, `BuiltinIndexSolver` impl +
  `static INDEX_SOLVER`) — the documented seam a Z3/CVC-class SMT
  backend slots behind later with no call-site changes. Decided-vs-
  stuck contract kept: distinct numerals / non-divisible products /
  absent roots → Disproven (error); two symbolic factors (`?n*?m`)
  → Unknown (stuck), never silent-accept. Bonus: distribution makes
  `c*(X+d)`/`(X+d)*c` shapes linear-solvable where they were stuck.
  Notes: `Vec(n+1+0) ≡ Vec(Succ n)` literally means `n+1` vs the
  `App{Succ}` index term — still stuck by design (the `Succ p ≡ k+1`
  bridge is the ctor-result instantiation `?k+1`, not `Succ`→`+1`
  rewriting); kernel `nbe::conv` on `Val::Const` index objects is
  structural — a `Val`-domain port is the documented follow-up if
  needed. New tests: `dependent::index_solver_tests` 5/5. Verify: lib
  **290**/290 (285 + 5), dependent_structure **24/24**, parity **46/46**
  — all green single-threaded. Commit: `engine-typecheck-nbe fcb1134`.
  Collision note: concurrent WS-F/WS-H agents were editing this repo
  mid-run — a mid-session file overwrite wiped the first patch;
  reapplied on top of their WIP and staged only WS-D hunks (`git apply
  --cached`). A 1-char borrow fix inside the in-flight `handle_sig_check`
  block (`self.zonk` signature) was needed to compile; left in the
  worktree for the WS-F agent's commit.

- **WS-C wave-3 (kit-prelude sweep reds — `ordered_annotations` 2/2
  green).** Two `kit-prelude` `ordered_annotations` fails surfaced in
  the post-sweep retest, both NBE-only gaps (the deleted `.hnl` model
  covered them; the suite moved to `typecheck_source` in WS-B):
  1. `generic_dim_arrow_binds` (`dedupe<N> := (g >>> g) ~[N]~> g`) —
     the `dimension` lhs shape (`name<N>` generic dim params,
     ADR-0106 §12) hit `synth_assign`'s bare-lhs `node_name` fallback
     ("synth_assign: var name"). `implicit_binder_head` now accepts
     `dimension` heads (same `[target, args…]` shape as `index_app`),
     the bare-lhs arm tries it before `node_name`, and generic
     binders on an *unannotated* assign scope rigidly (`var{N}` in
     results + Γ) over the initializer — the scope pass's
     `dim_param_names` semantics. `synth_dim_arrow` endpoints are
     quoted law terms: free vars (`g`) bind fresh metas under a
     scoped `law_mark` (pattern mode, ADR-0119), and a scope-bound
     dim var elaborates as its rigid index term instead of a fresh
     `Dim` meta (same in `dep_synth_type`'s dim_arrow arm)
     (`engine-typecheck-nbe 819b593`).
  2. `ordered_value_annotations` (`x : 5 >>> inc := 6`) — bf4affd's
     unruled-kind fallback made `dep_synth_type` succeed with
     `type.unknown` on a `flow` annotation, so the ADR-0103
     term-eval fallback never ran and the value's `type.number`
     unified against `type.unknown`. A `type.unknown` annotation
     result now takes the term-eval path first (`5 >>> inc` → `6`),
     keeping the placeholder only when eval also fails (`819b593`).
  Verify: `kit-prelude` `ordered_annotations` 2/2, `fp_integration`
  37/37, `sugars` 14/14; `engine-typecheck-nbe` lib 285/285 +
  dependent_structure 24/24 + parity 46/46 — all green.
