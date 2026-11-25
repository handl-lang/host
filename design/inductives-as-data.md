# Inductives as data — design record

Status: proposal (design only — no code). Scope decision: user scoped
"inductives as data" IN (2026-11-25); this doc is the design-space record
the plan [`plans/pl-0003-inductives-as-data.md`](../plans/pl-0003-inductives-as-data.md)
executes against. Complements [`0152-data-nominal-inductive-families.md`](0152-data-nominal-inductive-families.md)
(the nominal-families decision this builds on) and
[`0200-inductive-types.md`](0200-inductive-types.md) (the desugaring
decision this partially revisits).

## What "inductive as data" means

Today an inductive is a **declaration**: `data F :: ctors` /
`new Inductive {...}` is lowered at parse/desugar time and the checker
sees only its *footprint* — `type.family{name, base}` bound to the
family name, `type.ctor_sig{...}` specs bound to each ctor name. The
spec itself is not a value: it lives in the `data` AST node, is
re-walked per use (`collect_ctor_specs_` in the model,
`parse_data_node`/`collect_ctors` in NBE), and cannot be computed,
returned from a function, composed, or inspected.

"Inductives as data" means the spec — parameters, constructor
signatures, result indices — is a first-class Handl `Value`:

```hnl
NatSpec := new Inductive { zero : Nat, succ : (pred : Nat) ~> Nat }
--- NatSpec is a VALUE; Nat := NatSpec (or equivalent registration)
--- makes the family usable exactly as today.
```

and, at full scope, specs returned by functions, derived generically,
passed to generic eliminators.

## Current machinery (verified 2026-11-25, main HEADs)

| Piece | Where | What it is |
|---|---|---|
| `type.family{name, base}` | `types.yaml` ~2073; model `synth_data` ~7139; NBE `dependent.rs` | declared family former; `base` = `type.array` of param sorts |
| `type.family_app{name, args}` | `types.yaml` ~2553 comment; model ~2137 unify; NBE ~2180 | saturated indexed application; index terms are *evaluated values* |
| `type.ctor_sig{ctor, family, params, args, result, bare}` | model `collect_ctor_specs_` ~2431, `instantiate_ctor_` ~2556; NBE `DepCtor` ~2630, `instantiate_ctor_full` ~3569 | ctor spec — **`args`/`result` are AST node indices into the source AST**, re-elaborated per lookup for fresh index metas |
| `synth_data` judgment | `types.yaml` ~2562 `type.data`; model ~7139; NBE `synth_data` port | binds `Name → type.family`, each ctor → `type.ctor_sig` |
| Match refinement | `refine_scrutinee_` ~3138, `synth_match_arms` ~5933 (model); NBE `dependent.rs` ~6782 | per-arm index equations (`eq_*` witnesses) — the dependent eliminator, checker-side |
| `F.rec` runtime eliminator | `desugar.rs` `desugar_data` ~1176ff emits `F.rec` fold machine; `desugar_match_to_recursor` ~1974; `eval.rec`/`ast.rec` (`eval.yaml` ~830); `.hnl` overlay `lib-desugar/src/index.hnl` ~1086–1401 | first-order fold over ctor-tagged values; NOT dependent — dependent reading lives only in the checker |
| Positivity | `desugar.rs` `check_strict_positivity` ~752 + type-expr parser ~792–1010; `.hnl` mirror in `lib-desugar` | syntactic pre-check in the desugar native; checker never sees ctor sigs in checkable form (0152 §4) |
| Universes | model `construct_universe`/`universe_level`/`universe_max` ~6541; `type.universe{level}` | exist; `data` param sorts already synth'd into `base` |

## Design space — the six questions

### 1. What is the spec value?

Two candidate representations:

- **(a) Surface spec as data** — a record
  `{ name, params: [{name, sort-expr}], ctors: [{name, args: [{name, type-expr}], result: type-expr | null}] }`
  where `type-expr` is an *owned AST/`Term` value* (serializable,
  inspectable), replacing today's AST-node-index references. Preserves
  the "re-elaborate per use → fresh index metas" discipline of
  `instantiate_ctor_`; positivity stays a syntactic check over spec
  data. This is the minimal honest generalization of
  `type.ctor_sig`.
- **(b) Elaborated spec** — the ctor sigs stored as already-synthesized
  `type.function` chains ending in `type.family_app`. Loses
  fresh-meta-per-use (metas would need re-generalization — a real
  problem for indexed families) and bakes in whatever scope the
  elaboration ran under. Rejected for the core rep; may appear as a
  derived/cached view later.

**Recommendation: (a).** The spec IS the existing internal rep
(`type.ctor_sig`'s shape) with node refs replaced by owned type-expr
values. `type.family`/`type.family_app` machinery is untouched — the
spec describes the *declaration*; applications still go through
`family_app`.

Concretely the spec value could be tagged (`{"__tag__": "inductive.spec",
...}`) or a plain record under a reserved key — decide at I1; the
`__tag__` convention matches ctor objects (`__ctor__`) and former apps.

### 2. Eliminators — the hard question

Today there are *two* eliminators and neither is spec-driven:

- **Runtime**: `F.rec` is emitted per-family by desugar as a fold
  machine over `{"__ctor__": sym, ...}` values; `match` lowers to
  `T.rec(motive, c0..)(indices.., v)`. The fold is generic in shape but
  the ctor→case dispatch is baked into the emitted machine.
- **Checker**: `synth_match_arms`/`refine_scrutinee_` implement
  dependent elimination by walking the `data` AST (`collect_ctor_specs_`)
  and unifying per-arm index equations. This is where dependent
  elimination actually lives.

"Generic eliminator" has two tiers:

- **Non-dependent `Ind.rec(spec, cases…, v)`** — interpret the spec
  value at eval: read ctor list, dispatch on the value's ctor identity,
  fold. Feasible: the existing fold machine is already a data-driven
  interpreter over an emitted table; making the table a runtime spec
  value is a modest generalization. Needs a ctor→case-index mapping
  carried *in the spec value* (today resolved by name-scanning `data`
  decls in `lib-desugar` `scan_data`/`lookup_decl`).
- **Dependent `match`/recursor typing over a spec value** — the checker
  reads the spec from gamma/`results` instead of the AST. The
  per-arm refinement machinery (`refine_scrutinee_`, motive typing,
  `eq_*` witnesses) is family-agnostic already — it consumes ctor sigs,
  not the `data` node, once `collect_ctor_specs_` is replaced by a spec
  lookup. **Feasible but the risky core**: both engines (model `.hnl`
  AND NBE Rust) must implement spec-driven match in parity, and the
  spec's type-expr values must elaborate identically in both. This is
  refactoring, not new theory — the dependent-elimination algorithm
  already exists; only its *input source* changes (AST scan → spec
  value).

Verdict: tractable in the current checker architecture — no kernel
work needed. The kernel `Term`/`Value` substrate already carries
arbitrary `Value` records; spec values are data. The risk is
engineering breadth (two engines, parity suites) not feasibility.

### 3. Indices

Keep `type.family_app{name, args}` exactly as-is. The spec stores
result-index *type expressions* (e.g. `n + 1` for `succ`); index
computation stays "evaluate the index term under the shared evaluator"
— `Vec(1+1)` ≡ `Vec(2)` by normalization, per 0152 §3. Spec-driven
ctors reuse `instantiate_ctor_`'s discipline: one fresh meta per family
param, bound in both gamma and results.

The genuinely hard part — indexed-family unification during match
refinement — is unchanged machinery. What changes is only *where the
ctor sigs come from*. Indexed families through spec values are
therefore I3/I4 scope, gated on the spec rep carrying index exprs
faithfully (including the `bare` ctor / unrecoverable-index `lit null`
convention — see NBE `parse_data_node` ~2685).

### 4. Positivity

`check_strict_positivity` (desugar.rs ~752) is a syntactic walk over a
type-expression *string* parsed by a small recursive-descent parser
(~792–1010). As-data version: the same algorithm over spec type-expr
*values* — arguably cleaner (no tokenize/parse round-trip). Options:

- Port the checker into a `.hnl` function over spec data (lib-type) —
  most "everything is data", but duplicates the algorithm a third time
  (Rust native + `.hnl` desugar mirror + checker-side).
- Keep it in the desugar/native layer but have it consume the spec
  value — preserves single implementation, but positivity stays a
  desugar-time concern rather than a checkable property.

Recommendation: positivity becomes a **checker-side judgment**
(`check_inductive_spec`) implemented once in the model + NBE parity,
operating on the spec value. The desugar native keeps its early-error
mirror during transition. This also fixes 0152 §4's "the checker never
sees ctor signatures in a form it can verify" — with specs as values,
it does.

### 5. Universe

`type.universe{level}` + `construct_universe`/`universe_level`/
`universe_max` exist (model ~6541). `synth_data` already synthesizes
param annotations into `base` sorts. Specs quantifying over `Type`
(e.g. a `Type`-sorted param, or ctor domains mentioning `Type`) need:
universe-level computation on spec type-exprs (the level of a type-expr
is itself a checker judgment — `universe_level` exists for type
*values*), and a positivity+universe combined check (strict
positivity in `Type` is not sufficient for `Type : Type`-style
families — needs the usual "indices in `Typeᵢ`, ctor args may mention
`F` only positively and at levels < the family's"). **Honest scope:
I4, and only the level-arithmetic part; full universe-polymorphic
inductives are research-grade.** Predicative `Type0`-only specs are
the realistic target.

### 6. `new Inductive` relationship

`new Inductive { zero : Nat, succ : (pred : Nat) ~> Nat }` (syntax
decision in graded-arrows.md §3, implementation in-flight by agent
b2ef7ae0 — `new` does NOT yet exist in kit grammar/ast data as of
2026-11-25) should be **declaration sugar over spec data**: it mints
the spec value, registers the family/ctors exactly as `data` does
today, and additionally binds the spec value to the declared name so
it is inspectable. I.e. the cheap version IS `new Inductive` — the
spec is a value *and* the declaration still works unchanged. Full
scope (specs as return values, composition, generic derivation) is
what I3+ adds on top.

Open naming question: does `new Inductive` bind `Nat` to the *spec*
value or to the *family former*? Recommend: `Nat` keeps binding the
family former (ctor namespace + `type.family`), and the spec is
bound under a derived name (`Nat.spec` or `NatSpec` if the user
writes `NatSpec := ...` explicitly). Details at I1.

## Honest feasibility assessment

| Capability | Size | Verdict |
|---|---|---|
| Spec value representation (record of ctor sigs with owned type-exprs) | P | tractable — generalizes `type.ctor_sig` |
| Specs inspectable/passable as values (`NatSpec.zero`, `Ind.ctors(spec)`) | P | record projection, free |
| `new Inductive`/`data` also binding a spec value | P–M | needs `new` machinery (b2ef7ae0, in-flight) + spec materialization in `synth_data` |
| Positivity as a check on spec values | P–M | algorithm exists; re-target input rep; two engines |
| Non-dependent generic `Ind.rec(spec, cases, v)` at eval | M | fold machine generalizes; needs ctor-identity→case-index in spec |
| Dependent match/recursor driven by spec value (no AST scan) | M–L | refactor of `synth_match_arms`/`refine_scrutinee_` input source; parity across model + NBE is the cost |
| Indexed families via spec values | M–L | rides on the above; index machinery unchanged |
| Specs computed/returned/composed by functions | M | once spec is a value this is free *for the spec itself*; *using* a computed spec for dependent elimination needs the checker to resolve spec values dynamically — see risks |
| Universe-polymorphic inductives | research | needs universe checking on type-exprs + predicativity proof |

**Bottom line**: representation + inspection + positivity = P-sized.
Generic *dependent* elimination is tractable without kernel work but
is the project's risk center — it is a two-engine parity refactor of
the hardest checker code, not new theory.

## Risks (carried into the plan)

- **Dynamic spec resolution**: if a spec is *computed* (returned by a
  function), the checker's match refinement must evaluate the spec
  expression to read ctor sigs — spec values must be in normal form
  (or the checker normalizes them) at match time. Stuck/metavariable
  specs → honest-bound diagnostic, same convention as index
  computation.
- **Type-expr fidelity**: AST-node refs today preserve *source* spans
  and laziness; owned type-expr values must round-trip through eval
  (they are data, so they will) but diagnostics that pointed into the
  `data` decl's source need span preservation in the spec value.
- **Parity drift**: model `.hnl` and NBE Rust must agree on spec
  shape, instantiation, and match refinement — the parity suites are
  the guard.
- **`new` dependency**: `new Inductive` syntax is in-flight
  (b2ef7ae0); the plan sequences spec-value work so it does not block
  on `new` — `data` decls can mint spec values first.
- **Scope creep toward W-types/generic recursion schemes**: deriving
  `map`/`fold`/`bind` generically from specs is explicitly out of
  scope; noted as future work.
