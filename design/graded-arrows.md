# Graded arrows — generalization design (MTT modes framing)

Status: proposal (design only — no code), **reframed to modes 2026-11-25**.
User decision: "MTT looks the best one, because it generalizes" — the
bracket payload `G` in `A ~[G]~> B` is a **mode** (an object of a mode
theory, MTT/Gratzer–Kavvos–Nuyts–Birkedal), generalizing coeffect grades.
Filename kept (`graded-arrows.md`) because `graded_arrow`,
`attrs["grade"]`, `Arrow.grade`, and `grade`/`sort ... grades` are
committed identifiers and ~30 files link here; "grade" in prose below
means "mode" unless it names one of those identifiers. Migration plan:
[`plans/pl-0002-mtt-modes-migration.md`](../plans/pl-0002-mtt-modes-migration.md).
Scope: the semantic/design side of the arrow taxonomy split approved in
[`syntax-decisions.md`](syntax-decisions.md) (2026-11-24). The grammar/parser
mechanics (`~>[n]` dim spelling, `~[G]~>` grade/mode spelling, old `~[n]~>`
kept as sugar) are being implemented separately; this doc covers what a
*mode* is, what the checker does with it, how it relates to
`IO<>`/`handle`/`perform`, and the codemod disambiguation rule.

Verified against:

- `crates/handl-kit-prelude-lang/data/{grammar,tokens,lexer_rules,ast,eval,types,transforms}.yaml`
  (canonical kit data — the parser agent's in-flight edits noted where relevant)
- `tool-typecheck-handl/src/models/typecheck_model.hnl` (the Handl-authored
  typecheck model, ADR-0105)
- `tool-typecheck-nbe/src/{checker,dependent,judgments}.rs` (NBE engine)
- `exec-tree-walk/src/lib.rs` (`arrow_field` native op, ~line 3038)
- `crates/handl-kit-arrow-lower/src/lib.rs` (`op` lowering, `lower_dim_term`,
  ~lines 1960–2037)
- `tool-parse-pratt/src/lib.rs` (legacy `graded_arrow` special case, ~line 1292)
- A workspace-wide grep census of `~[…​]~>` in `.hnl` sources (see §9).

---

## 0. Modes reframing (MTT) — 2026-11-25

The "grade" framing below generalizes to **modes**. What changes and what
doesn't:

- **A mode is an object of a mode theory** (MTT: a 2-category of modes;
  Gratzer–Kavvos–Nuyts–Birkedal). Coeffect grades (QTT/Granule) are the
  canonical instance; usage counts, permission rows, session protocols
  (ADR-0203), cost, and staging are sibling instances — one mechanism,
  many algebras, exactly the generalization the user chose.
- **Dimensions are NOT modes.** `~>[n]` is the globular cell index —
  `Hom(n, src, tgt)` per ADR-0153/0119 — an index into the globular
  structure, not an object of a mode theory. The two axes stay strictly
  separate: `dim_arrow` vs `graded_arrow` node kinds, `Op.dim` vs
  `attrs["grade"]` channels, `infer_hom` vs mode checking. MTT itself
  keeps these apart (its modes annotate judgements; globular dimension
  is a different structure).
- **Effects are a mode-algebra instance — deferred (M4).** MTT places
  effect rows in the mode theory; handl's `IO<sig>` codomain former +
  `handle`/`perform` is already a complete implemented story
  (`handle_sig_check`, residual sigs). Unifying them is assessment-only;
  `IO<sig>` stays canonical. Assessment:
  [`effects-as-modes.md`](effects-as-modes.md) — **deferred**; optional
  subset = effect row as a registry algebra instance (gated on M2).
- **Naming decision (recorded)**: keep `graded_arrow`,
  `attrs["grade"]`/`OP_GRADE_ATTR`, `Arrow.grade`, `grade`/`sort ...
  grades` — serde-visible and already landed/drafted; "mode" is the
  semantic term. A rename (`mode_arrow`/`attrs["mode"]`) is evaluated at
  M5 cleanup, not assumed. Rationale: renaming a serde key breaks
  serialized `Term`s for zero semantic gain.
- **Mode-on-former vs coeffect-on-input — resolved (was §11.9)**:
  `A ~[m]~> B` reads as **the arrow is a morphism in mode `m`** — the
  MTT judgement-mode reading, which specializes to the QTT/Granule
  coeffect reading ("using this arrow consumes `A` at mode `m`") for
  binder-style algebras, and to whole-morphism readings (session
  protocol of the arrow's behavior, ADR-0203) for protocol algebras.
  The governing mode algebra decides which reading applies — same
  conclusion as the grade design, now with the MTT vocabulary.
- **Literal-mode spelling — resolved**: modes are named/data values;
  `~[2]~>` is permanently claimed by the dimension reading, so a
  numeric-count mode needs `~[(2)]~>` or a named mode (`Usage(2)`).
  Unchanged from §5/§10.
- **Algebra interface**: unchanged from §3 adjustment 1 — a mode
  algebra is a Handl data structure exposing compose/unit/order,
  selectable per composition site (Grass precedent). `mode_compose`
  replaces `grade_compose` as the judgment-op name.
- **Honest scoping**: representation + well-formedness + canonical
  default algebra + subsumption where annotations exist is P2-sized.
  Full MTT — mode contexts (`Γ ⊢ M : A @ m`), mode substitution,
  strictness/usage extraction, mode polymorphism, modality bridges —
  is research-grade and **out of scope**. See pl-0002 "Honest scoping".

---

## 1. What exists today (verified)

### 1.1 Surface and AST

- `A ~[N]~> B` parses as node kind `dim_arrow`, arity 3, roles
  `source`/`dim`/`target` (`ast.yaml` `ast.dim_arrow` ~line 627;
  `grammar.yaml` `grammar.dim_arrow.infix` ~line 56, infix token
  `token.dim_open` = `~[`, separator `token.dim_close` = `]~>`).
- The in-flight parser work adds `grammar.dim_arrow.bare.infix`
  (`token.dim_arrow_open` = `~>[`, separator `token.rbracket`) minting the
  **same** `dim_arrow` node — `A ~>[n] B` and `A ~[n]~> B` are already two
  spellings of one node kind (grammar.yaml ~lines 67–86).
- There is **no `graded_arrow` node kind** in the canonical kit data. The
  name survives only in the legacy `tool-parse-pratt` special case
  (`A ~[R]~> B` / `A ~ R ~> B` → `graded_arrow`, grade child = arbitrary
  expression incl. array grade lists — `lib.rs` ~lines 1292–1360, tests at
  ~4102–4233) and in `handl-kit-arrow-lower`, which accepts
  `dim_arrow | graded_arrow` signature shapes interchangeably (~line 1983).

### 1.2 Eval

`eval.dim_arrow` (`eval.yaml` ~line 1191) mints a tagged `Arrow` record
`{inputs, grade, output}` where the **middle (`dim`) child is stored in the
`grade` field** — the field name is legacy; today it carries the *dimension
payload value*, not a grade. The `Op`'s own `dim` slot is the dim child term.
`eval.op` (~line 1226) mints `{__tag__: "op", name, inputs, grade, output}`
via the `arrow_field` native op, which projects `inputs`/`grade`/`output`
from either an `Arrow` record or a `TaggedValue::Arrow` closure
(`exec-tree-walk/src/lib.rs` ~line 3038).

### 1.3 Typecheck — two engines, two representations

- **Handl model** (`typecheck_model.hnl` `synth_dim_arrow`, ~line 7034):
  synthesizes `Arrow{inputs: src_t, grade: <synth of middle child>,
  output: tgt_t, dim: <numeric literal or null>}`. The `grade` field is the
  *type* of the middle expression (`type.number` for a literal dim; the
  declared type of a `var`/call payload). `cell_boundary_`/`cell_paste_`
  (~lines 2761–2844) project globular boundaries off `inputs`/`output`/`dim`
  for `a >>>[k] b`; `cell_paste_` propagates `grade` unchanged — **no grade
  constraint is ever checked** (no join, no subtyping, no algebra dispatch).
- **NBE** (`tool-typecheck-nbe`): `dim_arrow` in *term* position →
  `synth_dim_arrow_judgment` (`dependent.rs` ~line 9118) emits canonical
  `Former{type.hom}` data `Hom(n, src, tgt)` per ADR-0153 — the dim index is
  the first `Hom` arg; the comment at ~9157 notes the emitted former
  "no longer carries a `grade` field". In *annotation* position,
  `dep_synth_type`'s `dim_arrow` arm (~line 3346) emits `hom_former(dim_v,
  va, vb)` directly. `checker.rs::infer_hom` (~line 622) enforces the
  boundary law: `Hom(1,·,·)` is a 1-cell type; `Hom(n≥2,·,·)` requires
  parallel `(n−1)`-cells with convertible `(n−2)`-faces; non-literal `n` is
  stored unchecked ("honest bound").
- **Arrow lower** (`handl-kit-arrow-lower`): an `op` whose signature is a
  `dim_arrow`/`graded_arrow` lowers its middle child into `Term::Op.dim` —
  numeric literal → `Const n`, bare `var` → named dim `Const "S"`, anything
  else → lowered term (`lower_dim_term`, ~line 2014). ADR-0120 Phase 3
  unified grade and dimension into the single `Op.dim` slot.

### 1.4 The census — what `~[…​]~>` actually means in sources today

Workspace-wide grep over `.hnl` files (excluding `target/`):

| payload | count | reading |
|---|---|---|
| `~[1]~>` | 8 555 | dimension-1 arrow (= plain `~>`, `Hom(1,·,·)`) — export signatures |
| `~[2]~>` | 505 | 2-cells — rewrite/law types (`lib-type/src/dependent.hnl` etc.) |
| `~[3]~>` | 274 | 3-cells — coherence laws |
| `~[N]~>`, `~[n]~>` | 140 | open dimension variables in dimension-polymorphic laws |
| `~[4]~>`, `~[5]~>` | 6 | high-dim cells |
| named/expr payloads (`~[Addable]~>` etc.) | **0** | — |

**Every existing `~[…​]~>` site is a dimension.** The graded reading of the
bracket has zero real usages: `op` declarations in the wild use plain `~>`
signatures (`op "assert.equal" : (Value, Value) ~> Value`), and effects are
carried by `IO<sig>` codomain annotations, not arrow grades. The only
"grade-like" bracket uses are comments (`lib-loader/src/index.hnl` ~line
268: "all signatures grade `~[1]~>`" — i.e. dimension-1).

This makes the codemod nearly mechanical (§9), but it also means the
*transition parse rule* in `syntax-decisions.md` — "numeric payload →
dimension reading; named/expr → grade" — would misclassify the 140
`~[N]~>`/`~[n]~>` open-dimension sites as grades. See §9.3.

### 1.5 Grades that already exist as *data*

The graded-theory declarations are real and unrelated to the bracket:

- `grade Name` / `grade Name(params)` — a grade declaration, a "generating
  0-cell proposition" (`grammar.grade.prefix` ~line 642, `ast.grade`,
  `type.grade` ~line 2601).
- `sort Name(params) grades G1, G2` — a sort carrying a grade list; the
  grades clause is a **single expression** (tuple/call), since `,` is not
  infix (`grammar.sort.prefix` ~line 620, comment ~592–595).
- `op name : signature` — "graded operation declaration", but the grade
  lives in the *signature arrow*, not a clause (`grammar.op.prefix` ~line
  664; `ast.op` roles `name`/`signature`). `tokens.yaml` ~line 243 still
  documents an aspirational `op name : sig grade G` trailing clause that no
  grammar row implements.
- Legacy dialect: `A ~ R ~> B` and `A ~[R]~> B` → `graded_arrow` in
  `tool-parse-pratt` (grade = arbitrary expression, incl. `[G1, G2]` array
  grade lists).

So "grade" already names a first-class *declared entity* (a proposition/tag
produced by `tag.construct`), distinct from both the dimension index and
the effect row. The design question is what `~[G]~>` *annotates*.

---

## 2. Literature review

What the graded/coeffect literature actually does, per system:

| System | What a grade is | Where it attaches | How it is enforced |
|---|---|---|---|
| **Granule** (Orchard, Liepelt, Eades III et al., ICFP'19 "Quantitative Program Reasoning with Graded Modal Types") | Element of a **semiring** (resource bounds), a security **lattice**, or an effect **monoid** — three built-in grade kinds, user-extensible only within those shapes | Graded *modality* on types (`A [r]`, `A <IO>`); in "graded base" calculus the grade also sits on every binder and on the arrow's *input* (`A[r] ⊸ B`) | Bidirectional typechecking + Z3 SMT solver discharging grade constraints; subsumption via the semiring **preorder**; grade polymorphism by quantifying over grade variables |
| **QTT** (Atkey, LICS'18; McBride; Abel–Coquand–Dybjer erasure) | Element of a **semiring**, parameterized — the theory is generic over the semiring | On **variable binders** in contexts (`(q x : A)`), never on arrow types | Semiring arithmetic + preorder during type checking; usage counts accumulate through contexts |
| **Coeffects** (Petricek, Orchard, Mycroft 2014; Petricek thesis) | Element of a user-chosen **coeffect algebra** (monoid, vector space, …) | On contexts / individual variables (structural coeffects); graded-base puts it on function inputs | Algebraic combination of contexts; shape of the algebra must match the shape of the typing rules |
| **Koka** (Leijen) | Not a grade — an open **effect row** | On the function type's effect slot: `τ -> e τ'` | Row unification + subsumption by row width/depth; fully inferred |
| **MTT** (Gratzer, Kavvos, Nuyts, Birkedal) | A **mode** — an object of a mode theory (2-category); modes can be effect contexts | On every judgement (`Γ ⊢ M : A @ E`) | Mode matching between domain/codomain; modalities bridge modes |
| **"Graded arrows" (category theory)** | A morphism in a **grading category** — Sanada's category-graded effects (`CatEff`), compositional-game-theory graded arrows | On the arrow itself, indexed by grading-category morphisms | Composition = grading-category composition; handlers = graded algebra morphisms |
| **Grass** (Hanukaev & Eades) | Elements of **multiple coexisting grade algebras** — each variable controlled by its own algebra | Per-variable in context | Per-algebra operations; closest literature precedent for "pluggable algebra per site" |

**MTT is the chosen unifying frame (2026-11-25, §0)**: of the systems
above, MTT's *mode* is the only notion general enough to host coeffect
grades, usage counts, session protocols, and (deferred) effect rows under
one mechanism — which is why `~[G]~>` payloads are called *modes*.

Concrete instantiations worth noting:

- **Idris 2** implements QTT with the concrete `{0, 1, ω}` zero-one-many
  semiring on binders (erasure / linear / unrestricted). **Agda** has only
  the `0` erasure modality so far — full QTT linearity is blocked on
  constraint machinery (agda/agda#4642). **Lean** has no graded-arrow or
  quantity mechanism at all. No mainstream proof assistant ships
  semiring-graded *arrows*; graded arrows remain a research-language
  feature (Granule) or a categorical abstraction.
- **Granule's graded possibility modality** is the literature's answer to
  "can effects and coeffects share machinery": yes — effects are a graded
  monad over the effect-set monoid `(𝒫(IO), ∪, ∅)`, coeffects a graded
  comonad over a semiring. They coexist under one modality framework but
  remain *different grade algebras*. This is the strongest argument that
  `IO<sig>` and `~[G]~>` need not be identified — but also shows unification
  is possible if ever wanted.
- Every enforceable system fixes **some** algebraic structure: a semiring
  (QTT/Granule), a monoid (effect rows, graded possibility), a lattice
  (security), or a grading category (Sanada). "Arbitrary data with no
  operations" is not a grade domain anywhere in the literature — it is a
  grade *representation* awaiting an algebra.

---

## 3. Design assessment

> **MTT update (2026-11-25)**: the assessment below was written for the
> "grade" framing; under MTT every "grade" reads "mode". The verdicts
> hold — MTT *strengthens* the case: modes are precisely the
> generalization of coeffect grades to arbitrary mode theories, and the
> "pluggable algebra per site" requirement is the mode-theory
> requirement. Phasing now lives in `plans/pl-0002` (M0–M5).

### What holds

- **Three separate axes is the literature consensus.** Dimension (cell
  level), coeffect/usage grade, and effect are distinct axes in MTT, QTT,
  and Granule alike. ADR-0153 already resolved bracket = dimension; giving
  grades their own `~[G]~>` notation matches how the field separated these
  concerns.
- **Grades orthogonal to `IO<sig>` is defensible.** Koka puts effects on
  the function type and tracks no usage; QTT/Granule put usage on
  binders/modalities and no algebraic effects. handl's `IO<sig>` codomain
  former + arrow grade mirrors exactly this split. Granule shows the axes
  *can* be unified under one modality mechanism — but as different algebras,
  which is what this design already allows (a grade value may be an effect
  row; `IO<>` remains canonical for effects).
- **Arbitrary-data grades are MORE feasible here than in ML-family
  languages — with one caveat (adjustment 1 below).** Types, terms, proofs, and grades
  share one evaluator and the checker is itself Handl data
  (`typecheck_model.hnl`), so there is no metatheoretic requirement that
  grades form a semiring *in the host*. ADR-0207 §8–9 explicitly wants
  grade algebras to be ordinary typed expressions. A grade that is a
  record, an array, a `Session(!T)` call, or a proof-carrying value is
  unthinkable in Idris2/Agda and natural here.
- **`~[G]~>` bracket-infix is fine.** Comparable to Granule's `A [r]` /
  `A[r] ⊸ B` and the category-theory `A -[r→] B`; the notation already has
  in-repo precedent (ADR-0203, legacy `graded_arrow`).
- **P2 = representation + well-formedness only is the right scope.** No
  shipped system enforces grades without a fixed algebra + constraint
  solver; deferring subsumption is honest, not timid.

### What needs adjustment

1. **Pin the algebra *interface*, not the algebra.** §4's "MVP algebra =
   equality / record-merge / meta" is too ad hoc to be the composition
   semantics — every literature system requires *some* structure
   (semiring ops, monoid op, preorder). The fix that preserves
   arbitrary-data grades: a **grade algebra is a Handl data structure**
   exposing the operations `gradeCompose` needs — at minimum a compose
   operation, a unit grade, and a subsumption/order predicate — selectable
   per composition site (ADR-0207 §9, Grass precedent). P2 ships one
   canonical default algebra (record-row union for rows, equality
   otherwise, honest-bound fallback) *as data*; user algebras plug in at
   P3. This turns "pluggable" from aspirational into an interface.
2. **Pin what `G` constrains.** The literature attaches coeffect grades
   to the *consumed input* (QTT binder quantities, Granule `A[r] ⊸ B`),
   while ADR-0207's examples (`Path ~[IO]~> String`) read as a whole-morphism
   annotation. `A ~[G]~> B` should be documented as: **G grades the arrow
   as a consumer of A** (coeffect reading, QTT-aligned) — i.e., "using
   this arrow consumes A at grade G" — with whole-morphism readings
   (session protocol of the arrow's behavior, ADR-0203) as algebra-defined
   interpretations. Flagged as open question §11.9.
3. **Numeric-literal grade spelling.** `~[(2)]~>` works but is ugly and
   undiscoverable; every real usage-count system uses a named grade
   (`Two`, `Usage(2)`) anyway. Keep the restriction, document the named
   spelling as the recommended one.
4. **Phased plan: unchanged in shape.** P2 gains the algebra-interface
   item from adjustment 1; see §8.

### Algebra selection: call-head instances (user decision, 2026-11-25)

Algebras are defined **in HNL itself**: `Algebra` is an ordinary type —
a record of `compose`/`unit`/`order` functions — and each algebra is an
`instance` (reusing the existing instance-resolution machinery). Mode
payloads are *calls*: `~[Cost(3)]~>` reads as "mode `3` in the `Cost`
algebra" — the call head `Cost` selects the algebra instance, the
argument is the grade payload. This makes modes self-describing: no
registry lookup, no head-symbol table, no type-directed dispatch needed
at the selection step — instance resolution does it.

Because `Algebra` is just a type, **algebras are computable**: algebra
transformers (`product(A, B)`, `lift(f, A)`), algebra composition
(graded-monad-style), and mode expressions like `Cost(3) + Session(p)`
(mode sum = algebra compose) are ordinary Handl functions returning
algebra records. This is the everything-is-data payoff: the algebra of
algebras exists for free.

Two invariants the checker relies on (both fall out of evaluation, no
new machinery):

- **Canonical forms**: `compose` must return a mode value in the same
  call-head shape (`Cost(2) ⋄ Cost(5)` → `Cost(7)`), or unification
  sees distinct normal forms. Algebra authors get this for free by
  writing `compose` as a function returning `Cost(...)`.
- **Confluence/termination**: algebra `compose`/`order` are ordinary
  Handl eval — a diverging or non-confluent algebra diverges/sticks the
  checker, same caveat as index computation and `loop`/`rec`.

Bare non-call modes (`~[{cost: 3}]~>`, `~[Addable]~>`) still need a
fallback: either require the call form (recommended — self-describing
beats terse), or a canonical default algebra for untagged values.

---

## 4. Mode domain — recommendation ("grade domain" in the grade framing)

**Recommendation: a mode is an arbitrary Handl data value (a typed
expression), composed by a pluggable mode algebra — not a literal, not a
closed semiring, not a row type.**

Concretely, in order of increasing ambition:

1. **Grade values are data.** `G` in `A ~[G]~> B` evaluates (in type
   position) to a value: a declared grade name (`Addable`, `Session(!T)`),
   a record row (`{puts: Str, fs: Path}`), an array/tuple of grades
   (`[Addable, Comparable]` — the legacy pratt grade-list precedent), a
   numeric literal (usage count — kept legal but *not* a dimension), or a
   computed grade expression (`r >>> s`, `dual(S)`). This is the
   "everything is data" reading and matches ADR-0207 §8–9: grades are
   algebraic structures expressed with arrows, algebras are themselves
   typed expressions selectable per composition site.
2. **Composition is algebra-dispatched, defaulting to a canonical algebra.**
   `A ~[r]~> B` composed with `B ~[s]~> C` yields `A ~[r ⊕ s]~> C` where
   `⊕` is resolved by the grade algebra governing the composition —
   mirroring how `cell_paste_` already computes `dim = k+1` for dimensions.
   MVP algebra: record-row union for effect-like rows, equality for named
   grades, numeric add for literal counts; anything else → fresh meta +
   deferred diagnostic (the "honest bound" convention already used for
   non-literal dims in `infer_hom` and `handle_sig_check`).
3. **Grade checking is subsumption, not equality.** A `~[G1]~>` arrow is
   usable where `~[G2]~>` is expected iff `G1 ≤ G2` in the governing
   algebra (row width subtyping for records — the same discipline
   `IO<sig>` already implements; `s'` names at least what `s` does).
   Unresolvable → pass through (the established `handle`/`IO` convention
   for symbolic sigs).

Why not the alternatives:

- **Closed semiring (QTT-style)**: too small — can't express
  `Session(!T)` protocol grades (ADR-0203) or permission sets. Keep
  numeric grades as *one* algebra instance, not the grade type.
- **Rows only**: collapses grades into effects — forecloses usage counts,
  cost, staging, session protocols. Rows are one grade *value shape*, not
  the domain.
- **Type-level only (grades as types)**: grades like `Lengthable(n)` are
  already `sort`/`grade` declarations — propositions/tags. Treating the
  grade *annotation* as a term-level data value keeps `~[G]~>` orthogonal
  to `IO<sig>` (a type-level codomain former) and to `Hom` (a type-level
  index). The grade value can *denote* a type-level entity without being
  one.

This is deliberately the minimal reading of ADR-0207 §8–9 and ADR-0203:
the full "grade algebra as dimension argument" vision (`f<SomeAlgebra>`,
`r >>> s` inside annotations with algebra dispatch) is P3+; P1–P2 only need
grade-as-data to flow through the existing `Arrow` record.

---

## 5. Payload grammar — what `G` can be

**Recommendation: `G` is a single arbitrary expression child**, parsed
between `~[` and `]~>` and terminated by the `token.dim_close` separator —
the same capture mechanism `dim_arrow` and `axis_flow` (`>>>[k]`) already
use. No new lexer rules are needed (`~[`, `]~>` tokens exist; `~>[` is
already lexed distinctly by the in-flight work).

| payload | example | reading |
|---|---|---|
| identifier | `~[Addable]~>` | named grade (declared via `grade`/`sort`) — **or** dimension var during transition (see §9.3) |
| call | `~[Session(!T)]~>`, `~[Lengthable(n)]~>` | parameterized grade (ADR-0203 precedent) |
| record | `~[{ puts: Str, fs: Path }]~>` | effect/permission row as a grade value |
| array | `~[[Addable, Comparable]]~>` | grade product/list (legacy `graded_arrow` array-grade precedent) |
| paren | `~[(r >>> s)]~>` | composed/compound grade expression |
| literal | `~[2]~>` | usage-count grade — legal, but **numeric payloads are claimed by the dimension reading during transition** (see §9.3); post-transition `~[2]~>` stays `dim_arrow` sugar, so literal grades need a non-numeric spelling (`~[(2)]~>`) or a named grade |

**Multiple grades**: do not add `~[g1, g2]~>` comma syntax — `,` is not an
infix operator in the language and `sort ... grades` already established
the "one expression, multiplicity via data" convention (tuple/record/
array inside the clause). `~[[G1, G2]]~>` (array literal) or
`~[{g1, g2}]~>` (record row) covers it; which one *means* depends on the
grade algebra, exactly as ADR-0207 intends.

**Binding power**: the payload should parse at a binding power that admits
`>>>` (grade composition per ADR-0207 §8: `A ~[r >>> s]~> C`) but cannot
swallow the closing `]~>` — the separator token terminates it naturally,
mirroring how `axis_flow`'s `k` child parses. Whether `~>` itself is
admissible inside `G` is an open question (§11): recommend **no** — grades
compose with `>>>`, not `~>`; a grade that needs an arrow type is a
category error the checker should reject, not parse.

**Node shape**: recommend a **distinct `graded_arrow` node kind**
(arity 3, roles `source`/`grade`/`target`) rather than overloading
`dim_arrow`:

- The engine dispatches on `node_kind` via `lang.eval`/`lang.types`
  (engine-genericity rule); giving grades their own kind keeps the
  dimension machinery (`cell_boundary_`, `infer_hom`, `Op.dim` numeric
  path) untouched and makes the grade path independently extensible.
- `handl-kit-arrow-lower` already treats `graded_arrow` as a known
  signature shape (~line 1983) — the name has precedent.
- Cost: every `dim_arrow` consumer needs a `graded_arrow` sibling entry
  (`eval.*`, `type.*`, NBE `synth_*`, transforms). That is the honest
  price of not conflating two axes — the exact conflation ADR-0153 §6
  withdrew.

Alternative considered and rejected: one `dim_arrow` node with a payload
flag. It saves consumer duplication but re-introduces kind-level ambiguity
at every dispatch site — the parser agent's "numeric → dim, named/expr →
grade" transition rule would have to be re-decided at *every* consumer,
not once at parse time.

---

## 6. Semantics — what the checker does today vs. what's needed

### Today

- `synth_dim_arrow` (model) / `synth_dim_arrow_judgment` + `dep_synth_type`
  `dim_arrow` arm (NBE): produce `Arrow{inputs, grade, output, dim}` /
  `Former{type.hom}`. The middle child is *synthesized* (its type flows
  into `grade`, its literal into `dim`) but **never constrained**: any
  expression is a legal "dimension", `cell_paste_` copies `grade` through
  composition verbatim, `infer_hom` stores non-literal dims unchecked.
- `op` declarations extract `inputs`/`grade`/`output` via `arrow_field`
  and carry the middle child into `Op.dim` (`handl-kit-arrow-lower`) —
  where numeric and *named* payloads are already unified as "dimensions"
  (`Const 2` vs `Const "S"`).
- **No grade constraint exists anywhere**: no composition rule
  (`gradeCompose` from ADR-0106 §10 was never implemented — grep finds no
  `grade_compose`/`gradeCompose` in the model or NBE), no subsumption, no
  algebra dispatch. `grades_overlap` exists only in the legacy
  `type_model.hnl` fixture, not the live model.

### Needed, phased

- **P1 (representation)**: `eval.graded_arrow` mints the same `Arrow`
  record with `grade` = the evaluated grade *value* and `dim` absent (or
  `dim: 1` — see open question §11.4 on `cell_boundary_` interaction);
  `synth_graded_arrow` produces `Arrow{inputs, grade: <grade type>,
  output, dim: 1}` — a graded arrow *is* a 1-cell with an annotation, so
  `dim: 1` keeps `cell_boundary_`/`infer_hom` correct for free. The grade
  child synthesizes as an ordinary expression (term position) — its
  *type* lands in `grade`, matching the `dim_arrow` convention where the
  field holds the middle child's type.
- **P2 (minimal discipline)**: grade well-formedness only — the grade
  expression must elaborate (resolve names, check arity of grade
  constructors like `Session(!T)`). Composition carries the grade through
  a single `grade_compose` judgment op that **dispatches to a grade-algebra
  interface** (§3 adjustment 1): the algebra is a Handl data structure
  exposing compose / unit / order operations, and P2 ships one canonical
  default algebra (record-row union for rows, equality otherwise,
  meta+deferred fallback). Subsumption checking
  at `~[G]~>`-annotated binding sites is **explicitly deferred** — same
  honest-bound posture as symbolic `IO(s)` sigs.
- **P3 (full vision, ADR-0207)**: grade algebras as data, `r >>> s` inside
  grade annotations with algebra dispatch, `forall G. A ~[G]~> B`
  quantification, inference of grades from wiring. Out of scope here.

> **Superseded by `plans/pl-0002` (2026-11-25)**: P1–P3 map onto M1
> (representation), M2 (algebra interface), M3 (subsumption), with M4
> (effects-as-modes assessment) and M5 (transition cleanup) added. The
> P2 "grade_compose" judgment op is now `mode_compose`.

The key semantic commitment to record now: **`Hom` stays purely
dimensional; grades never enter `type.hom`'s index.** ADR-0153 resolved
that the bracket forms n-cell types and pushed coeffect grades to a
separate axis; the new `~[G]~>` spelling is that separate axis getting its
own notation. `A ~[2]~> B` (dim) and `A ~[Two]~> B` (grade) must never
unify — different formers, different checkers paths.

---

## 7. Grades vs `IO<>`/`handle`/`perform` — the key conceptual question

**Conclusion: complementary, orthogonal axes — not redundant, not the same
thing at different levels.**

| | `IO<sig>` effects | `~[G]~>` grades |
|---|---|---|
| What it describes | **Effects/authority**: which operations the computation may *perform* (output) | **Coeffects/usage**: how the arrow *consumes* resources — counts, cost, permissions, protocols, staging (input discipline) |
| Where it lives | Codomain: `A ~> IO<{ puts: Str }>` — an indexed former wrapping the result | On the arrow itself: `A ~[G]~> B` — annotation on the morphism |
| Checked by | `synth_perform` mints `IO{op: T}`; `handle_sig_check` (`dependent.rs` ~line 10151) verifies handler coverage and computes residual `IO(S')` via record subtyping | Nothing yet; P2 adds well-formedness + algebra-dispatched composition |
| Handler story | `handle ... with {op: impl}` interprets ops, residual sig = unhandled ops | Grade transformation under handlers is algebra-specific (a `Session` grade steps; a usage grade is unchanged by handling) — no uniform rule, which is exactly why it can't reuse `IO`'s |
| Literature | Algebraic effects (Plotkin–Pretnar), Koka `e` rows | Granule/coeffect grades, QTT quantities, MTT modes, session-type grades (ADR-0203) |

Arguments for keeping them separate:

1. **Different variance.** Effects are *produced* (covariant in the
   codomain — `IO(S)` subtypes by row width); grades constrain the arrow's
   *behavior* (a usage-2 arrow can't be used where usage-1 is expected —
   contravariant-ish discipline). One notation enforcing both would need
   two subtyping directions in one slot.
2. **Handlers already have a complete story.** `handle`'s coverage check,
   residual-sig computation, and `acc` clause are implemented and tested
   (`host/tests/smoke/effects*.hnl`). Grades must not disturb it; a
   unified "annotation = effects" reading would force `handle` to also
   reason about usage grades it has no semantics for.
3. **The MTT precedent** (cited approvingly in ADR-0153 §5): modes/grades
   and effects are different axes; conflating them was the original sin
   ADR-0153 corrected.
4. **Escape hatch preserved.** A grade *value* may still *be* an effect
   row (`~[{puts: Str}]~>`) — that's a user choice of grade algebra, not a
   language-level identification. `IO<sig>` remains the canonical effect
   mechanism; `~[G]~>` remains free to mean anything a grade algebra says.

What grades buy that `IO` can't: usage/linearity (`~[2]~>`-as-grade would
be "consume twice" — distinct from dimension-2!), cost, staging,
permissions, session protocols. Note the collision this creates and §9.3
resolves: `~[2]~>` must stay dimension-2 forever; a usage-count grade
needs `~[(2)]~>` or a named grade — numeric literal payloads are
permanently claimed by the dimension reading.

---

## 8. Phased plan

> **Superseded by `plans/pl-0002-mtt-modes-migration.md` (2026-11-25)** —
> the table below is kept for history; M0–M5 there is the live plan.

| Phase | Content | Depends on | Implementable? |
|---|---|---|---|
| **P0** | Syntax reservation: `~>[n]` dim spelling, `~[G]~>` grade spelling, old `~[n]~>` sugar. **In flight** (parser agent). | — | now |
| **P1a** | `graded_arrow` node kind in `ast.yaml`; `grammar.graded_arrow.infix` on `token.dim_open`/`token.dim_close` reusing the bracket capture; payload = expression child. Transition rule: non-numeric payload → `graded_arrow`, numeric → `dim_arrow` (per syntax-decisions). | P0 grammar slots | soon — pure data |
| **P1b** | `eval.graded_arrow` (Arrow record, grade field = grade value, `dim: 1` or absent — §11.4); `type.graded_arrow` → `synth_graded_arrow` in model + NBE judgment + `dep_synth_type` arm; `handl-kit-arrow-lower` `graded_arrow` signature path (name already accepted — verify grade child lowers to a grade term, not `Op.dim`). | P1a | soon — mirrors `dim_arrow` entries 1:1 |
| **P2** | Grade semantics: `grade_compose` judgment op dispatching to the grade-algebra interface (§3 adj. 1; canonical default algebra as data), grade well-formedness diagnostics, subsumption stub. Codemod + `~[n]~>`→`~>[n]` migration of all sources. Docs + ADR-0153/0106 conformance amendment. | P1b, codemod | medium — the semantics is the real work |
| **P3** | Vision (ADR-0207/0203): grade algebras as data, `r >>> s` in annotations, `forall G.` grade quantification, grade inference, session types as grade morphisms. | P2 + research | not soon — honest bound |

**What is NOT recommended soon**: any grade *constraint enforcement*
(subsumption at use sites, algebra dispatch ambiguity resolution, handler
interaction). P2 ships representation + well-formedness + trivial
composition; everything sharper waits for a grade algebra to exist worth
checking against. This mirrors how `IO<sig>` shipped: former first,
coverage check second, row polymorphism last.

---

## 9. Codemod spec

### 9.1 The problem

`~[x]~>` today means dimension for **every** payload kind. After the split:
`~>[x]` = dimension, `~[G]~>` = grade. A blind `~[n]~>` → `~>[n]` rewrite
is wrong for grade sites; a blind keep is wrong for dim sites. The codemod
must classify each `dim_arrow` node's middle child.

### 9.2 Disambiguation rule (recommended): payload kind first, binding second

For each `dim_arrow` node `A ~[x]~> B` in a `.hnl` source:

1. **Numeric literal payload** (`value` node, number) → **DIMENSION** →
   rewrite to `A ~>[x] B`. Covers 8 561 of ~8 721 sites (98%).
2. **Identifier payload** (`var` node) → resolve the binding:
   - bound by a **dimension binder** in scope (enclosing `<N>`/`forall`
     parameter, law pattern binder — the `f<N> := … ~[N]~> …` shape,
     `dependent.rs` ~line 3398 documents this) → **DIMENSION** →
     `A ~>[x] B`.
   - bound by a **`grade`/`sort` declaration** → **GRADE** → keep the
     `~[x]~>` spelling (it now parses as `graded_arrow`).
   - **unbound** → emit a diagnostic, skip the file (do not guess).
3. **Any other expression payload** (call, record, array, paren, string,
   template) → **GRADE** → keep `~[x]~>` (now `graded_arrow`).
   *Caveat*: a parenthesized *numeric* `~[(2)]~>` is a grade by this rule —
   correct, and the only way to write a literal grade.

Rationale for payload-kind-first: the parser is scope-free, but the
codemod runs on whole modules where bindings *are* resolvable (the same
scope pass `bind_pattern_vars_`/module prep already performs). Position
(op-signature vs type position) is **not** the rule: `~[1]~>` appears in
`export` signatures *and* law bodies and means dimension in both; `op`
signatures today contain zero bracket payloads (§1.4), so position would
buy nothing and mislead later.

### 9.3 The transition-parse hazard (must be sequenced)

`syntax-decisions.md` fixes the *transition* reading: "numeric payload →
dimension; named/expr → grade". Applied naively, `~[N]~>`/`~[n]~>` —
named payloads that are **dimension variables** — parse as grades,
silently changing the meaning of every dimension-polymorphic law
(`dedupe<N> := (g >>> g) ~[N]~> g`). Two ways out, pick one:

- **(a) Codemod-first sequencing (recommended).** Ship P0 grammar with
  `~[x]~>` → `dim_arrow` for **all** payloads (status quo parse), land the
  codemod, migrate every in-repo source to `~>[x]`, *then* flip the
  non-numeric branch to `graded_arrow`. External/user code keeps parsing
  as dimensions until they run the codemod — a safe, honest degradation
  (old meaning preserved) rather than a silent re-interpretation.
- **(b) Scope-aware disambiguation at parse/desugar.** `~[ident]~>` emits
  a provisional node; a scope-aware pass rewrites to `dim_arrow` when the
  ident is a bound dimension variable, `graded_arrow` when it's a declared
  grade. More machinery, handles un-migrated code, but adds a
  binding-dependent parse — against the "parse is scope-free" architecture.

Recommend (a): it matches the approved "codemod later, don't break
existing files" decision and keeps the parser dumb. The only cost is that
`~[Addable]~>` grade syntax is unavailable until the codemod lands —
acceptable, since zero sources use it.

### 9.4 Codemod mechanics

- Vehicle: `plugin-ext-codemod` / `hnl-tool-transform` rewrite pass (the
  same playbook as `let`→`:=`), driven by the parsed AST — classify by
  node kind + payload kind + resolved binding, rewrite the *source text*
  span of the `~[…​]~>` bracket only (payload text unchanged).
- Dry-run/report mode mandatory (global rule: mechanical changes script,
  review diff first). Expected output: ~8 721 `dim_arrow` sites →
  `~>[·]`; 0 grade sites today.
- Optional follow-up (separate pass, not this codemod): `~>[1]` → `~>`
  simplification, since `Hom(1,A,B)` is the plain arrow. 8 555 sites —
  tempting but changes annotation verbosity; leave as a style decision.
- Legacy `A ~ R ~> B` (unbracketed tilde) and `graded_arrow` nodes from
  `tool-parse-pratt`: out of scope for the canonical-kit codemod; flag
  for the legacy dialect's own migration.

---

## 10. Decision table

| Question | Decision | Basis |
|---|---|---|
| What is a mode (was: grade)? | Arbitrary Handl data value; a **mode** in the MTT sense — object of a mode theory; algebras pluggable per ADR-0207 | §0, §4; ADR-0207 §8–9; MTT |
| Mode domain closed? | No — named modes, calls, records, arrays, expressions | legacy `graded_arrow` accepted full exprs incl. arrays |
| Multiple modes syntax | Single expr; multiplicity via `[m1,m2]`/`{…}` data | `sort ... grades` precedent; `,` not infix |
| Node kind | `graded_arrow` (roles source/grade/target) — name kept, semantics = mode arrow | engine-genericity; precedent in arrow-lower; §0 naming decision |
| `Hom`/`dim` vs mode | Strictly separate formers; **dimensions are not modes** — mode never enters `type.hom` index or `Op.dim` | ADR-0153 §6; §0 |
| Modes vs `IO<>` | Orthogonal: modes = coeffect/usage discipline on the arrow; `IO<sig>` = effects/authority in codomain. Effects-as-modes unification deferred to M4 assessment | §7; MTT precedent; implemented handle story |
| Literal numeric mode | Legal but spelled `~[(2)]~>` — bare `~[2]~>` is forever dimension | §7 collision note; §0 |
| Composition mode rule | `mode_compose` judgment op dispatching to a **mode-algebra interface** (compose/unit/order as data fields, §3 adj. 1); canonical default algebra ships in M2 | §6 P2; pl-0002 |
| What `m` constrains | The arrow is a morphism in mode `m` (MTT judgement-mode); specializes to coeffect-on-input for binder algebras, whole-morphism for protocol algebras — governing algebra decides | §0 (was §11.9) |
| Enforcement now | M1–M2: representation + well-formedness; M3: subsumption where annotations exist; unresolvable → pass | honest-bound convention; pl-0002 |
| Full MTT (mode contexts, substitution, strictness, mode polymorphism) | **Out of scope** — research-grade | pl-0002 "Honest scoping" |
| Codemod disambiguation | Payload kind, then binding for idents (§9.2) | census §1.4 |
| Transition hazard | Codemod-first; `~[x]~>` stays `dim_arrow` until migrated (§9.3a); flip lands in M5 | syntax-decisions sequencing |

---

## 11. Open questions

1. **Grade expression level**: are grade children elaborated as *terms*
   (values, like `Session(!T)` constructing a tag) or *types* (like
   `sort`/`grade` declarations which are propositions)? Recommendation:
   terms — the grade annotation is data *about* the arrow. But `grade`
   declarations produce propositions; whether `~[Addable]~>` should
   require `Addable` to resolve to a declared grade (vs any value) is a
   P2 well-formedness decision. Lean: resolve against declared grades
   first, warn on undeclared.
2. **`~>` inside `G`**: recommend rejecting (`~[A ~> B]~>` is a category
   error). Needs a parser-level binding-power cap on the payload or a
   post-parse check.
3. **`graded_arrow` in term vs type position**: `dim_arrow` already has
   two readings (`synth_node` term path vs `dep_synth_type` annotation
   path). Does a graded arrow *type* `A ~[G]~> B` differ from a graded
   arrow *term*? Recommend: same node, same split — annotation position
   yields the graded-arrow *type*, term position yields the signature
   value (mirroring `op` signature usage).
4. **`dim` field on graded `Arrow` records**: `cell_boundary_` treats an
   `Arrow` with non-numeric `dim` as boundary-at-every-level — wrong for a
   1-cell. Set `dim: 1` on graded arrows (they *are* 1-cells) or teach
   `cell_boundary_` that grade-annotated arrows are dim-1. Recommend
   `dim: 1` — zero consumer changes.
5. **`arrow_field`/`Op.dim` interaction**: `handl-kit-arrow-lower` puts a
   `dim_arrow` signature's middle child into `Term::Op.dim`. For
   `graded_arrow` signatures the grade must **not** go into `Op.dim`
   (that's the dimension slot — a grade would mint `Const "Addable"` as a
   bogus dimension). Needs a distinct lowering: grade → op metadata/attr,
   `Op.dim` stays `1`. Verify `attrs`/`lit` slots on `Term::Op` can carry
   it, or add a grade field — kernel change, flag for P1b.
6. **`op ... grade G` trailing clause** (documented in `tokens.yaml` ~243,
   unimplemented): superseded by `~[G]~>` in the signature? Recommend
   yes — one grade notation, drop the clause idea; amend token docs.
7. **Grade inference**: ADR-0207 wants grades inferred from wiring like
   types. P3+; needs the algebra dispatch of open question in ADR-0207 §9.
8. **`~[G]~>` where G mentions dimension variables** (`~[f<N>]~>`?):
   grades depending on dims — allowed by "everything is data"; checking
   deferred. Note only.
9. ~~**What `G` constrains**~~ — **RESOLVED (MTT, 2026-11-25)**: `A ~[m]~> B`
   annotates the arrow as a morphism *in mode `m`* — the MTT
   judgement-mode reading. For binder-style algebras (usage counts,
   permission rows) this specializes to the QTT/Granule coeffect
   reading ("consumes `A` at mode `m`" — the default documented
   reading); for protocol algebras (ADR-0203 `Session`) it is the
   whole-morphism reading (the session protocol of the arrow's
   behavior). The governing mode algebra decides which operations the
   mode supports; richer readings remain algebra-defined. See §0.

**MTT resolutions summary (2026-11-25)**: Q9 resolved above; literal-mode
spelling confirmed (`~[(2)]~>`/named mode, `~[2]~>` stays dimension);
algebra interface = mode algebra (compose/unit/order as data, §3 adj. 1);
Q1 leans "resolve against declared `grade`/`sort` names first" (mode
well-formedness is an M2 deliverable in pl-0002); Q6 (`op ... grade G`
trailing clause) — recommend drop, one mode notation; Q2 (`~>` inside
`G`) — still recommend rejecting; Q3/Q4/Q5/Q7/Q8 unchanged, tracked in
pl-0002 phases.

---

## 12. References

- [`syntax-decisions.md`](syntax-decisions.md) — approved arrow taxonomy
- ADR-0153 (`adrs/0153-…`) — `~[n]~>` is a dimension index; grades pushed
  to a separate axis (this doc is that axis's notation)
- ADR-0207 (`design/0207-generic-arrow-calculus-any-dimension.md`) —
  graded arrows vision: grade algebras, `>>>` composition, `forall G.`
- ADR-0203 (`design/0203-session-types-grades.md` + ADR record) —
  `Session` grade family, `A ~[Session(!T)]~>` precedent
- ADR-0205 (`design/0205-n-dimensional-arrow-calculus.md`) — dim arrows,
  `~[n]~>` surface
- ADR-0120 (`adrs/…`) — `>>>[k]` composition, `Op.dim` unification
  (Phase 3: grade and dimension share the `dim` slot — this doc splits
  them again at the *surface*; the `Op.dim` slot stays the runtime
  dimension, grades need their own channel — §11.5)
- ADR-0098 (`design/0098-one-language-multiple-interpreters.md`) —
  `sort`/`grade`/`op` graded-theory declarations
- Literature (surveyed in §2): Orchard, Liepelt, Eades III et al.,
  "Quantitative Program Reasoning with Graded Modal Types" (ICFP 2019) —
  Granule graded modal types; Atkey, "Syntax and Semantics of
  Quantitative Type Theory" (LICS 2018) — semiring quantities on binders;
  Petricek, Orchard, Mycroft, "Coeffects" (2014) + Petricek thesis —
  coeffect algebras on contexts; Leijen, "Koka: Programming with
  Row-polymorphic Effect Types" — effect rows; Gratzer, Kavvos, Nuyts,
  Birkedal, "Multimodal Dependent Type Theory" — modes as the
  generalization; Sanada, "Algebraic Effects and Handlers for Arrows" /
  "Category-Graded Algebraic Theories" (ICFP/JFP 2024) — graded arrows
  via grading categories; Hanukaev & Eades, "Grass" — multiple coexisting
  grade algebras; Idris 2 QTT documentation ({0,1,ω} semiring);
  Abel–Coquand–Dybjer–Lorenzen, "A Graded Modal Dependent Type Theory"
  (partially ordered semiring modalities).
