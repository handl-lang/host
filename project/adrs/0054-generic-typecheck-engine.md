# ADR-0054: Generic Typecheck Engine

## Status

Accepted

Refines [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
§3 "engines as interpreters", [ADR-0032 — Types are Values](./0032-types-as-values-remaining.md),
and [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md).
Depends on [ADR-0055 — Tagged Values](./0055-branded-values.md): the
typechecker represents types as tagged values minted by the host type
registry (unforgeable, structural equality within the registry). Completes
the data-driven promise for the **typecheck**
operation. After this ADR, the typechecker is the last stage migrated to the
"language as data + abstraction/engine split" pattern already used by lex,
parse, eval, and compile. Adding a type rule for a construct (including
`bind`, `if`, `spread`, param destructuring) becomes authoring a data
fragment — no engine source changes.

## Context

ADR-0044 established that every stage of the pipeline is driven by `Language`
**data**, interpreted by a generic **engine** that names no concrete construct.
The shipped architecture realizes this for four stages:

| Stage   | Abstraction crate                      | Engine crate(s)                                     | `Language` fragment               |
| ------- | -------------------------------------- | --------------------------------------------------- | --------------------------------- |
| lex     | `plugin-engine-lex` (`LexerEngine`)    | `plugin-engine-lex-pattern`                         | `lang.tokens`, `lang.lexer_rules` |
| parse   | `plugin-engine-parse` (`ParserEngine`) | `plugin-engine-parse-pratt`                         | `lang.grammar`                    |
| eval    | `plugin-engine-exec` (`EvalEngine`)    | `plugin-engine-exec-{tree-walk,bytecode,cranelift}` | `lang.eval` (`Scheme`)            |
| compile | (in `handl-runtime`)                   | `plugin-engine-exec-{bytecode,cranelift}`           | —                                 |

The **typecheck** stage is the one exception. It is not data-driven and has no
engine abstraction:

- `plugin-typecheck-nbe` is a **monolithic, hardcoded** normalization-by-evaluation
  typechecker. Its `Term`/`Value`/`Neutral` enums are a fixed Rust enumeration of
  canonical constructs (`Pi`, `Sigma`, `Lambda`, `App`, `Pair`, `Proj`, `Let`,
  `Forall`, `Membership`, `Vau`, …). The `synth`/`check` functions are a single
  giant `match term { … }` over that enum.
- It reads **no `Language` data**. Adding a construct means adding a `Term`
  variant and a `match` arm in Rust — exactly the situation ADR-0044 eliminated
  for lex/parse/eval.
- It does not cover `bind` or `if`, which are AST node kinds consumed by the
  eval engine via `lang.eval` schemes, not `Term` variants. The typechecker is
  disconnected from the data-driven AST. (`spread` and param destructuring do
  not yet exist as AST node kinds at all — those are language gaps, not
  typecheck gaps; this ADR makes the typechecker _ready_ for them.)
- `plugin-typecheck-core` is a thin wrapper contributing `CanonicalTypecheck`
  to the platform's `BasicTypechecker` `RuleAcceptor`; `plugin-ext-typecheck`
  re-exports it. Neither is a real engine abstraction.
- `handl-runtime` exposes `lex`/`parse`/`eval`/`compile`/`run_compiled` on
  `RuntimeHandle` but **no `typecheck`**; there is no `EngineRegistry` slot,
  no `RuntimeConfig` field, and no `plugin-lib-lang-typecheck` meta leaf.

### The defining property we want

> We do not change the engine when we change the language's type rules, laws,
> etc. Type rules are part of the `Language` (data). The generic engine just
> works on data.

This is precisely the property ADR-0044 §3 gives lex/parse/eval. Typecheck must
get it too.

### Types are values — and types are _expressions_

ADR-0032 establishes that types are values: the typechecker shares the
runtime's value space; a type is a collection of values; `is` is membership
evidence. handl commits to **full dependent types**: a type is an _arbitrary
expression evaluated at typecheck time_ to a value. For example:

```
MyType = str.upper("Hello")
a: MyType = 2   // rejects: 2 ∉ "HELLO"
```

`MyType` is a type-level expression; the typechecker must **evaluate**
`str.upper("Hello")` to `"HELLO"` and check `2 ∈ "HELLO"`. This has two
consequences that drive the design:

1. **The type-level language _is_ the value language.** There is no separate
   "type IR." A type is a value computed by evaluating an expression in the
   same language the runtime uses. Inventing a second kernel for type-level
   computation would duplicate the value language.
2. **Typecheck requires a normalizer.** To reduce type-level expressions
   (`str.upper("Hello")`, `Vec(A, n+1)`, …) to values, the typechecker must
   _run the eval engine_ (or its normalization core). NBE — full
   normalization, universe levels, Pi/Sigma/Forall, equality, membership —
   is therefore justified, not optional: it is the mechanism that evaluates
   type expressions to values.

### What typecheck adds on top of eval

Typechecking is not _just_ evaluation. It is a **judgment process** layered on
top of value evaluation. It requires operations the arrow kernel has no notion
of:

1. **Check mode** — `Γ ⊢ e ⇐ T`: verify `e` has type `T` _directed_. This is
   how a `Lambda` is checked against a `Pi` (bind the param, check the body
   against the codomain) and a `Pair` against a `Sigma`. Pure synthesis
   (`Γ ⊢ e ⇒ T`) cannot type a bare lambda; check mode is essential.
2. **Unification + metavariables** — solve `_` holes. ADR-0032 lists this as a
   remaining feature; the current nbe already has `MetaVar`/`MetaContext`/
   `unify`.
3. **Typing-context extension** — bind a variable with its type.

These are **judgments**, not arrow combinators. The decision below expresses
them as `Arr` ops the eval engine dispatches to the typecheck engine, rather
than a separate kernel — because the _computation_ in a type rule is value
computation (reuse the eval engine), and only the _judgment glue_ is new.

### The typecheck host is pure

The CLI's `typecheck` command evaluates type expressions (via NBE) but does
**not** run effects. `RuntimeHandle::typecheck` is the **pure-eval path**,
distinct from `eval` which runs effects. Type-level `str.upper("Hello")` is
fine (pure); type-level `fs.read(...)` is not — it is rejected at typecheck
time.

This has a real consequence for the engine wiring: the eval engine the
typecheck engine delegates to must be a **pure subset** — no `Foreign`
effectful ops, no IO. The capability boundary (`ArrSource::Judgment` vs
`ArrSource::Foreign`) is what makes this enforceable: the typecheck host
rejects effectful `Foreign` ops at typecheck time, matching on `source`
rather than a string prefix. The typecheck engine's bound eval engine is
constructed with effectful `Foreign` dispatch _disabled_ (or routed to a
pure-error handler), so a type expression that attempts an effect fails
typecheck instead of executing.

### Why the existing NBE `Term` enum is not the data IR

The current `Term`/`Value` enum is a _term language_ (lambda calculus + types),
not a _typechecking-algorithm description_. Mapping a node kind to a `Term`
and "evaluating" it yields a type value but does not express check/unify —
the same bidirectionality gap, with a heavier IR. It is also a big
handl-specific Rust enum (`Membership`, `Is`, `Vau`, `Collection`,
`Language`, …); serializing it bakes handl specifics into the "generic"
engine input — the opposite of generic — and keeps a separate type IR
(`Value` with `Closure`/`Neutral`/`Level`/`Forall`), contradicting "types are
values."

## Decision

Make typecheck a first-class data-driven stage. **Reuse the arrow-calculus
`Scheme` (ADR-0052) and the eval engine as the type-level computation core**;
add the judgment operations (check/unify/meta/extend) as `Arr` ops the eval
engine dispatches to the typecheck engine. NBE becomes the normalizer the
typecheck engine uses to reduce type expressions to values. No separate
type-rule DSL is invented.

### 1. Type rules reuse `Scheme` — `lang.types`

A type rule for a node kind is a `Scheme` (the same arrow-calculus kernel
`lang.eval` uses) that, when interpreted, computes the node's type (synth) or
verifies it against an expected type (check). The `Scheme` may call ordinary
value-level ops (`add`, `str.upper`, `proj_child`, `proj_env`, `const`, …)
_and_ judgment ops (`unify`, `check_child`, `synth_child`, `fresh_meta`,
`solve_meta`, `lookup`, `extend`, `quote`, `normalize`). Judgment ops are
`Arr` ops whose `source` is a new `ArrSource::Judgment` variant (or, if the
eval engine's `Foreign` dispatch suffices, registered as a dedicated module)
so the capability boundary stays structural: the typechecker and load-time
audit match on `source`, not a string prefix.

Add a new fragment set to `Language`, mirroring `EvalSchemeDef`:

```rust
pub struct Language {
    // …existing fields…
    /// Type-rule definitions: node kind → synth/check `Scheme`.
    pub types: FragmentSet<TypeRuleDef>,
}

pub struct TypeRuleDef {
    pub id: String,            // e.g. "type.if"
    pub node_kind: String,    // e.g. "if"
    pub synth: Scheme,        // Γ ⊢ node ⇒ T   (computes the type value)
    pub check: Option<Scheme>, // Γ ⊢ node ⇐ T (None ⇒ synth-then-unify)
    pub replaces: Vec<String>,
    pub requires: Vec<String>,
}
```

`TypeRuleDef` is a `Fragment` (identity_key = `node_kind`; collision without
`replaces` is an error), so a node kind has exactly one type rule. `check` is
optional: a kind without a check rule falls back to _synthesize-then-unify_
against the expected type. This keeps simple kinds authorable as `synth`-only
while preserving directed checking where it matters (`lambda`⇐`Pi`,
`pair`⇐`Sigma`, `refl`⇐`Eq`).

### 2. Judgment ops as `Arr` ops

The judgment operations are exposed to type-rule `Scheme`s as `Arr` ops. The
target op set (refined during implementation; motivated directly by the
current `synth`/`check`):

| Op                               | Judgment it expresses                                                                                    |
| -------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `synth_child(i)`                 | `Γ ⊢ child_i ⇒ T` — synthesize the type of AST child `i`                                                 |
| `check_child(i, T)`              | `Γ ⊢ child_i ⇐ T` — check child `i` against type value `T`                                               |
| `synth_type_child(i)`            | synthesize a _type-level_ child (Pi/Sigma/Forall domains)                                                |
| `eval_child(i)`                  | evaluate child `i` to a value (for the environment / closures)                                           |
| `unify(a, b)`                    | solve `a ≃ b`, updating metavariables                                                                    |
| `extend(name, ty, val)`          | extend `Γ` with a binding (scoped via the arrow's environment)                                           |
| `lookup(name)` / `lookup_var(i)` | look up a variable's type in `Γ`                                                                         |
| `fresh_meta`                     | allocate a fresh metavariable                                                                            |
| `solve_meta(id, val)`            | solve a metavariable                                                                                     |
| `quote(v)` / `normalize(v)`      | read back / normalize a semantic value to a value                                                        |
| `construct(ctor, args)`          | build a type value (`pi`, `sigma`, `forall`, `universe`, `pair`, `eq`, `collection`, `refl`, `empty`, …) |
| `match_type(v, cases)`           | dispatch on the constructor tag of a type value (e.g. `Pi` vs `Sigma`); cases bind fields                |

Directed dispatch (e.g. the `App` rule matching the function's synthesized
type: `Pi => check_child(arg, param) >> closure.apply(eval_child(arg))`, else
`ExpectedFunction`) is expressed with `match_tag` + `Choice` (ADR-0055), the
same combinators eval rules already use. Type values are **tagged values**
minted by the host type registry (ADR-0055):
`{"__tag__":"type.collection","__registry__":{...},"name":"number"}`,
`{"__tag__":"type.pi","__registry__":{...},"param":…,"body":…}`, etc. —
unforgeable, structurally equal within the host type registry. The type
tags (`type.collection`, `type.pi`, `type.sigma`, `type.forall`,
`type.universe`, `type.pair`, `type.eq`, `type.neutral`, …) and their
payload fields are the **value-encoding contract** the typecheck engine
interprets. `construct` is the `tag` op (ADR-0055) called against the host
type registry; `match_type` is `match_tag` (ADR-0055) on type tags. The
engine owns how closures/neutrals/metas are represented internally (as
`ScopeStack` is internal to the eval engine); the type values it produces
and consumes through the `Scheme` contract are tagged values.

### 3. The abstraction crate — `plugin-engine-typecheck`

A new crate, the analog of `plugin-engine-lex`/`-parse`/`-eval`. **No engine
code lives here** — only the operation marker, request, and engine trait:

```rust
pub struct Typecheck;                                  // operation marker
pub struct TypecheckRequest { pub ast: Ast, pub options: serde_json::Value }

pub trait TypecheckEngine {
    /// Typecheck `req.ast` under `lang`, returning the synthesized type
    /// (a value) or an error.
    fn typecheck(&self, lang: &Language, req: &TypecheckRequest)
        -> Result<Value, LangError>;
    fn box_clone(&self) -> Box<dyn TypecheckEngine>;
}
```

The engine reads `lang.types[node.kind]` → `TypeRuleDef` → `Scheme` and
interprets it (delegating value-level computation to the eval engine and
judgment ops to its own primitives). It names no concrete construct. This is
the "generic engine that just works on data."

### 4. NBE as the normalizer + one engine impl — `plugin-engine-typecheck-nbe`

The existing `plugin-typecheck-nbe` is refactored into an engine crate
implementing `TypecheckEngine`. The NBE algorithm — `unify`, `quote`/
`normalize`, metavariable solving, universe-level `max` — becomes **the
implementation of the judgment ops** (`unify`, `quote`, `normalize`,
`fresh_meta`, `solve_meta`, `match_type`, `construct`). The hardcoded
`Term`-enum `match` in `synth`/`check` is **deleted**; dispatch is data-driven
via `lang.types`. The dependent-type `Value`/`Neutral`/`Closure` machinery is
retained as the engine's _internal_ semantic representation (closures,
neutrals, metas are engine-internal, as `ScopeStack` is internal to the eval
engine); the types it produces and consumes through the `Scheme` contract are
values.

The typecheck engine delegates value-level computation (ordinary `Arr` ops
like `add`, `str.upper`, `proj_child`) to a bound `EvalEngine`, so type
expressions are evaluated by the _same_ engine the runtime uses — honoring
"types are values" literally. NBE's normalization core is what lets the
typecheck engine reduce a type expression to a value (e.g.
`str.upper("Hello")` → `"HELLO"`).

A second engine (e.g. a simpler algorithmic engine, or a future
elaboration-based engine) can implement the same `TypecheckEngine` against
the same `lang.types` data — the defining property of the abstraction/engine
split.

### 5. Runtime wiring — on `RuntimeHandle` (pure path)

Typecheck is _static analysis_, but because types are values it **runs the
eval engine** to normalize type expressions. It therefore depends on a bound
`EvalEngine` and belongs on the runtime, symmetric with the other stages. The
typecheck path is the **pure-eval** path (see "The typecheck host is pure"
above): the bound eval engine has effectful `Foreign` dispatch disabled, so
type expressions that attempt effects fail typecheck instead of executing.
Extend `handl-runtime`:

- `EngineRegistry::register_typecheck` + `typecheck: HashMap<String, Box<dyn TypecheckEngine>>` + `typecheck_engine(name)`.
- `RuntimeConfig.typecheck: String` (engine name, e.g. `"nbe"`; empty ⇒ unbound).
- `RuntimeHandle::typecheck(&self, ast: &Ast) -> Result<TypecheckResult, LangError>` returning the synthesized type (a value) plus diagnostics.
- `Runtime::from_registry` resolves the name against the registry; the typecheck engine is constructed with a reference to the bound eval engine configured for **pure** evaluation (effectful `Foreign` ops rejected).

The CLI's `typecheck` command calls `handle.typecheck(ast)` — it evaluates type
expressions via NBE without running any effects. This is the pure counterpart
to `handle.eval(ast)`, which runs effects.

### 6. Meta leaf crate — `plugin-lib-lang-typecheck`

A new `plugin-lib-lang-*` leaf (analog of `plugin-lib-lang-lex`/`-parse`/
`-eval`), exposing `typecheck.typecheck(lang, ast)` as a host function. Holds
`Rc<dyn RuntimeFactory>`, builds a sub-runtime, calls `handle.typecheck(ast)`,
returns the synthesized type. Wired into the runtime via the `delegate`
parameter, matching the established meta-module pattern.

### 7. Type rules as authored data — `types.yaml`

The canonical handl type rules move to `crates/handl-kit-data-canonical/data/
types.yaml`, embedded via `include_str!` and merged into `lang.types`, exactly
as `eval.yaml` feeds `lang.eval`. Rules for `bind`, `if`, `spread`, and param
destructuring are **authored here as data** — including node kinds not yet in
the AST (`spread`, param-destructuring shapes), which requires corresponding
`ast.yaml`/`grammar.yaml`/`eval.yaml` rows but **no typecheck engine change**.

Illustrative `if` rule (synth): the condition synthesizes to a boolean
collection, both branches synthesize, the branches unify, the result is the
then-branch's type — written with the same `!Arr`/`!Seq`/`!Choice` tags as
`eval.yaml`, plus judgment ops:

```yaml
- id: type.if
  node_kind: if
  synth: !Seq
    fst: !Arr { op: synth_child, args: [], source: Judgment, lit: 0 } # cond : T_cond
    snd: !Seq
      fst: !Arr { op: unify, args: [], source: Judgment, lit: null } # unify(T_cond, type.bool)
      snd: !Seq
        fst: !Arr { op: synth_child, args: [], source: Judgment, lit: 1 } # then : T_then
        snd: !Seq
          fst: !Arr { op: synth_child, args: [], source: Judgment, lit: 2 } # else : T_else
          snd: !Seq
            fst: !Arr { op: unify, args: [], source: Judgment, lit: null } # unify(T_then, T_else)
            snd: !Arr { op: proj_child, args: [], source: Native, lit: 1 } # return T_then
```

(The exact sequencing/threading of intermediate results — whether via `first`
over pairs or a `bind` op — is finalized in implementation; the point is that
type rules use the _same_ `Scheme` representation as eval rules, with judgment
ops available.)

## Consequences

### Positive

- **Typecheck becomes data-driven.** Adding `bind`/`if`/`spread`/param-
  destructuring type rules is authoring `types.yaml` rows — no engine edit.
  This closes the last gap in ADR-0044's promise.
- **One kernel, one value space.** Types are values; the type-level language
  _is_ the value language; type rules are `Scheme`s. No second kernel is
  invented — the most literal reading of "types are values" (ADR-0032) and
  the arrow-calculus kernel (ADR-0052).
- **Type expressions reuse the eval engine.** `str.upper("Hello")` in a type
  position is evaluated by the _same_ engine the runtime uses. NBE is the
  normalizer that reduces type expressions to values — justified by full
  dependent types.
- **The typecheck host is pure.** `RuntimeHandle::typecheck` is the pure-eval
  path: it evaluates type expressions via NBE without running effects. The
  capability boundary (`ArrSource::Judgment` vs `Foreign`) makes effect
  rejection structural — a type expression that attempts an effect fails
  typecheck instead of executing. The CLI's `typecheck` command is safe by
  construction.
- **A generic engine.** `plugin-engine-typecheck` names no construct; NBE is
  one impl. A second engine can coexist against the same `lang.types` data,
  enabling experimentation without forking the language definition.
- **Judgments are first-class ops.** Bidirectionality (synth/check),
  unification, metavariables, context extension are `Arr` ops — available
  to type rules alongside ordinary value ops. Check mode is preserved
  (lambda⇐Pi, pair⇐Sigma, refl⇐Eq), so the dependent-type power of the
  current nbe is retained.
- **Architectural symmetry.** Typecheck gets the same shape as lex/parse/
  eval: abstraction crate + data fragment + engine crate(s) + runtime slot
  - meta leaf. The runtime gains a `typecheck` stage alongside `lex`/`parse`/
    `eval`/`compile`.
- **`plugin-lib-lang-typecheck`** makes `typecheck` a meta function callable
  from handl, completing the self-hosting surface (`typecheck.typecheck(lang,
ast)`).

### Negative

- **Judgment ops in arrow syntax are more verbose than a purpose-built DSL.**
  Threading intermediate results (synthesized types, expected types) through
  `Seq`/`first`/tuples, and expressing directed dispatch via `match_type` +
  `Choice`, is more verbose than a dedicated judgment DSL would be. This is
  the cost paid for _not_ inventing a second kernel. Mitigation: a small set
  of sequencing combinators (`bind`, `let_val`) can be added as judgment ops
  if the verbosity proves severe; the `Scheme` representation is unchanged.
- **Coupling to the eval engine's op set.** Type rules share the eval engine's
  `Arr` op namespace. A type rule that needs a value-level op assumes that op
  is implemented by the bound eval engine. This is usually correct (types are
  values), but it means the typecheck engine is not fully standalone — it
  requires an eval engine. This is acceptable given "types are values" but
  worth noting.
- **Migration blast radius.** The hardcoded `Term`-enum `match` in
  `plugin-typecheck-nbe` is deleted and re-expressed as `Scheme` data. The
  dependent-type `Value`/`Neutral`/`Closure` machinery is retained as the NBE
  engine's internal representation, so the algorithmic core (unify/quote/metas)
  is preserved, but the dispatch layer is rewritten. Mitigation: the existing
  tests pin behavior; migration is mechanical (each `match` arm → a
  `TypeRuleDef`).
- **Value-encoding contract risk.** The `type.*` tag set and payload field
  layout (ADR-0055) are a contract between `types.yaml` and the engine. Getting
  this right is the riskiest part; it must be pinned early (see Status: the
  ADR-0055 spike fixes the encoding before authoring the full rule set).
- **`spread`/param-destructuring need AST/grammar/eval rows too.** These
  constructs do not yet exist as node kinds; typecheck data alone is not
  sufficient — `ast.yaml`/`grammar.yaml`/`eval.yaml` rows are also required.
  This ADR covers only the typecheck stage; the AST/grammar/eval work is
  prerequisite or parallel.

### Scope boundary — what this ADR does NOT promise

- It does **not** change the dependent-type _theory_ (universe levels, Pi/
  Sigma/Forall, equality, membership). The theory is unchanged; only its
  _dispatch_ moves from Rust `match` arms to data rules. Full dependent types
  are a _commitment_ this ADR relies on, not a change it introduces.
- It does **not** promise a second engine ships; it only makes a second
  engine _possible_.
- It does **not** define the full `types.yaml` rule set; that is the
  implementation work. The op set above is the target; exact tags and the
  value-encoding of constructors are finalized in a spike before authoring.
- It does **not** add `spread`/param-destructuring to the AST; it makes the
  typechecker ready for them. The AST/grammar/eval work is separate.

## Status

Accepted — the ADR-0055 value-encoding spike (tagged values + host type
registry with `__registry__` field) and the judgment-op spike
(`ArrSource::Judgment` + `synth_child`/`check_child`/`unify`/`construct`/
`match_type`/`fresh_meta`/`solve_meta`/`lookup`/`extend`/`bind`/`result`/
`eval_child`/`quote`/`normalize` judgment ops) are implemented. The full
`types.yaml` rule set (57 rules for all canonical kinds) is authored and the
NBE engine (`plugin-engine-typecheck-nbe`) implements `TypecheckEngine`
against it. `handl-host-cli typecheck` ships in the CLI for end-to-end
testing.

### Completed migration work

The data-driven typecheck engine has been brought to parity with the legacy
`plugin-typecheck-nbe` crate for **surface-syntax** typechecking. The
following work is complete:

1. **Core architecture** — `plugin-engine-typecheck-nbe` implements
   `TypecheckEngine`, interprets `Scheme`s from `types.yaml`, delegates
   non-judgment ops to `EvalEngine`, and enforces purity (effectful `Foreign`
   ops are rejected).
2. **Recursive structural unification** — `unify` handles tagged type values
   with the same tag by recursively unifying payload fields (e.g.
   `type.function{param, body}`). Previously `unify` used flat JSON equality,
   which could not unify two structurally-equal `type.function` values that
   were distinct objects. The recursive case is tried after the equality and
   metavariable-solving cases, before the error case.
3. **Occurs-check** — `unify` prevents circular metavariable solving.
4. **Metavariable solving** — `fresh_meta`, `solve_meta`, `extract_meta_id`
   for hole-filling.
5. **22 judgment ops** — `synth_child`, `check_child`, `eval_child`,
   `unify`, `extend`, `lookup`, `lookup_var`, `fresh_meta`, `solve_meta`,
   `quote`, `construct`, `match_type`, `bind`, `result`, `node_payload`,
   `json_type_construct`, `payload_field`, `field`, `type_field`,
   `synth_last_child`, `record_field_type`, `array_element_type`,
   `synth_first_child_or_unknown`, `check_json_value`, `check_last_child`,
   `synth_match_arms`, `check_all_children`.
6. **57 type rules in `types.yaml`** — all surface-syntax node kinds covered.
7. **Synth rules** for: `value`, `var`, `if`, `ternary`, all binary ops, all
   unary ops, `paren`, `block`, `array`, `object`, `function`, `call`, `let`,
   `bind`, `field-access`, `index`, `match`, `handle`, `flow`, `pipe`, `proc`,
   `rest`, `apply`, `typeof`, `type-annot`, `template`, `assign`, `loop`,
   `mood.int`, and all `stmt.*` nodes.
8. **Check rules** for: `paren`, `type-annot`, `value`, `block`, `if`,
   `ternary`, `let`, `bind`, `function`, all binary arithmetic ops, all binary
   comparison ops, all binary logic ops, `unary.neg`, `unary.not`, `array`.
9. **Purity enforcement** — Foreign ops are rejected in type rules and in
   `eval_child` delegation, matching on `ArrSource` rather than a string
   prefix.
10. **CLI** — `handl-host-cli typecheck` command for end-to-end testing.

### Remaining gaps

The following gaps remain vs the legacy `plugin-typecheck-nbe`:

1. **Foreign op nodes** — `fanout` now returns
   `type.function{param: type.unknown, body: type.unknown}` (more precise
   than `type.unknown` — it at least says "this is a function"). `json`
   and `resolve` remain `type.unknown` by design: they are dynamic
   operations (JSON path evaluation, module loading) whose result types
   cannot be determined statically. `mood.*` nodes (`mood.int`,
   `mood.cnd`, `mood.neg`, `mood.norm`, `mood.pot`) are now fully typed
   (all return `type.number`).
2. **Legacy core calculus** — Pi/Sigma/Forall/Vau with NBE closures,
   universe levels, polymorphism, and equality types are not ported
   (fundamentally different architecture from surface-syntax rules).
3. **Record field checking** — the `object` check rule doesn't verify
   individual fields against a `type.record` (needs a field-name-to-child
   mapping).
4. **Dependent types** — no closure-based type computation (Pi/Sigma body
   depends on param value).

### Architecture decision

The data-driven engine handles **surface syntax** typechecking (the
`types.yaml` rules), while the legacy `plugin-typecheck-nbe` handles the
**core calculus** (dependent types, polymorphism, universes). These are
complementary, not competing — the surface syntax lowers to the core
calculus for advanced features. The data-driven engine is the primary
typechecker for surface-level programs; the legacy engine remains for core
calculus research.
