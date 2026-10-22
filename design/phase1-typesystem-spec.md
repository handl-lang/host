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
  Next: WS-B — `infer`/`check`/`conv` over `Val` in
  `engine-typecheck-nbe`; port `.hnl` model coverage.
