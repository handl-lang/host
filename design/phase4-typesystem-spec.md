# Phase-4 implementation spec — unified type system

Normative design: ADR-0106 (kernel, as amended) + 0152 (`data`) +
0153 (`~[n]~>` dimension index) + 0154 (identity/equality/universes/
literals/erasure) — same as phases 1–3 — plus the phase-3 resolutions:
instance coherence is **declaration-time no-overlap** (decision 1a,
ordered first-match is the deferred alternative); ctor index
projections are **`data`-declared metadata** (decision 3b); coverage is
**bounded-split + "coverage unknown" warning** (decision 4b); `x ~>
{…}` disambiguates **by brace shape** (decision 5a); `Hom(n)` checking
is **generic** (decision 6 superseded by user direction). The index
surface is now `<…>` — `Vec<n, a>`, `f<a, ev : Eq(a)>`, `IO<{ puts }>`
— the `{…}` index-app spelling is deleted. This spec scopes what
remains: the existential surface, open effect rows, a solver backend,
compiled-backend parity gaps, and the polish round. Waves of ≤4
parallel workstreams. New semantics that settle here get their own
ADRs; several items are parked behind an explicit **needs user
decision** section below.

## Phase-3 end state (what exists)

NbE checker over `Val`; `data` families + `F.rec` (checker-visible via
the `<F>.rec` Γ binding) + dependent `match` with Kőszegi split-tree
coverage — decided-impossible arms auto-discharge, `absurd` stays
legal documentation; generic `Hom(n)`/`>>>[k]`/`id_k` checked at every
literal `n`, `CellNf` comp-tree normalization in `conv` (assoc, unit
absorption, interchange on uniform grids — all definitional);
implicit `<…>` binders with two erasure classes (indices erase,
evidence materializes as a runtime arg); `instance` decls +
coherence-first dictionary search (`synth_instance`, dependent.rs
~L4166+, overlap rejected at declaration); `IO(sig)` typed effects —
`synth_handle` residual `IO(S\H) × results` (dependent.rs
~L6386-6530), closed-sig row-metas, `handle` dynamic-extent install on
all three exec engines (shared `handle_dyn` op, engine-exec
native_ops.rs ~L3603, + `handler_push/pop/lookup` on `NativeOpCtx`);
`Succ`-shape ctor index projections declared on `data` (`Vec(Succ n)
≡ Vec(n+1)` both directions); `eval.checked`/`reify_ast`/`quote_ast`
metaprogramming; `export` binds + `hnl run --name`; explicit `Sigma`
encoding verified end-to-end (`filter` in lib-collections; `exists`
sugar deferred to BACKLOG); spanned diagnostics + expected-vs-actual
`display_type` through `plugin-ext-diag`; `hnl lsp` v1+v2 (checker
diagnostics, zonked hover, scope-aware completion/references/rename,
`KitResolver`); stdlib `__tag__`/`tag.*` consumer migration round 1
(`match`-based ctor-sym dispatch covers both mint shapes); the hnl
smoke suite in CI. Verified at phase-3 close: lib 302 +
dependent_structure 51 + parity 46 + smoke stage green.

## Workstream map

| WS | Scope | Repo(s) | Blocks |
|---|---|---|---|
| A | `exists` sugar → `Sigma` (parse, elab, destructure, erasure) | `engine-parse-pratt`, `kit-prelude-lang`, `engine-transform-rewrite`, `engine-typecheck-nbe`, `lib-desugar` | — |
| B | SMT backend behind the `IndexSolver` seam | `engine-typecheck-nbe`, `host` (driver/env wiring) | — |
| C | Compiled-backend parity — self-recursive closures + cranelift `Term::Child` | `handl-kit-arrow-lower`, `engine-exec`, `engine-exec-bytecode`, `engine-exec-cranelift`, `host` (`handl-runtime`) | — |
| D | Open effect rows — `IO<{ puts \| r }>` row-var machinery | `engine-typecheck-nbe`, `kit-prelude-lang`, `engine-parse-pratt` | E (shares `synth_handle`/`dependent.rs`) |
| E | Handler-accumulator `handle … acc` (State design B) | `engine-typecheck-nbe`, `engine-exec`, `engine-exec-*`, `kit-prelude-lang` | needs D landed (same file) |
| F | stdlib migration round 2 — `op`/`cap` decl disposition + remaining lib-* dynamic shapes | `plugin-lib-handl`, `lib-*`, `engine-typecheck-nbe` (if decls become checked) | decision 7 |
| G | LSP v3 — workspace index, UTF-16 positions, completion/rename polish | `host-lsp` | — |
| H | Error-quality pass — unification chains, related spans, zonk hygiene | `engine-typecheck-nbe`, `plugin-ext-diag`, `host-lsp` | — (late) |
| I | Quotient/HITs — design note only | `host/project` | — |
| J | `intl_c` env fix + `lib-intl-c` conformance adapters | `kit-prelude`, `lib-intl-c` | — |

Waves: **1: A ‖ B ‖ C** → **2: D ‖ F ‖ G ‖ J** → **3: E ‖ H ‖ I**
(≤4 concurrent). D and E share the `synth_handle`/`dependent.rs` seam —
same rule as phase-3 C/D: sequence their landings to avoid the
mid-session overwrite collisions. H slots late so its rendering work
prints whatever A/D landed (open-row and `exists` spellings in
`display_type`).

## A — `exists` sugar → `Sigma`

The phase-3 design call was explicit `Sigma`, sugar deferred to
BACKLOG — this is the sugar done for real, including the destructure
half the deferral noted.

- **Problem**: no existential index syntax — `?m` fails to lex
  (phase-3 dogfood), so `filter`-style return types write
  `Sigma(Nat, m ~> Vec<m, a>)` directly. Two real gaps under it:
  (1) the spelling itself; (2) a Sigma-destructured `m` reads as the
  `Nat` *sort* inside `Vec(m, a)` — bare names resolve through Γ to
  their sort, only `m + 0`-shaped compounds stay `var` — so a
  computing `filter` still can't rebuild its result type. Sugar
  without (2) is cosmetic.
- **Surface** (decision 1 for the binder sort):

  ```hnl
  filter<n, a> : (a ~> Bool) ~> Vec<n, a> ~> exists m. Vec<m, a>
  -- elaborates to Sigma(Nat, m ~> Vec<m, a>)

  find<n, a> : (a ~> Bool) ~> Vec<n, a> ~> exists m : Nat. Vec<m, a>
  -- explicit sort form, if kept
  ```

- **Elaboration**: `exists` prefix/prefix-keyword form parses to a
  binder node; `dep_synth_type`/`node_to_ann_term` map it to
  `type.sigma{param, body}` — the same `Val` `dep_synth_call_type`
  already mints for `Sigma(A, F)` (dependent.rs ~L2658 `Sigma →
  type.sigma`). No new `Val` variant, no new former — `exists` is a
  spelling.
- **Destructure**: Sigma intro is the `[witness, value]` pair
  (`dep_intro_sigma`, dependent.rs ~L4914 — componentwise check).
  Elimination options (decision 2): `match r { [m, v]: … }` pair
  pattern binding `m` as an *index var* (the fix is the session-5
  `__fam_<name>` trick again — the Sigma-bound name must bind
  `var{m}` in Γ/`results`, not its sort), `.0`/`.1` projections, or
  both. Without it the pair is opaque to `Vec(m, a)` in the arm.
- **Erasure**: `exists` in annotation position is ordinary
  type-level content — erased like `Sigma`/index binders (0154 §6
  artifact lint unchanged). The *pair value* is runtime data —
  nothing to erase.
- **Display**: `display_type` prints `type.sigma` as
  `exists m. Vec<m, a>` when the param sort is `Nat` (and
  `exists m : S. T` otherwise) — round-trips the sugar.
- Scope: parser prefix row + `token.exists`/`AstShapeDef` roles in
  kit-prelude-lang; `dep_synth_type`/`node_to_ann_term`/`display_type`
  arms; match pair-pattern binder indexing; `lib-desugar` overlay
  parity.
- Exit: `filter<n, a>` above typechecks *and* the destructured `m`
  indexes `Vec(m, a)` in the arm (the recomputing-`filter` case the
  phase-3 log couldn't write); `exists` renders in expected-vs-actual
  output; dependent_structure/parity/smoke green.
- Est. **M**.

## B — SMT backend behind `IndexSolver`

- **Problem**: `BuiltinIndexSolver` (dependent.rs ~L1424) covers ring
  normalization + the linear/division/root fragment + nonnegativity —
  enough for `Succ` projections and coverage splits — but anything
  outside the linear fragment stays `Unknown`: nonlinear products
  (`?n * ?m`), modulo/gcd shapes, disjunctive bounds. `Unknown` is
  honest, not wrong — the SMT backend widens the *decidable*
  fragment, never the accepted one.
- **The seam already exists**: `trait IndexSolver` (dependent.rs
  ~L1416) + the `INDEX_SOLVER` static (~L1460) — "swap this static
  for an SMT-backed `IndexSolver`" is written into the seam; call
  sites (`unify_index`, `prove` callers) don't change.
- **Design**: `SmtIndexSolver` encodes index terms to SMT-LIB2 `Int`
  — `var{x}` → `declare-const`, `Bin{add,mul,sub,div}` → `(+ - * div)`,
  numerals → literals; nonnegativity facts (`ctx.nonneg_index_vars`/
  `nonneg_metas`) become `assert (>= x 0)` premises. `a ≡ b` is a
  validity query: `assert (distinct a b)` + `check-sat` → `unsat` =
  `Proven`, `sat` = `Disproven` *only when both sides are ground*
  (a model against open terms is a counterexample to the rigid goal —
  still `Disproven` for ground rigids; metas are peeled by the
  builtin front-end first). Timeout/parse-error/absent binary →
  `Unknown` — never worse than today.
- **Meta boundary (hard rule)**: the `extract_meta_id` solve arms stay
  in the builtin — an SMT model yields numerals, not open terms; meta
  instantiation is unification's job. SMT fires on meta-free goals.
- **Driver**: subprocess, not linked — `HNL_INDEX_SOLVER={builtin,z3,
  cvc5}` + `HNL_SMT_BIN` + a wall-clock timeout (the bounded/fuel
  contract applies to wall time too). Per-query spawn vs persistent
  `check-sat` session is an implementation detail — measure; the
  suite is parse-bound anyway (WS-H perf notes).
- **Determinism**: solver version pinned in CI (script-provided
  binary); a version drift changing `Unknown`↔decided flips must be
  loud — smoke files asserting an `Unknown`-boundary case guard it.
- Kernel implications: **none** — the trait impl lives in
  `engine-typecheck-nbe`; the index-term vocabulary (`add`/`mul`/…)
  is the checker's own index algebra, calculus-adjacent — fixed in
  Rust is allowed, same class as `poly_of`.
- Exit: `HNL_INDEX_SOLVER=z3 hnl typecheck` decides a goal the builtin
  reports `Unknown` on (e.g. a nonlinear index identity with ground
  rigids); absent binary degrades every query to `Unknown` and the
  suites stay green; determinism guard test lands.
- Est. **M**. Decision 5 (which solver is the blessed binary) gates
  CI wiring, not the seam.

## C — Compiled-backend parity: self-recursive closures + cranelift `Term::Child`

Two related honest-bound gaps, both in the compiled-engine lowering
path — one file, one audit.

- **Self-recursive closures**: `decline_self_reference`
  (`handl-kit-arrow-lower` lib.rs ~L1461) declines `var f` in its own
  RHS — "the arrow calculus has no recursion" — and the engines fall
  back to AST eval. The fallback computes correctly on tree-walk
  (`self_recursive_arrow_evals_via_ast_fallback`,
  `handl-runtime` engine_parity.rs ~L753) but **bytecode/cranelift
  still diverge** — `vsum`/`vmap`/`len` produce wrong values there.
  Options (decision 6):
  (a) `with_self_ref_bind` on the compiled closure path — the
      mechanism already exists (`handl_value::with_self_ref_bind`,
      used by the `export` op at native_ops.rs ~L817/888/2787): bake
      the name into the bound arrow's `binds` and stop declining;
  (b) port the `eval_rec` region semantics (native_ops.rs ~L1028,
      ADR-0074) to the compiled engines;
  (c) a surface `rec`/`fix` desugar — `fix_step` machinery already
      exists among the derived step ops (native_ops.rs ~L3636), so
      this is a grammar + desugar-rule surface decision, not new
      runtime machinery.
  (a) is transparent — self-recursion just works, `decline` shrinks;
  (c) makes recursion opt-in syntax. Honest-bound alternative if
  neither lands: make the decline *loud at run* on compiled engines
  (today it silently diverges — the worst shape).
- **Cranelift `Term::Child`**: `engine-exec-cranelift`
  compile.rs ~L1159 — `"Child" => Err(NotImplemented)` (alongside
  `Iter`, `Path`, `Equiv`). Bytecode solved the same shape:
  `EvalChild` via `lang.child_role_index_for` + a per-node compile
  cache resolving the child's `AstRef` (engine-exec-bytecode
  compile.rs ~L379-395, instr.rs ~L160). Cranelift's compile arm
  lacks the `lang`/`node` context — the fix ports the bytecode
  pattern: a `helper_v2_eval_child` calling back into a
  compiled-children table keyed by `AstRef`, with the compile-time
  role resolution moved into the artifact's lang handle.
  Currently reachable only via *named*-target `Handle` elimination
  (surface `.hnl` never produces it — `eval.handle` fixes
  `target: ""`), so the honest bound holds; this closes the last
  `NotImplemented` exec gap for a real term. `Iter`/`Path`/`Equiv`
  stay or get deleted per their fold status (`Path`/`Equiv` are dead
  arms post-ADR-0119 — delete with this pass or leave documented).
- Scope: `handl-kit-arrow-lower` (`decline_self_reference`), the
  three `engine-exec-*` self-call/closure paths, `with_self_ref_bind`
  reachability on compiled engines, cranelift compile.rs +
  `lib.rs` helpers; parity tests in `handl-runtime`.
- Exit: `vsum`/`vmap`/`len` evaluate identically on all three engines
  (the parity test flips from tree-walk-only to all-engines, or the
  divergence is a loud error — decision 6 picks); `Term::Child`
  compiles on cranelift or the arm is removed with the bound
  restated; conformance suites green.
- Est. **L**.

## D — Open effect rows — `IO<{ puts | r }>`

- **Problem**: row-metas bind *whole* rows only — a symbolic sig
  `IO(s)` hits `synth_handle`'s undecidable-passthrough arm
  (dependent.rs ~L6487: "coverage is undecidable at this use"). There
  is no surface for "this function performs `puts` **and whatever the
  caller's row has**" — effect forwarding is unwritable. Example
  today you can't express:

  ```hnl
  instrumented<r> : (Unit ~> IO<r> Unit) ~> IO<{ puts: Str | r }> Unit
  -- "runs f, logs through puts, forwards the caller's other effects"
  ```

- **Surface** (decision 3 — `|` inside braces collides with
  `token.pipe`, the `data` ctor-alternative infix at rbp ~0,
  grammar.yaml ~L165): (a) `{ puts: Str | r }` — needs the
  `brace_is_record` shape test extended to read a `| tail` slot
  (engine-generic — a `tail` role on the record AstShapeDef, not a
  kind check); (b) `{ puts: Str, ..r }` spread tail; (c) former
  spelling `IO<Row<{ puts: Str }, r>>` (zero new syntax, worst
  ergonomics).
- **Row binder** (decision 4): `r` needs a sort — (a) explicit
  `f<r : Sig>` sorted implicit binder (the `Sig` sort already exists
  — dependent.rs ~L2578 "the `IO(sig)` signature-record sort"), the
  evidence-vs-index classification unchanged; (b) implicitly
  quantified free row vars — smaller surface, worse errors.
- **Checker** — this is the genuinely-hard item (Rémy-style rows):
  - *Encoding*: sig records gain an optional tail — e.g.
    `{"fields": {…}, "tail": var{r}}` — inside the existing
    `type.record` value shape; closed records = absent `tail`.
    `IO`'s former arg reads the same `Val` either way.
  - *Unification*: `{a:T | r} ≡ {b:U | s}` — shared fields unify
    pointwise; a field present on exactly one side folds into the
    *other* side's tail via a fresh tail-meta (`r := {b:U | r'}`);
    field-on-closed-side-missing → decided failure; same-name
    different-type → decided failure; identical tails → equal.
    Tail-metas need occurs-check + scope tracking in the existing
    meta store — `solve_meta` gains a row-sorted solution check.
  - *`handle_sig_check` residual*: `IO<{puts | r}>` handled by
    `{puts: …}` → `IO<r>` — the covered entry leaves the record *and*
    the tail. **Spurious-impl rule change**: the "handler declares
    ops `S` never names" error (dependent.rs ~L6506) can only fire on
    a *closed* sig — on an open sig the impl might cover a tail op;
    undecidable → skip the check (honest bound, consistent with the
    symbolic-sig passthrough).
  - *Width rule*: `IO<{puts | r}>` used where `IO<{puts, fs}>` is
    required solves `r := {fs}` (closed-row instance of the tail
    meta). `IO<{puts}>` where `IO<{puts | r}>` required → `r := {}`.
    The record-width subtyping semantics (narrowing rejected, smoke
    `effects_err.hnl`) is unchanged for *closed* sigs.
- Erasure: rows are type-level — erased with the rest of the sig.
  Runtime `interpret_io_former` reads the sig's *fields*; a tail on a
  produced `IO` object is inert data (or absent — produced `IO`s are
  closed records anyway; verify and pin).
- Exit: `instrumented<r : Sig>` above typechecks; `handle` over an
  open sig produces the tail as the residual; `IO<{puts | r}> ≡
  IO<{puts, fs}>` solves `r := {fs}`; open-vs-closed decided failure
  still rejects (narrowing); the spurious-impl rule stays on closed
  sigs; dependent_structure/parity/smoke green.
- Est. **L** — the row-unification half is the real cost; the syntax
  is small whichever spelling lands.

## E — Handler-accumulator — `handle … acc` (State design B)

- **Problem** (dogfood2 verdict, phase-3 log): `interpret_io_former`
  dispatches each sig entry independently — `handler_lookup` per
  field, result replaces the entry (native_ops.rs ~L3575-3580). There
  is **no carrier for a state thread**, so `handle` cannot express
  `runState`. Option A's record-fold is sound only as an op-*summary*
  (`get`→s0, `put`→final s, ≤1 entry/op, record order); real
  sequencing needs the free-monad chain encoding, which the dogfood
  verified working (`runState_c` folds `get→put→get→ret` → `(6,6)`).
- **Design B** — `handle <body> with <h> acc <init>`:

  ```hnl
  handle m with {
    get : (p, s) ~> [s, s],      -- payload × acc ~> [result, new acc]
    put : (p, s) ~> [unit, p],
  } acc 0
  -- each covered sig entry interprets as (payload, acc) ~> [res, acc'];
  -- uncovered entries pass the acc through unchanged;
  -- result is [residual-or-value, final_acc]
  ```

- **Checker**: `synth_handle` variant — `acc` is a new optional role
  on the `handle` AstShapeDef; `h`'s fields must check as
  `(S[k] × Acc) ~> (Res_k × Acc)`; residual `IO(S')` plus the acc —
  result type `[A', Acc]`-pair (shape under decision 8). The
  closed-vs-open sig rules from D apply unchanged (acc threads only
  through *covered* entries).
- **Runtime**: a `handle_acc` op or an `acc`-flagged `handle_dyn` —
  same push/eval/interpret/pop shell, but `interpret_io_former`'s
  per-entry dispatch becomes a fold: `(payload, acc)` in,
  `[res, acc']` out, `out_sig[k] := res`, `acc := acc'`. Three-engine
  parity via the shared native-op (the WS-E phase-3 port pattern:
  `OpService` + `NativeOpCtx` impl per engine is already there).
  Deep-handler case: acc + `attach_carried_handler` composition needs
  a stated rule — acc lives for the dynamic extent, not the carried
  one (decision 8 side).
- **Gate**: the dogfood already answered "is the record-fold
  sufficient" — no, for sequencing; but the *chained* encoding works.
  Decision 8 is go/no-go: build `acc` machinery vs bless the
  free-monad chain as the idiom (document it in the tutorial) and
  spend the budget elsewhere.
- Exit (if built): `handle` + `acc` runs `runState` natively on all
  three engines — `put` accumulates, `get` reads current; residual
  `IO` keeps uncovered ops with acc untouched; checker rejects an
  impl whose `acc` type changes mid-handler.
- Est. **M** (mechanism well-understood — it's a fold over the
  existing dispatch), gated on decision 8.

## F — stdlib migration round 2 — remaining `lib-*` dynamic shapes

- **Problem** (phase-3 migration table's NOT-migrated row): `lib-fs`,
  `lib-event`, `lib-serve`, `lib-kv*`, `lib-fetch` contract files
  carry `op "x" : Sig`/`cap "…"`/`handle "op" with <impl>` decl
  tails that are **inert text** — `strip_declaration_lines`
  (plugin-lib-handl lib.rs ~L490) removes `cap`/`op`/`handl`/
  string-target-`handle` lines plus `guarantee`/`requires`/
  `termination` continuations before any type pass. Verified: `op
  "demo.op" : String -> GARBAGE(((` typechecks clean. `IO(sig)`
  doesn't fit — it models a *suspended* computation; these ops are
  eager FFI calls, and writing `~> IO<{…}>` codomains would
  misrepresent the contract. Contract `export X := A ~[1]~> B` lines
  are already correct (dimension index, post-0153).
- **Disposition decision** (decision 7 — this is the `extern`-shaped
  question, see Rejected/deferred):
  (a) inert + documented — the tails stay a *convention*, the gate
      never reads them;
  (b) checked decls — a minimal `stmt.op` judgment records
      `op "x" : Sig` into `Sig_m` (no runtime semantics — a contract
      linter then diffs declared sigs against exported impls); the
      loader stops stripping them;
  (c) delete the tails — move sigs onto `export x : Sig = impl`
      annotations and drop the decl surface entirely.
- **Then migrate**: the five lib families' remaining `tag.*`/dynamic
  reads (mostly covered by round-1 `match` dispatch — verify) and
  whatever the decl decision produces. `{"type":"codec"/"parser"/
  "serializer"}` descriptors and `{name: op}` vocabs stay — tag-is-
  data, round-1 verdict stands. Blocking tooling gap to fix first:
  `hnl test` can't desugar `data` and `export` inside `do {}` is an
  unbound wire — migrated libs need both to be testable.
- Scope: `plugin-lib-handl` (strip/match decl handling per the
  decision), `engine-typecheck-nbe` (a `stmt.op` judgment only under
  7b), the five lib families, `engine-test`/`host-cli` data-desugar
  gap.
- Exit: the disposition lands; the five families carry whatever the
  decision prescribes — and either way, **a garbage sig stops
  typechecking clean** or the inert convention is written into the
  docs where users can see it.
- Est. **M**.

## G — LSP v3

`hnl lsp` v1+v2 landed in phase 3 (diagnostics through the checker
pipeline, zonked hover, scope-aware completion/references/rename via
the `binding` provider, per-workspace `KitResolver` wired into
handlers — kit_resolver.rs, `kit_for_file` in hover.rs/symbols.rs/
type_definition.rs/…). v3 is the polish + workspace layer:

- **Workspace indexing**: today the binding/scope index is
  per-document — goto-def/references/rename stop at the file edge.
  Build the workspace index: file graph off `.handl.toml`
  `[dependencies]` + `from "…" import` resolution (ADR-0095 path
  semantics), cached per `KitResolver` workspace root; cross-file
  defs land through the same `binding` provider.
- **UTF-16-exact positions**: `line_map.rs` (~L35) counts UTF-8 code
  points for `character` — LSP positions are UTF-16 units; every
  position is off on non-BMP lines (and wrong-ish on astral
  punctuation in strings/comments). Convert the map; conformance
  tests pin a multi-plane fixture.
- **Completion/rename polish**: keyword + former docs on hover
  (`data`/`match`/`IO`/`exists` once A lands), record-field hover,
  signature-help coverage, rename guards — ctor names are sym
  identity, not scope bindings (renaming `VCons` the pattern must hit
  the `data` decl and all uses, not the shadowed var).
- Exit: cross-file goto-def/rename works across an import edge; a
  UTF-16 conformance case passes; keyword hover docs land.
- Est. **M**.

## H — Error-quality pass

Phase-3 WS-G landed spans + expected-vs-actual; this is the next
layer — *why*, not just *what*:

- **Unification chains**: `cannot unify — expected T, got U` reports
  the failing node's span but not the obligation chain — "expected
  because the annotation at `f.hnl:3:7` says `Vec(3, Bool)`;
  instantiated at the call `g(x)` at `f.hnl:9:2`; match arm refines
  `n ↦ Succ k`". `plugin-ext-diag`'s `Diagnostic` (~L128) has
  `span`/`fix` but no related-spans field — add
  `related: Vec<(Span, String)>` (labels at secondary spans) and
  thread the chain through `unify`/`dep_unify`/`handle_sig_check`
  failures.
- **Zonk/display hygiene**: phase-3 noted meta-name collisions in
  `display_type` rendering both sides identically; `?m` numbering in
  errors should be per-diagnostic fresh (renumber for the message,
  not the store id). `display_type` grows arms for whatever A
  (`exists`) and D (open rows) landed — spellings must round-trip.
- **Multi-error surfacing**: `diagnose_with_prelude` keeps
  diagnostics in-band (v2); audit that unification chains render
  through the LSP path (related spans → `relatedInformation`).
- Exit: a two-hop failure prints the chain — `cannot unify — expected
  Vec(3, Bool), got Vec(1 + 1, Bool)` *plus* a "required by
  annotation at …" related span, through `hnl typecheck` and the LSP;
  meta names in messages never collide within one diagnostic.
- Est. **M**.

## I — Quotient/HITs — design note only

Phase-3 WS-B recorded the verdict and deferred the write-up — file it
this phase:

- **The question** (restated): can a `data` ctor target
  `Hom(k≥1,·,·)`/`Path(a,b)` — a higher-inductive constructor? The
  former machinery admits the *declaration*; `F.rec` computes on
  0-dim ctor values only — a path ctor needs the eliminator to
  specify its action on paths (an ap/map clause), which the fold
  machine has no rule for.
- **Deliverable**: a `design/` note (or parked ADR) sketching the
  two options — (a) a new eliminator clause: `F.rec` gains a
  per-path-ctor action argument (computes `F.rec` on 1-cell values —
  needs `CellNf` to expose a transport the clause can consume);
  (b) quotient equations stay propositional: `Path`-valued ctors are
  data, the action is a `J`-derived eliminator, nothing is
  definitional. The note should also record what `CellNf`'s stuck
  shapes imply for (a)'s feasibility (non-uniform grids can't
  transport definitionally — a quotient over a stuck grid is
  propositional-only by construction).
- **Not a workstream**: no implementation follows from the note in
  this phase — it's the input to a phase-5 ADR if the verdict is "go".
- Est. **S**.

## J — `intl_c` env + `lib-intl-c` conformance adapters

- **`intl_c` env issue**: kit-prelude's `intl_c` suite fails 13/13
  on this box — `from "../../../lib-intl-c/src" import *` resolves to
  `null` (module eval yields `field_access` on null). This is *not*
  the missing-ICU path — the test doc (tests/intl_c.rs header) says
  the module still loads without ICU4C and ops return `err
  "icu-unavailable"`; the failure is `resolve_import` not finding the
  module at all — relative-path resolution vs test cwd/sibling
  checkout layout. Diagnose and fix or env-gate honestly (a skip
  needs a reason string naming the env requirement, per the
  silent-green-skip rule).
- **Conformance adapters**: `lib-intl-c/src/conformance/
  generate.mjs` template was updated in round-1 migration; the
  remaining work is running the generated `.hnl` adapters against the
  CLDR conformance data — env-gated on ICU (native absent on this
  box); wire the gate so CI-without-ICU reports skipped-not-failed.
- Exit: `intl_c` suite either green or honestly env-skipped with the
  env requirement named; conformance generation runs (or is
  documented blocked on ICU) and the generated adapters parse clean.
- Est. **S–M**.

## Cross-cutting invariants (unchanged)

- One value space: checker and exec share `Val`; arrow *values* stay
  self-contained serializable data (`body_def`+`binds`) —
  defunctionalized data, closures only as domain machinery (0106).
- Types-as-values; nominal heads + structural bodies (0154 §3).
  Evidence args are runtime arguments; index args erase — the
  implicit-binder slot's two erasure classes stay classified by
  declared domain, and `exists`/rows add no third class.
- **Dimension ≠ mode ≠ effect axes** (0153): row tails enter the
  *sig-record* slot of `IO`, not the `~[n]~>` bracket; the acc role
  on `handle` is a statement role, not a dimension.
- Honest bounds stay honest: SMT timeout → `Unknown`, open-sig
  spurious-impl checks skip rather than guess, undecidable coverage
  warns — never a silent accept.
- Engine-genericity: new surface forms get `AstShapeDef` roles
  (`exists`, row `tail`, `handle`'s `acc`) — no `kind ==` reads; the
  SMT encoder's index vocabulary is calculus-adjacent (allowed fixed
  in Rust); `decline_self_reference`'s mechanism choice must keep
  `var`-name counting role-driven.
- `Sig`-sorted metas are a new meta sort — solution checks in the
  store, not ad-hoc at solve sites (same rule as `nonneg_metas`).

## Rejected / deferred

- **`extern` op decls — REJECTED.** The dogfood gap read "no
  `extern`/`builtin` effect-op decl form", but `handle` + `IO(sig)`
  already covers *effect* ops as data — what was actually missing is
  a machine-checked *contract* surface for eager FFI ops (`op "x" :
  Sig` tails are inert text today). That is WS-F decision 7 — a
  checked `stmt.op` is a declaration judgment, not an `extern`
  language form. **Revisit triggers**: (1) WS-F lands 7b and a
  producer-side `extern` twin is still missing; (2) the platform
  permission-manifest work (BACKLOG §security) needs per-op declared
  capabilities the `IO` sig can't carry; (3) an effect appears that
  can't be expressed as `IO(sig)` data (eager, non-suspendable,
  non-record-shaped).
- **`Type:Type` — rejected, permanently.** `Type(i) : Type(i+1)`
  landed (0154 §4); `Type:Type` is Girard-inconsistent and nothing in
  the surface needs it.
- **Ordered/first-match instance resolution — deferred.**
  Coherence-first (declaration-time no-overlap) landed (phase-3
  decision 1a). Revisit only if a real lib needs overlapping
  instances — the `store_mark`/`store_restore` machinery supports
  either policy.
- **Suspension / globular unit coherences — deferred.** Phase-3 WS-B
  stretch not landed; the flat `Hom(j,x,x)` `id`-tower absorption
  covers the unit rule; a suspension former stays out of the former
  set.
- **HIT/quotient implementation — deferred** behind the WS-I note;
  no eliminator clause exists and none is specced here.
- **Self-hosted tooling (RFC-0001..0007) — parked** per BACKLOG;
  blocked on compiled-backend perf, unchanged by this phase.
- **`let`/`stmt.let` surface — dead** per ADR-0079; `rec` (if
  decision 6c lands) is a new form, not a `let` revival.
- **`match_recursor` desugar parity — carried**, not a workstream
  (the runtime `try_tag_pattern` gap is covered; the lowering
  divergence between desugar's annotated-scrutinee + full-coverage
  requirement and the checker's relaxed rules remains).

## Needs user decision

| # | Decision | Options | Lands in |
|---|---|---|---|
| 1 | `exists` binder sort | (a) `exists m. T` defaults `m : Nat` (the index-sort convention); (b) require `exists m : S. T` | WS-A — (a) matches how `<n>` binders sort; (b) is uniform but heavier |
| 2 | Sigma destructure form | (a) `match r { [m, v]: … }` pair pattern, `m` bound as index var; (b) `.0`/`.1` projections; (c) both | WS-A — (a) is what makes the computing `filter` writable; the `m`-as-index fix rides whichever lands |
| 3 | Open-row tail spelling | (a) `{ puts: Str \| r }` — pipe tail inside record braces (`brace_is_record` shape-test extension, `tail` role); (b) `{ puts: Str, ..r }` spread tail; (c) `IO<Row<{puts:Str}, r>>` former form | WS-D — (a) collides with the `data`-alt pipe infix and needs the shape test; (b) is unambiguous; (c) needs zero syntax |
| 4 | Row-binder introduction | (a) explicit `f<r : Sig>` sorted implicit binder; (b) implicitly-quantified free row vars | WS-D — (a) reuses the `<…>` binder machinery and the existing `Sig` sort |
| 5 | SMT driver | (a) z3; (b) cvc5; (c) env-selected behind `HNL_INDEX_SOLVER`/`HNL_SMT_BIN` | WS-B — an external binary dep enters the toolchain; pick the blessed one for CI |
| 6 | Self-recursion mechanism | (a) transparent — `with_self_ref_bind` on the compiled closure path, `decline_self_reference` shrinks; (b) port `eval_rec` to compiled engines; (c) surface `rec`/`fix` desugar | WS-C — (a) reuses the landed `export` mechanism; (c) is a syntax decision (ADR-worthy) |
| 7 | `op`/`cap` decl disposition | (a) inert + documented convention; (b) checked `stmt.op` judgment recording sigs into `Sig_m` (loader stops stripping); (c) delete the tails, sigs move to `export x : Sig` annotations | WS-F — this *is* the `extern` question; see Rejected/deferred |
| 8 | `handle … acc` go/no-go | (a) build design B (`acc` role + fold-dispatch); (b) bless the free-monad chain encoding + tutorial docs | WS-E — dogfood2 verified the chain works; `acc` is real machinery |
| 9 | LSP workspace index scope | (a) `.handl.toml` `[dependencies]` graph only; (b) workspace-root directory scan | WS-G — (a) follows ADR-0095 resolution; (b) finds orphan files |

## Open items (tracked, not blocking)

- `type.unknown` consistent-equality — dogfood2's top-ranked gap:
  `+`/`if`/`&&`/`>=`/`==` on host-call results error instead of
  propagating unknown (parser-style code lives in `while` bodies for
  exactly this reason). A declared-`Sig` floor for the ambient
  prelude is the alternative shape.
- `while` bodies are a checking black hole — the escape hatch that
  makes everything writable; a typed `while` protocol is owed.
- `IO(sig)` can't be *produced* in checked code — record mints
  synthesize plain records; `Former{type.io}` needs a declared stub
  or a real `perform`/op-mint surface (row work D doesn't cover this
  — the mint side is separate).
- `data` ctors are file-local — `__splice__` binds don't reach
  imported-arrow envs (ADR-0155 `ModuleEnv` track); `hnl test` can't
  desugar `data` (WS-F partially blocks on it).
- `:=` arrows are monomorphic across call sites (shared param metas —
  `{get}` vs `{put}` record calls collide); let-generalization/`rec`
  is the deeper item.
- `export` inside `do {…}` is an unbound wire at typecheck (scope
  pass doesn't bind it — pre-existing).
- Dict member projection `ev.eq` on an implicit-evidence binder (F8,
  carried) — `match ev { mkEq(f): … }` is the idiom; the match form
  typechecks but can't run (evidence binder has no runtime dict —
  erasure-class edge worth a diagnostic).
- Coverage-vs-arm-reachability judgment reconciliation — auto-
  discharge landed but the two passes can still disagree in
  principle (the `zipWith` class).
- `match_recursor` desugar divergence — carried (see Rejected).
- Metacircular `.hnl` eval stack depth — re-threaded spine /
  explicit-continuation eval owed, not another MiB bump.
- `type Name body` contextual parse — carried from phases 1–3;
  `type.Name :=` covers the need.
- `__tag__` payload discriminator on host-internal model records
  (Cell/Cell2/Arrow/op/Handler/law) — carried, separate cleanup.
- `sym` seeds for unexported formers; FFI boundary convention —
  carried ×3.
- `hnl run` on Path/J-only binding files — proof-erasure erases the
  runnable content (documented in the tutorial's honesty table).
- `lib-desugar` overlay lacks the ctor-proj table — parity gap if
  `Succ`-indexed families are ever exercised through it.
- Cranelift `Iter`/`Path`/`Equiv` arms — dead post-0119 folds; delete
  or document with WS-C's `Child` pass.
- `hnl run --unchecked` flag — dogfood2 ask, unscheduled.

## Progress log

*(Appended as workstreams land — newest workstream entries first,
dated `###` sections for session-scale landings; per-workstream
entries open `- **WS-X landed** (repos): …` with semantics,
surprises, and exit-criteria verdicts — the phase-1–3 convention.)*
