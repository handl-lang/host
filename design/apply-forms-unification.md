# Apply-forms unification — proposal

Status: proposal + conservative smoke coverage (see §7).
Scope: the *application* surface of Handl — the ways a former/eliminator
is applied to arguments. Verified against
`crates/handl-kit-prelude-lang/data/{grammar,tokens,lexer_rules,ast,eval,types,transforms}.yaml`
(canonical kit, ADR-0044 "language as data") and probed through the
production `hnl typecheck` path.

## 1. Inventory (verified, not assumed)

`grammar.yaml` declares **91 rows**. The application-relevant ones:

### 1.1 Call-form — `f(a, b, c)` (primitive)

- `grammar.call.infix` (`token.lparen`, bp 90/91) → node kind `call`,
  children `[callee, args…]` (variadic `args` role; `ast.call`).
- `grammar.paren.prefix` → `paren` node; **`paren` is transparent**:
  `type.paren` synths `synth_child(0)`, `eval.paren` is a native
  passthrough. Consequence verified by probe: every *unary* prefix
  keyword already accepts call syntax — `symm(p)`, `fibrant(A)`,
  `cobox(x)`, `univalence(e)`, `id2(x)`, `first(f)`, `second(f)`,
  `left(f)`, `right(f)`, `infer(e)` parse the `(x)` as a `paren` child
  and typecheck identically to `symm p` etc. (`/tmp/probe2.hnl` → ok).
- Currying is just nested calls: `eqv(Zero)(Zero)` → `call(call(eqv,
  Zero), Zero)`. No special form.

### 1.2 Prefix keyword forms (primitive node kinds, NOT sugar)

Each is a dedicated AST node kind with its own `ast.*` shape,
`eval.*` scheme, and `type.*` synth rule in the `.hnl` typecheck model:

| Surface | Row | Node kind | Arity / separators |
|---|---|---|---|
| `fibrant A` | `grammar.fibrant.prefix` | `fibrant` | 1 |
| `symm p` | `grammar.symm.prefix` | `symm` | 1 |
| `cobox x` | `grammar.cobox.prefix` | `cobox` | 1 |
| `univalence e` | `grammar.univalence.prefix` | `univalence` | 1 |
| `id2 x` | `grammar.id2.prefix` | `cell_id` | 1 |
| `J p, motive, base` | `grammar.j.prefix` | `j` | 3, `,` `,` |
| `trans p, q` | `grammar.trans.prefix` | `trans` | 2, `,` |
| `glue A, φ, T̂` | `grammar.glue.prefix` | `glue` | 3, `,` `,` |
| `transp A, i, φ, u` | `grammar.transp.prefix` | `transp` | 4, `,`×3 |
| `hcomp A, i, φ, u, cap` | `grammar.hcomp.prefix` | `hcomp` | 5, `,`×4 |
| `Comp F, i, φ, u, cap` | `grammar.comp.prefix` | `comp` | 5, `,`×4 |
| `arr op` | `grammar.arr.prefix` | `arr` | 0 + `payload_tokens` (bare identifier, **no** call form) |
| `first/second/left/right f` | `grammar.{first,second,left,right}.prefix` | same | 1 (bp 70) |
| `infer e` | `grammar.infer.prefix` | `infer` | 1 (bp 70) |
| `neg`/`!x` | `grammar.neg.prefix`/`not.prefix` | `unary.neg`/`unary.not` | symbol prefixes, n/a |

The multi-arg rows use `child_separators: [Required(token.comma), …]`
— **required** separators, not `optional_children`.

### 1.3 Infix operator forms (primitive)

| Surface | Row | Node kind | bp (l/r) |
|---|---|---|---|
| `p # v` | `grammar.transport.infix` (`token.sharp`) | `transport` | 8/9 |
| `a >>> b` | `grammar.flow.infix` | `flow` | 25/26 |
| `a >>>[k] b` | `grammar.axis_flow.infix` | `axis_flow` | 25/26 |
| `a =>> b` / `a =>| b` | `cell_vcomp`/`cell_hcomp.infix` | `cell_comp_v`/`cell_comp_h` | 8/9 |
| `a <@ b` / `a @> b` | `at_left`/`at_right.infix` | `whisker_left`/`whisker_right` | 8/9 |
| `x ?> y` | `grammar.cobind.infix` (`token.cobind`) | `cobind` | 8/9 |
| `x where T` / `e as g` | `refine.where`/`witness.as` | `refine`/`witness` | 8/9 |
| `f app x` | `grammar.app.infix` (`token.keyword.app`) | `app` | 85/86 |
| `A ~> B` | `grammar.arrow.infix` | `function` | 10/9 (right-assoc) |
| `A ~[N]~> B` | `grammar.dim_arrow.infix` (`token.dim_open` `~[`) | `dim_arrow` | 10/9 |
| `F<i, …>` | *parser special production* `dimension_applies` (not a row) | `dimension` | — |

### 1.4 Special forms (declarations / contextual — out of scope)

`match` (+ `return … with` motive via `keyword_clauses`), `handle …
with … [acc …]`, `instance C(T) := {…}`, `data`, `forall`/`exists`
(declared sugar → `Pi`/`Sigma` former calls, lowered by
`transform.desugar.exists`), `sort`/`grade`/`op`/`trait`/`impl`,
statements (`:=`, `export`/`import`/`from`).

### 1.5 Already-existing call-form aliases (the precedent)

`Glue(A, φ, T̂)`, `isFibrant(A)`, `unglue(G)`, `ua(e)`, `Univalence(e)`,
`Equiv(A, B)`, `Path(A,x,y)`, `PathP(F,x,y)`, `Family/Pi/Sigma/Forall`
are **ordinary identifiers** intercepted inside the typecheck model's
call judgment — `synth_type_call_cubical_`
(`tool-typecheck-handl/src/models/typecheck_model.hnl:5304`, dispatch
sites ~7339–7378). They need no grammar rows and no keywords; the
lexer_rules comment says it outright: *"`Glue`/`isFibrant`/`unglue` stay
ordinary identifiers — the call forms need no keyword."*
(`lexer_rules.yaml:462-463`). Eval needs no rule: these live in type
position only (proof-erasure reading, `eval.yaml:1439-1465`).

## 2. Primitive vs sugar

- **Primitive**: every node kind in §1.2/§1.3 — each has `ast.*` roles,
  `eval.*` scheme, `type.*` rule. The keyword spellings are the *only*
  surface for the multi-arg forms.
- **Sugar (declared)**: `forall`/`exists` → `Pi`/`Sigma` calls;
  `dimension`/`index_app` → `call` (`transform.desugar.dimension`);
  `entry` computed keys → `paren`. None of the §1.2 forms are sugar.
- **De-facto sugar today**: unary keyword forms accept `f(x)` for free
  via transparent `paren` (§1.1). Multi-arg forms do **not** — verified:
  `trans(P, Q)` fails to parse (`expected separator token.comma, got
  token.rbrace`), because `trans` is a keyword: the prefix row parses
  `(P, Q)` as ONE `paren` child, then requires the `,` separator.

## 3. Why `Kw(a, b, c)` cannot be added in YAML alone

Three parser facts (all in `tool-parse-pratt/src/lib.rs`, the generic
Pratt engine — ADR-0053/0081):

1. `lookup_row` keys grammar rows by `token@position` — **one row per
   token per position** (`lib.rs:128-134`). `J` cannot have both a
   comma-separated row and a paren-delimited row.
2. A prefix shape's `delimited` block is parsed **unconditionally after
   the normal children** (`lib.rs:1204-1212`) and errors when the open
   token is absent — it cannot be "optional" or "instead of" the
   comma-separated children.
3. `child_separators` entries are `Required`/`Optional` *separator
   tokens*; making the commas `optional_children` would let `J(p,m,b)`
   parse as `j(paren(p,m,b))` — one child — which then needs a desugar
   unwrap AND matching unwraps in all six cubical `synth_*` ops in the
   typecheck model (which sees the **pre-desugar** AST — the `exists`
   comment in `transforms.yaml` documents this), across **three md5-synced
   copies** of `typecheck_model.hnl` (`scripts/check-model-sync.sh`).
   It also silently relaxes `J p` (missing args) from parse-error to a
   later type error. Rejected as not-clean.

The clean engine route already has precedent: `dimension_applies`
(`lib.rs:243`) is a parser *special production* for `F<…>` that fires on
token adjacency, exactly the shape a "keyword-call" production would
take (see §5, Route A).

## 4. Recommended unification

**Canonical style: call-form `f(a, b, c)` everywhere it denotes
application of a former/eliminator** — `j(p, motive, base)`,
`trans(p, q)`, `symm(p)`, `transport(p, v)`, `glue(A, φ, T̂)`,
`hcomp(A, i, φ, u, cap)`. Rationale:

- It is already the dominant style for ordinary functions
  (`VCons(x, xs)`, `Vec(0, a)`, `perform(puts, "hi")`, `idn(Succ(Zero))`,
  `Glue(...)`, `isFibrant(...)`, `ua(...)`) — the keyword/comma forms are
  the *minority* orthography, confined to proof-level cubical code.
- Arity is self-delimiting: `hcomp A, i, φ, u, cap` relies on comma
  counting that breaks the moment an argument contains a top-level
  comma-ish construct; `(…)` nesting is unambiguous and matches every
  other language the kit interoperates with.
- `~>`/data-everything philosophy: `call` is already the universal
  application node; routing eliminators through it (or through their
  dedicated node kinds spelled `Kw(…)`) keeps one mental model —
  "formers apply like functions".

**Keep the keyword forms as deprecated-or-sugar aliases during
transition — zero breaking changes.** This mirrors the `:=` migration
precedent (grammar.yaml header: `=`/`==` demoted to `binary.eq`,
`:=` made canonical; ~86 repos moved mechanically). Old sources keep
parsing; a codemod/lint (`tool-codemod`, `hnl-tool-transform`) can
rewrite `trans p, q` → `trans(p, q)` mechanically when desired — the
rewrite is local and unambiguous because the comma-separated children
map 1:1 onto call arguments.

### Alternatives evaluated honestly

- **Prefix-uniform** (`f a b c` for everything): rejected — loses
  arity delimiters entirely; `hcomp A, i, φ, u, cap` becomes
  unparseable without commas anyway, and bare juxtaposition collides
  with statement sequencing. Strictly worse.
- **Method-style `p.transport(v)` / `p.trans(q)`**: attractive
  (reads like path algebra) but a semantic shift, not syntax: it
  requires every eliminator to be a record field/projection on the
  path value, i.e. changes what paths *are*. `transport`'s two
  operands aren't even same-typed (`p : Path`, `v : T`). High cost,
  questionable fit — rejected for this pass; could be revisited as a
  library-level combinator (`Path.transport := p ~> v ~> p # v`)
  which needs no syntax at all.
- **Status quo + documentation**: viable fallback; the forms are
  low-traffic (§6). But it leaves two application orthographies
  forever and blocks the "one application style" goal.
- **Demote keywords → identifiers + model interception** (the
  `Glue`/`ua` pattern extended to term level): would give
  `trans(p,q)`/`J(p,m,b)` as `call` nodes intercepted in the `.hnl`
  models — but `J p, m, b` then **breaks** (identifier can't head a
  bare-argument form), and term-level interception hijacks bindable
  names (`transport` probe: currently `unbound wire`, but a user
  `transport := …` binding would be silently captured). Rejected as
  breaking + namespace-polluting.

## 5. Implementation routes (for the follow-up workstream)

**Route A — parser special production (recommended).** Add a
"keyword call-form" production in `tool-parse-pratt`, alongside
`dimension_applies`: when the next token is a keyword whose grammar
row's `child_shape` declares `child_separators` of `Required(comma)`
and the token *immediately after* is `(` (same-line, mirroring
`call.infix`'s `same_line: true`), parse the parenthesized comma list
and build the row's `node_kind` with those elements as children —
**the same node kinds downstream**, so `eval.*`/`type.*`/`ast.*` need
no changes at all. Data-driven guard: only fire for keyword tokens
whose shape is comma-separated (a `Language`-queryable property), so
`if (`, `match (`, `handle (` etc. are untouched — their shapes have
no comma separators / different structure. `J(p,m,b)` then produces
literally the same `j` node as `J p, m, b`. Estimated: ~80–150 lines
Rust + parser tests; one repo (`tool-parse-pratt`).

**Route B — desugar/conditional-children (rejected, §3).**

**Route C — do nothing syntactic; document + lint.** Add a lint rule
(`lang.lints`, `!Native` or `!Match`) that *suggests* call-form for
new code while both parse. Zero risk, zero unification — fallback.

## 6. Migration cost

Traffic is minimal — the multi-arg keyword forms are confined to
cubical/proof code:

- `host/tests/smoke/{cubical,cubical_err,cubical_univalence_err,types,
  alias_forall}.hnl` — ~36 keyword-form hits, all in smoke/fixtures.
- `host/crates/handl-runtime/tests/fixtures/type_model.hnl` — fixture.
- Sibling `.hnl` corpus (libs, kit): **0 hits** for
  `J|trans|symm|transp|hcomp|Comp|glue|fibrant|cobox|univalence|id2`
  prefix usage outside comments (grepped `lib-*`, `kit-*`,
  `tool-parse-pratt`, `tool-typecheck-*`).
- The unary forms (`symm p`, `fibrant A`, `first f`, `arr op`,
  `infer e`, …) are somewhat wider (`arr`/`first`/`second` in
  arrow-heavy code) but already accept `f(x)` — no migration needed
  for them at all.

Because Route A keeps old forms parsing, **migration cost is zero**;
enforcement (lint warning on keyword forms) is optional and can ride
the existing `lang.lints` machinery later. If a hard cutover is ever
desired, the `:=` precedent applies: mechanical rewrite, one pass per
repo.

## 7. What was implemented in this pass (conservative subset)

No grammar/engine changes were needed or safe:

- **Verified already-working sugar**: unary prefix keywords accept
  call syntax today via transparent `paren` — `symm(pt)`,
  `fibrant(Bool)`, `cobox(x)`, `univalence(e)`, `id2(x)`,
  `first/second/left/right/infer f(x)` all typecheck identically
  (probe: `hnl typecheck` ok). No YAML change required; documented
  here.
- **Smoke coverage added** (`tests/smoke/cubical.hnl`): `symm(tt)`
  and `fibrant(Bool)` call-form twins of existing prefix-form lines,
  locking in the behavior.
- **Deliberately NOT implemented**: `trans(p,q)`/`J(p,m,b)`/`glue(…)`
  call forms (require the Route-A parser production — engine work in
  `tool-parse-pratt`, left for the follow-up), and `transport(p, v)`
  (parses as `call` today but is an unbound identifier — giving it
  meaning requires either keyword-izing `transport` (breaking) or
  term-level model interception that hijacks a bindable name; both
  are design decisions for the user, see Open Questions).

## 8. Interaction with `~[g]~>` graded-arrow split

Separate workstream, noted only: `grammar.dim_arrow.infix` (`~[N]~>`)
and the graded `~` arrow (`~ EXPR ~>`, parser special per the
`tool-parse-pratt` test comment at `lib.rs:3611`) are *type formers*,
not applications — the apply-forms decision doesn't constrain them.
If the graded-arrow split introduces a call-style graded former
(e.g. `graded(A, g, B)`), Route A's "comma-separated keyword shape"
rule would cover it for free.

## 9. Open questions for the user

1. Route A (parser special production) vs Route C (lint-only) — which
   should the follow-up workstream implement?
2. `transport(p, v)`: should `#` transport get a call-form name at
   all? If yes, prefer a *fresh* identifier (`transport` is bindable
   today) or accept the `Glue`-style interception + reserved-name
   consequence.
3. Should unary keyword forms (`symm`, `fibrant`, `first`, `arr`, …)
   eventually be demoted to plain identifiers + model interception
   (maximal uniformity, frees the keywords) or kept as keyword
   prefixes (status quo, already call-compatible)?
4. Timing: land Route A before or after the `~[g]~>` split?
