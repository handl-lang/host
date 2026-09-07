# ADR-0094: Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)

## Status

Accepted — implemented on `adr0094-transform-engine` (phases 1–5; see
"Migration sequence", all steps landed).

An instance of the data-driven engine program
([ADR-0044](./0044-language-as-data-and-runtime-split.md) "language as
data") and the abstraction/impl crate split used by `plugin-engine-exec` →
`plugin-engine-exec-{tree-walk,bytecode,cranelift}`. Subsumes the
declarative desugar machinery of
[ADR-0053](./0053-generic-pratt-parser.md) §7, the `when` conditions of
ADR-0058, `KeepRest` of ADR-0060, and the child-role resolution of
[ADR-0081](./0081-data-driven-ast-child-roles.md).

> **Terminology.** A **transform** is a pure `Ast → Ast` rewrite: it never
> evaluates, typechecks, or emits. A **pass** is a named, ordered set of
> transform rules plus a traversal strategy (e.g. `desugar`, `lower`,
> `optimize`). A **rule** (`TransformDef`) maps a node kind to a rewrite
> within a pass.

## Context

Every stage that rewrites an AST currently hand-rolls the same machinery:
a walk over the `Ast` arena, a `remap: Vec<Option<usize>>` old→new index
table, per-kind dispatch, and copy-on-write construction into a fresh
output arena. `handl-runtime::desugar_pass` (~4700 lines) is the largest
instance: a bottom-up pass whose dispatch is `lang.ast.get_by_identity(
node.kind).desugar`, plus a `try_rust_desugar` registry of Rust functions
for forms (`while`, `do-while`, `for`, `nullish`, `opt-chain`) that need
fresh-name generation or recursive construction the declarative
`AstDesugarChild` vocabulary cannot express.

This has three problems:

1. **The pass is a singleton.** Desugar is hardcoded into `Runtime::parse`
   (ADR-0082 Phase 1.2). There is no way to define a *second* transform —
   a `lower` pass for a backend, a codemod, an optimization pass, an LSP
   refactor — without cloning the walk/remap/copy-on-write machinery.
2. **The dispatch table is buried in `AstShapeDef`.** `desugar` is a field
   on a shape fragment, not a first-class fragment set. Passes cannot be
   named, ordered, composed into pipelines, or extended by a language
   fragment the way `eval`/`types`/`grammar` can via `Language::merge`.
3. **The escape hatch is a side channel.** `try_rust_desugar` is a Rust
   `match` on kind strings inside `handl-runtime` — invisible to `Language`
   data, unordered relative to declarative rules, and unavailable to any
   other consumer. Per the engine-genericity rule, residual `kind ==`
   dispatch is debt to close, not a pattern to extend — but the
   declarative vocabulary genuinely cannot express fresh-name generation
   yet, so an escape hatch must exist *inside* the data-driven dispatch.

Meanwhile, the primitives the engine needs already exist and are proven:

- `Ast`/`AstRef`/`AstNode` arena with post-order (children-before-parents)
  layout and `subtree()` remapping — exactly the copy-on-write pattern a
  rewriting pass needs.
- `AstDesugarChild` (`Keep`/`KeepPath`/`KeepRest`/`Create`) and
  `AstDesugarCondition` — ~80% of a rewrite DSL, already working.
- `AstShapeDef::role_index`/`literal_children` — named-role child
  resolution so transforms never hardcode positions (ADR-0081).
- `Fragment`/`FragmentSet`/`Language::merge` — the merge discipline every
  other engine's dispatch table uses.
- The abstraction/impl crate pattern — `plugin-engine-exec` (marker +
  request + trait) with `plugin-engine-exec-tree-walk` et al. behind it.

## Decision

### 1. Two crates, matching the `plugin-engine-exec` split

```text
plugin-engine-transform            abstraction — contract + vocabulary,
                                   no rewriting logic
plugin-engine-transform-rewrite    first impl — rule-interpreting
                                   rewriting engine
```

The abstraction crate exports:

```rust
/// The transform operation marker — AST in, AST out, no evaluation.
pub struct Transform;

/// The trait a transform engine implements.
pub trait TransformEngine {
    /// Apply the transform described by `req` to `req.ast`.
    fn transform(&self, lang: &Language, req: &TransformRequest)
        -> Result<TransformOutput, LangError>;

    /// Match-only: find every site where a rule would fire, without
    /// rewriting (lint, LSP "find references to pattern", preview).
    fn find_matches(&self, lang: &Language, req: &TransformRequest)
        -> Result<Vec<Match>, LangError>;

    /// Owned boxed clone (ADR-0047 §2).
    fn box_clone(&self) -> Box<dyn TransformEngine>;

    /// Wire in a native-rewrite registry (the `Native(name)` escape
    /// hatch). Default: clone + ignore.
    fn with_natives(&self, natives: Rc<dyn NativeRewrite>)
        -> Box<dyn TransformEngine>;
}
```

```rust
pub struct TransformRequest {
    pub ast: Ast,
    pub root: AstRef,
    pub pass: String,                  // ignored for Pipeline/ApplyAt
    pub mode: TransformMode,
    pub options: serde_json::Value,    // opaque to this crate
}

pub enum TransformMode {
    Pass,                              // run one named pass
    Pipeline(Vec<String>),             // ordered passes, output→input
    ApplyAt { at: AstRef, rule_id: String },  // surgical rewrite (LSP/codemod)
    Plan,                              // matched sites + proposed rewrites,
                                       // no new arena
}

pub struct TransformOutput {
    pub ast: Ast,
    pub root: AstRef,
    pub trace: RewriteTrace,           // Vec<Rewrite { from, to, rule_id }>
}
```

`RewriteTrace` is free provenance: `AstRef`s are stable arena indices, so
"which input node produced this output node, via which rule" is a `Vec`
append. Diagnostics keep pointing at original spans; the LSP gets "why did
this change" for free.

### 2. `TransformDef` is a first-class `Fragment` in `handl-language`

```rust
pub struct TransformDef {
    pub id: String,              // "transform.desugar.nullish"
    pub pass: String,            // "desugar" | "lower" | "optimize" | …
    pub node_kind: String,       // dispatch key
    pub priority: i32,           // ordering when several rules match a kind
    pub when: Option<TransformCondition>,
    pub rule: TransformRule,
    pub replaces: Vec<String>,
    pub requires: Vec<String>,
}
```

Identity key: `pass + ":" + node_kind` — a kind may have one rule per pass
at a given priority; collisions without `replaces` are errors, same as
`EvalSchemeDef`. `Language` gains `pub transforms:
FragmentSet<TransformDef>` and `transforms_for(pass, kind)`.

**Placement:** `TransformDef`, `TransformRule`, `RewriteStep`, and
`TransformCondition` live in `handl-language` (alongside `Scheme`,
`EvalSchemeDef`, `AstShapeDef`) so `Language::merge` produces the dispatch
table and the data crate stays self-contained. `plugin-engine-transform`
holds only the operation layer (`Transform`, `TransformRequest`,
`TransformEngine`, `TransformOutput`, `RuleSource`, `NativeRewrite`) — the
same split as `Scheme` (kernel data, in `handl-language`) vs.
`EvalEngine` (trait, in `plugin-engine-exec`).

### 3. The rewrite vocabulary generalizes `AstDesugarChild`

```rust
pub enum TransformRule {
    Declarative {
        target_kind: String,
        children: Vec<RewriteStep>,
        payload: Option<Value>,        // patch merged into the new node
    },
    Native(String),                    // "desugar.while" — escape hatch
}

pub enum RewriteStep {
    Keep(usize),            // positional — role-less fallback only
    KeepRole(String),       // resolved via AstShapeDef::role_index (ADR-0081)
    KeepPath(Vec<usize>),
    KeepRest(usize),
    Splice(usize),          // lift child's children into parent's list
    Create { kind: String, payload: Option<Value>, children: Vec<RewriteStep> },
    Apply { rule_id: String, index: usize },  // transform a child first
}

pub enum TransformCondition {
    KindAt { index: Option<usize>, role: Option<String>, kind: String },
    PayloadEq { path: String, value: Value },
    ChildCount(usize),
    And(Vec<TransformCondition>),
    Or(Vec<TransformCondition>),
    Not(Box<TransformCondition>),
}
```

- `Keep`/`KeepPath`/`KeepRest`/`Create` are today's `AstDesugarChild`
  variants with identical semantics.
- `KeepRole` and `KindAt { role }` are the ADR-0081-native forms;
  positional `Keep`/`index` remain for shapes declaring no roles.
- `Splice`, `Apply`, `PayloadEq`, and the boolean combinators are new —
  defined now so the vocabulary is complete, implemented as needed.
- `Native(name)` is the disciplined escape hatch: dispatch stays in
  `TransformDef` data (`{ node_kind: "while", rule: Native("desugar.while") }`),
  only the implementation is Rust. A language fragment that never ships a
  `while` rule never references it — the engine stays language-agnostic.

```rust
pub trait NativeRewrite {
    fn rewrite(&self, name: &str, ctx: &mut RewriteCtx)
        -> Result<AstRef, LangError>;
}
```

`RewriteCtx` wraps `(node, ast, out, remap)` and exposes
`remap_child(i)`/`push_node(kind, children)`/`push_leaf(kind, payload)` —
natives never touch raw arena indices.

### 4. Traversal is part of the pass definition

```rust
pub enum Traversal {
    BottomUp,               // insertion order — free on the Pratt layout
    TopDown,                // context-push rewrites
    Fixpoint { max: usize },// until stable or iteration cap
}
```

`RewriteEngine::run_pass` walks insertion order (children-before-parents),
builds `kind → Vec<&TransformDef>` once per pass, and does copy-on-write
into a fresh arena with a `remap: Vec<Option<usize>>` — the proven
`apply_desugar` algorithm, generalized. `Fixpoint` tracks a dirty range of
rewritten indices instead of re-walking the whole tree.

Rule lookup is a `HashMap<&str, Vec<&TransformDef>>` built once per pass —
one string lookup per node, sorted by `priority`. No `kind ==` anywhere in
the engine; all shape knowledge comes from `AstShapeDef` roles.

### 5. `RuleSource` seam

```rust
pub trait RuleSource {
    fn rules_for(&self, lang: &Language, pass: &str) -> Vec<TransformDef>;
}
```

Default `LanguageRuleSource` reads `lang.transforms`. An alternate impl
loads rules from a codemod file or extra YAML fragment set *without*
merging into the `Language` — so "the language ships its own lowering" and
"a user runs an ad-hoc codemod" are the same machinery (mirrors
`TestProvider` in `plugin-engine-test`).

### 6. Desugar migrates onto the engine

`handl-runtime::desugar_pass` becomes a thin delegate:

```rust
pub fn apply_desugar(lang: &Language, ast: &Ast) -> Result<Ast, LangError> {
    let engine = RewriteEngine::new().with_desugar_natives();
    let root = AstRef::new(ast.len() - 1);
    engine
        .run_pass(lang, ast, root, "desugar")
        .map(|out| out.ast)
}
```

- `AstShapeDef.desugar: AstDesugarRule` fields are **lowered to
  `TransformDef`s at `Language::merge` time** (pass `"desugar"`), so
  `data/ast.yaml` needs no edits in phase 1. First-class transform rules
  now live in `data/transforms.yaml` — a `Vec<TransformDef>` merged into
  `lang.transforms` after the ast pass (so a first-class rule may
  `replaces:` a lowered one). The `desugar:` field and its lowering remain
  for backward compatibility with third-party fragments.
- `try_rust_desugar` (`while`/`do-while`/`for`/`nullish`/`opt-chain`,
  ~4000 lines) moves verbatim into
  `plugin-engine-transform-rewrite::native::desugar` behind
  `NativeRewrite`. Each gets a `TransformDef { node_kind, rule:
  Native("desugar.<form>") }` — dispatch data-driven, implementation Rust.
- Two behaviors are preserved exactly: bottom-up insertion-order walk, and
  **span inheritance** — a rewritten node keeps the original node's `span`
  (today's `build_desugar` does this; it becomes the engine's documented
  span policy for `Keep`-derived and `Declarative` rewrites).

### 7. Crate layout

```text
plugin-engine-transform/
  src/lib.rs        Transform marker, TransformEngine, Match
  src/request.rs    TransformRequest, TransformMode
  src/output.rs     TransformOutput, RewriteTrace, Rewrite
  src/source.rs     RuleSource, LanguageRuleSource
  src/native.rs     NativeRewrite trait, RewriteCtx

plugin-engine-transform-rewrite/
  src/lib.rs        RewriteEngine + run_pass/run_pipeline/apply_rule/
                    find_matches
  src/dispatch.rs   pass → kind → Vec<TransformDef> table
  src/walk.rs       bottom-up/top-down/fixpoint arena walker
  src/step.rs       RewriteStep interpreter
  src/condition.rs  TransformCondition evaluator
  src/native/desugar.rs   moved try_rust_desugar
  tests/desugar_parity.rs golden tests vs. pre-migration apply_desugar

handl-language/
  src/fragments/transform_def.rs   TransformDef (new)
  src/fragments/…                  TransformRule, RewriteStep,
                                   TransformCondition (data)
```

Dependency graph (no cycles):

```text
handl-language                   TransformDef + RewriteStep +
                                 TransformCondition (data)
plugin-engine-transform          Transform/Request/Engine/Trace/Source
plugin-engine-transform-rewrite  RewriteEngine + native registry
handl-runtime                    calls RewriteEngine (drops
                                 try_rust_desugar)
```

## Consequences

### Positive

- **Any pass, not just desugar.** `lower`/`optimize`/codemod/LSP refactors
  become `TransformDef` data + a `run_pass` call — no new engine code.
- **Genericity rule satisfied.** Engine dispatch is `lang.transforms[pass]
  [kind]`; child access is role-based; the `try_rust_desugar` `kind ==`
  side channel moves behind a named, data-dispatched registry.
- **Free provenance.** `RewriteTrace` gives diagnostics and the LSP
  input→output node mapping at zero cost.
- **Matcher reuse.** `find_matches` exposes the match half — "find all
  nodes matching shape X" — to lint and LSP without instantiating a
  rewrite.
- **Future impls** (`-parallel` — immutable `Ast` in makes independent
  subtrees embarrassingly parallel; `-streaming` for large arenas) plug in
  behind the same trait.
- **Zero behavioral change** at migration: the existing desugar test suite
  is the conformance suite, plus a parity test asserting `run_pass
  ("desugar")` output is identical to the pre-refactor `apply_desugar`.

### Negative / costs

- **Two more crates** and a fragment type on `Language` (which grows a
  fourth dispatch table after `eval`/`types`/`grammar`).
- **`NativeRewrite` is a stable ABI seam** — `RewriteCtx` must be designed
  once, carefully, since natives depend on it.
- **Deferred work is explicit:** see "Deferred" below — most originally
  deferred items (`TopDown`/`Fixpoint`/`ApplyAt`/`Plan`/`Splice`/`Apply`/
  `PayloadEq`, and the `desugar:` → first-class `transforms.yaml` table)
  have since landed.
- **`handl-language` gains rule-vocabulary types** — a slight broadening
  of "pure data" (the vocabulary is data; only `plugin-engine-transform`
  has logic). Same trade-off already made for `Scheme`.

### Deferred (explicitly out of scope)

- `plugin-engine-transform-parallel`.

Landed after initial merge (originally deferred): the `hnl transform` CLI
subcommand (`host-cli`):

```text
hnl transform <pass> <file.hnl> [--pipeline a,b,c] [--plan] [--trace]
          [--stats] [--emit tree|json]
```

It parses each top-level statement RAW (bracket-depth-zero split + Pratt
engine — no `Runtime::parse`, which would pre-desugar per ADR-0082), runs
the pass via `RewriteEngine::run_pass`/`run_pipeline` with
`with_desugar_natives`, and renders the output arena as an indented tree
(`kind` + payload summary) or `--emit json`. `--plan` lists match sites
(kind + rule id, incl. `native:desugar.*` fallback sites) without
applying; `--trace` prints each `Rewrite`; `--stats` prints `PassStats`.
Exit codes: 0 success, 1 transform failure, 2 usage/parse error — an
unknown pass name (no `TransformDef` declares it) is a usage error, not a
silent no-op. A regression test asserts raw-parse + `desugar` produces an
arena identical to `Runtime::parse`'s output.

Landed after initial merge (originally deferred): the `ast.yaml` `desugar:`
field is superseded by a **first-class `transforms.yaml` data file** — a
`Vec<TransformDef>` merged into `lang.transforms` by
`handl-kit-data-canonical`'s `merge_fragments`/`language_from_yaml`/
`language_from_dir` (optional on disk; absent = no first-class rules) and
by the `transforms:` section of sectioned fragment docs (e.g.
`plugin-engine-test`'s test-language fragment). The canonical `call` →
`json` and `entry` computed-key rules are authored there using the
`RewriteStep`/`TransformCondition` vocabulary (`!Keep`, `!KeepRest`,
`!Create`, `!KeepPath`, `!KindAt {index, kind}`). `AstShapeDef.desugar` and
its merge-time lowering are kept — third-party fragments may still declare
`desugar:` on a shape.

Implemented at landing (originally deferred): `Traversal::TopDown`,
`Traversal::Fixpoint` (with `max` iterations + a node-count budget —
`RewriteEngine::with_node_budget`, default `MAX_NODES`), `TransformMode::
ApplyAt`, `TransformMode::Plan`, `RewriteStep::Splice`, `RewriteStep::
Apply`, `TransformCondition::PayloadEq`, and the shallow `payload` patch
on declarative rewrites.

## Production readiness

The architecture above is production-grade; the following requirements
make the *implementation* production-grade rather than MVP. They are part
of the decision, not follow-ups.

### Correctness

- **No silent fallthrough.** `apply_desugar`'s replacement returns
  `Result<Ast, LangError>`; a failed transform surfaces a diagnostic with
  `pass`/`rule_id`/`node_kind`/`span`, never the un-transformed AST.
  `Runtime::parse` propagates it.
- **Determinism.** Two `TransformDef`s that can both fire on the same
  node at the same priority in the same pass are a **merge error**, not a
  silent first-pick. Identity key `pass:node_kind:priority` enforces this
  structurally; `validate_transforms` checks condition-overlap where
  statically decidable.
- **Termination discipline.** `Fixpoint` passes carry `max` *and* a
  node-count budget; `validate_transforms` warns when a rule's
  `target_kind` is re-matchable in the same pass (potential rewrite
  cycle) or when a rewrite cycle exists across a pass's rules.
- **Rule-set validation** (`validate_transforms(lang) -> Vec<Diagnostic>`,
  in the abstraction crate): every `target_kind`/`Create.kind` exists in
  `lang.ast`; `Keep(i)`/`KeepRole`/`KeepPath` indices are in bounds for
  the shape's declared arity/roles; `Native(name)` resolves in the wired
  registry; `Apply` targets an existing rule id. Runs at `Language` build
  (wired in `handl-kit-data-canonical::merge_fragments` — Error-severity
  diagnostics fail assembly; warnings do not, since shapes and native
  registries may arrive from other fragments/engines). Wiring `hnl check`
  to re-report these diagnostics remains a follow-up — its bootstrap path
  in `plugin-engine-typecheck` merges `ast.yaml` shapes directly (without
  transform lowering), so the build-time check in `merge_fragments` is the
  enforcing site today.

### Errors & diagnostics

- Transform failures produce `plugin-ext-diag` `Diagnostic`s
  (`{ pass, rule_id, node_kind, span, why }`), not bare `LangError`s —
  `hnl` renders them like lint/typecheck failures.

### Spans & payloads

- Documented span policy: `Keep`-derived and `Declarative` rewrites
  inherit the source node's `span`; `Create`d nodes get `None` and are
  reachable via `RewriteTrace` so a diagnostic can attribute them to the
  rule that made them.
- `Declarative.payload` is a **shallow patch**: provided keys overwrite,
  absent keys are preserved from the original node's payload. Documented,
  tested, and the only semantics.

### Observability

- `TransformOutput` carries `PassStats { visited, rewritten, per_rule:
  BTreeMap<rule_id, usize>, duration }` alongside `RewriteTrace` — dead
  rules and hot passes are visible without instrumentation.

### Performance

- Stated bounds: O(nodes × rules-per-kind) per pass, O(nodes) memory for
  `remap`, single `HashMap<&str, _>` lookup per node. A criterion bench
  (large module, rule-count scaling) lands with the impl — the
  "performant" claim is measured, not asserted.

### Testing

- Parity suite vs. pre-migration `apply_desugar` (byte-for-byte arena
  compare on every existing desugar fixture).
- Idempotence property: `desugar(desugar(x)) == desugar(x)` on all
  fixtures.
- Adversarial rule tests: self-matching rules, infinitely-growing
  rewrites under `Fixpoint` (must hit `max`/budget, not hang),
  overlapping-priority rejection.
- Fuzzed `when` conditions over random ASTs — matcher never panics.

### Versioning

- `TransformDef` participates in the same `SchemeVersion`/fragment-compat
  discipline as other `Language` fragments; a rule authored against a
  shape version that changes arity is caught by `validate_transforms`,
  not silently misapplied.

## Migration sequence

1. `handl-language`: `TransformDef` fragment + `TransformRule`/
   `RewriteStep`/`TransformCondition` vocabulary + `Language.transforms`
   table + `AstShapeDef::desugar → TransformDef` lowering at merge time.
2. `plugin-engine-transform`: marker, request, output/trace, trait,
   `RuleSource`, `NativeRewrite`/`RewriteCtx`.
3. `plugin-engine-transform-rewrite`: `RewriteEngine` — the `apply_desugar`
   walk generalized; `native/desugar.rs` receives `try_rust_desugar`
   verbatim.
4. `handl-runtime`: `apply_desugar` returns `Result` and delegates to
   `run_pass("desugar")`; `try_rust_desugar` deleted; `Runtime::parse`
   propagates transform errors.
5. `validate_transforms` + `PassStats` + diagnostic error paths wired.
6. `cargo test --workspace` green; `desugar_parity.rs` asserts identical
   output vs. the pre-migration implementation; criterion bench lands.

## Second consumer: lint

The canonical v0.1 linter (`handl-plugin-ext-lint`) was the first non-desugar
use of the transform engine.  The exercise confirmed the engine generalizes as
a **pattern matcher** but also exposed why lint fixes cannot (yet) be pure
AST rewrites in this codebase.

### Findings

The three fixable lint rules in the v0.1 rule set are all fundamentally
text-level edits:

- `fmt` — whole-file canonical reformatting.
- `unused-import` — delete a top-level statement line, including its trailing
  newline.
- `naming-convention` — rename an identifier token.

None of them is a structural rewrite that can be expressed with the current
`RewriteStep` vocabulary.  `naming-convention` needs payload string
manipulation (`snake_case`); `unused-import` needs statement deletion;
`fmt` is a token-stream operation.  More importantly, the v0.1 Pratt AST
carries **no source spans**, and there is no span-preserving AST→source
renderer (the formatter emits canonical text, not the original layout).  A
pure `run_pass("lint")` + re-print would therefore discard comments,
whitespace, and line breaks.

### Chosen bridge

The honest minimal win is to keep the existing text-fix pipeline and use
`RewriteEngine::find_matches` as the lint's structural matcher for new,
pattern-based rules.  This satisfies the engine-genericity rule: dispatch is
`lang.transforms["lint"][kind]` and child positions resolve through
`AstShapeDef` roles; no new `kind ==` checks are added.

A demonstrative rule, `lint.simplify-paren`, lives in the embedded
`crates/plugin-ext-lint/data/lint_transforms.yaml` file and is merged into the
canonical `Language` at lint time.  It matches `paren` nodes with a single
`var`/`value` child via `find_matches`; the actual source edit is then a
span-preserving text replacement.  This proves the engine generalizes beyond
desugar without forcing a bad fit for the existing text-based fix rules.

### Consequence

When an AST-level lint fix eventually becomes desirable (e.g. a structural
codemod), the path is: define a `lint` pass rule and use `find_matches` (or
`RewriteEngine::run_pass` once the AST carries spans and a formatting-preserving
renderer exists).  The existing v0.1 rules remain text edits.

## Open questions

1. `TransformDef` identity key `pass:node_kind` — or does `priority` need
   to participate in identity (multiple same-priority rules per kind)?
   Current answer: identity is `pass:node_kind:priority`, first-match wins
   within a kind.
2. Should `RewriteTrace` be optional (zero-cost opt-out for the hot eval
   path)? Default: always collected — it's a `Vec` append — revisit if
   profiling shows it.
3. Does `Plan` mode belong on the trait or as a `RewriteEngine`-only
   method? On the trait for now so `-parallel` can offer it.

## Follow-up: a `lower` pass for codegen backends

The first concrete `lower` rule is `transform.lower.entry` in the canonical
`transforms.yaml`.  It matches an `entry` node whose `key` child is a `string`
literal and rewrites it to use the canonical `var` literal key shape.  This
removes the `var`/`string` object-key heuristic that the bytecode and
cranelift backends currently keep as a fallback in their compile/runtime
paths.

The rule is data-driven:

```yaml
- id: transform.lower.entry
  pass: lower
  node_kind: entry
  when: !KindAt
    role: key
    kind: string
  rule: !Native "lower.object_key"
```

Because the rewrite must copy the original `string` payload into a newly
allocated `var` leaf, the rule is `Native`.  The implementation lives in the
backend that opts in to lowering (`handl-plugin-engine-exec-bytecode` registers
`lower.object_key` and calls `RewriteEngine::run_pass(lang, ast, root,
"lower")` from `BytecodeEngine::eval`).  This keeps the canonical `Language`
free of backend-specific payload-manipulation rules while proving that the
pass machinery, dispatch, and native seam are reusable beyond `desugar`.

`BytecodeEngine` exposes the pass as an opt-in (`with_lowering()`), and the
bytecode conformance suite includes a parity test that runs the full eval
corpus with and without the `lower` pass, ensuring the normalization is
semantics-preserving on the existing eval surface.

## References

- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — language as
  data; `Fragment`/`Language::merge`.
- [ADR-0053](./0053-generic-pratt-parser.md) §7 — the
  `AstDesugarRule`/`AstDesugarChild` machinery this generalizes.
- ADR-0058, ADR-0060 — `when` conditions; `KeepRest`.
- [ADR-0081](./0081-data-driven-ast-child-roles.md) — named child roles;
  `role_index`.
- ADR-0082 — desugar pipeline ordering (parse-time placement is
  preserved).
- `crates/plugin-engine-exec` — the abstraction/impl split this follows.
- `crates/handl-runtime/src/desugar_pass.rs` — the code being migrated.
