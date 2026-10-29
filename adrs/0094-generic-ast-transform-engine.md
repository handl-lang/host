---
title: Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)
type: adr
date: 2026-09-06
status: accepted
status_note: |-
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
---

# ADR-0094: Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)

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
   (ADR-0082 Phase 1.2). There is no way to define a _second_ transform —
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
   yet, so an escape hatch must exist _inside_ the data-driven dispatch.

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
loads rules from a codemod file or extra YAML fragment set _without_
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
  (The five fixed-shape forms were later migrated to declarative
  `TransformDef`s in `transforms.yaml`; only `data`/`match` remain native
  — see the addendum below.)
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

`plugin-engine-transform-parallel` has been implemented and is no longer
outstanding.

The layout-preserving AST→source emitter has been implemented in
`handl-plugin-engine-fmt` as `PreserveEmitter` and is no longer out of scope.

### Implemented: `plugin-engine-transform-parallel`

`plugin-engine-transform-parallel` provides a parallel `TransformEngine`
implementation that partitions a bottom-up pass at the root's top-level
children. Each child subtree is processed by a cloned `RewriteEngine` running in
a `std::thread::scope` worker; the child output arenas are merged with offset
remapping, and the root itself is rewritten sequentially via
`RewriteEngine::run_pass_root`. This required making `NativeRewrite` and
`RuleSource` `Send + Sync` and switching `RewriteEngine` shared state from `Rc`
to `Arc`.

The parallel implementation passes a parity suite asserting byte-for-byte output
arena equality with `RewriteEngine` on generated module ASTs containing `while`,
`nullish`, and `opt-chain` statements. The largest real `.hnl` modules in the
repository (`handl-packages/str-handl/str-handl.hnl`, ~760 lines) are far below
the size where parallelism pays off, so the Criterion benchmark uses synthetic
mixed modules to sweep the desugar pass from 500 to 50 000 top-level statements.
On those inputs (release profile) the sequential engine is faster at every size
measured: ~0.96 ms vs ~3.47 ms at 500 statements, ~5.97 ms vs ~14.04 ms at 2 000,
~11.96 ms vs ~22.79 ms at 5 000, ~36.12 ms vs ~56.76 ms at 10 000, and ~202.5 ms
vs ~373.3 ms at 50 000. There is no measured crossover; the thread spawn,
subtree extraction, and per-worker arena merge overhead dominate the available
work for currently reachable inputs. `ParallelEngine`'s default threshold is
therefore set to 1 000 000 AST nodes, which keeps it on the sequential fallback
for all realistic workloads while preserving the implementation as a correct,
swappable `TransformEngine`. The merge step is the largest single bottleneck —
each worker produces a fresh arena that must be offset-remapped into a new shared
arena — so a shared-arena or zero-copy merge design is the most promising lever
if a workload ever justifies parallel desugar.

Landed after initial merge (originally deferred): the `hnl transform` CLI
subcommand (`host-cli`):

```text
hnl transform <pass> <file.hnl> [--pipeline a,b,c] [--plan] [--trace]
          [--stats] [--emit tree|json|source] [--write]
```

It parses each top-level statement RAW (bracket-depth-zero split + Pratt
engine — no `Runtime::parse`, which would pre-desugar per ADR-0082), runs
the pass via `RewriteEngine::run_pass`/`run_pipeline` with
`with_desugar_natives`, and renders the output arena as an indented tree
(`kind` + payload summary), `--emit json`, or a preserving source rendering
(`--emit source` / `--write`) using `handl-plugin-engine-fmt::PreserveEmitter`.
`--plan` lists match sites (kind + rule id, incl. `native:desugar.*` fallback
sites) without applying; `--trace` prints each `Rewrite`; `--stats` prints
`PassStats`.
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

Landed after initial merge: **the fixed-shape desugar natives went
declarative.** `while`, `do-while`, `for`, `nullish`, and `opt-chain` were
migrated from Rust `desugar.*` handlers to `TransformDef`s in
`transforms.yaml` — the audit's assumption that they needed
fresh-name generation was wrong (their `__continue`/`__result`/`__xs`/
`rec_v`/`rec_tail` binders are fixed `__`-namespace names, not gensyms)
and no `Apply`-style recursion was needed either (bottom-up
`Keep`/`KeepRest` already resolves children to their _rewritten_
counterparts via the `remap` table). Two findings of note:

- serde_yaml cannot deserialize an enum nested inside
  `TransformCondition::Not` (a newtype-variant limitation), so the
  `while` state-passing guard is expressed as a higher-priority
  identity rule (`transform.desugar.while.state_form`, `priority: -1`,
  `when: KindAt{index:1, kind:"function"}` → `while → while [Keep 0,
Keep 1]`), which produces output identical to a pass-through.
- The unregistered `desugar_if` (`if`→`match`, never part of the pass
  because `type.if` must observe `if` nodes) was deleted — it is
  trivially a `Declarative` rule and its coverage was duplicated by the
  declarative `if`→`match` lowering tests.

`DESUGAR_NATIVES` now holds only `data` and `match`→recursor
(`desugar.data`, `desugar.match_recursor`). Those two are **irreducibly
semantic**: `desugar_data` parses type expressions out of node payloads,
iterates a variable-length constructor list to generate predicate /
constructor / recursor functions (including `termination` metadata and
strict-positivity rejection), and `desugar_match_to_recursor` searches
the whole input arena for `data` declarations to resolve constructor
names and arity-check arms. Expressing either would require
constructor-list iteration, payload-string parsing, and environment
lookup — a macro system, not a rewrite step. They stay native behind the
same disciplined `desugar.<form>` registry.

Implemented at landing (originally deferred): `Traversal::TopDown`,
`Traversal::Fixpoint` (with `max` iterations + a node-count budget —
`RewriteEngine::with_node_budget`, default `MAX_NODES`), `TransformMode::
ApplyAt`, `TransformMode::Plan`, `RewriteStep::Splice`, `RewriteStep::
Apply`, `TransformCondition::PayloadEq`, and the shallow `payload` patch
on declarative rewrites.

## Production readiness

The architecture above is production-grade; the following requirements
make the _implementation_ production-grade rather than MVP. They are part
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
- **Termination discipline.** `Fixpoint` passes carry `max` _and_ a
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
  registries may arrive from other fragments/engines) _and_ under the CLI
  check path: `hnl typecheck` (the `hnl check` surface) builds its
  `Language` in `plugin-engine-typecheck::bootstrap`, which cannot depend
  on `handl-kit-data-canonical` (cycle via `handl-runtime`), so it embeds
  the same nine YAML files and mirrors `merge_fragments` —
  `merge_ast_fragment` lowering, the `transforms.yaml` merge, and
  `validate_transforms`. Error-severity diagnostics there surface as
  `CheckError::Language` ("language error: transform rule validation
  failed: …") on stderr with exit code 2, instead of a panic or a check
  run against a malformed language.

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

### Splice-preserving emitter

The `PreserveEmitter` in `handl-plugin-engine-fmt` renders a transformed AST
back to surface syntax while keeping the user's original formatting and
comments wherever possible. It reuses the data-driven `FormatEngine` for all
layout decisions and walks the output AST top-down:

- A node that is not in the `RewriteTrace` and whose entire subtree also
  contains no rewritten node, and that still carries a Pratt-parser span, is
  emitted as `src[span.start..span.end]` — copying the original bytes,
  including any comments or whitespace inside the span.
- Any node that was produced by a rewrite, has `span: None` (created by a
  `Create` step), or has a changed descendant is routed through the same
  data-driven `FormatEngine` formatting as a plain format call. The recursive
  child rendering still consults the preserving emitter, so unchanged children
  of a rewritten outer node are spliced while the changed children are
  reformatted.

Change detection is trace-driven: `PreserveEmitter::new` is passed the set of
`to` `AstRef`s from `TransformOutput.trace.rewrites` and precomputes a
`has_changed` flag for every reachable node (itself in the set or any
transitive child in the set). This is safe because ADR-0094's span policy
already guarantees that `Keep`-derived and declarative rewrites inherit the
original source span; only created sub-trees and explicit rewrite targets are
formatted.

At the CLI level, statements with no rewrites are emitted by copying their
entire trimmed source chunk (`src[start..end]`) verbatim, while statements that
were rewritten are formatted by `PreserveEmitter` and placed back at their
original location. Gaps between statements (`src[0..first_start]` and
`src[end_i..start_{i+1}]` and `src[last_end..]`) are preserved, so leading
comments, blank lines, and trailing text survive.

`--emit source` and `--write` are not appropriate for lowering passes that
create internal-only nodes (e.g. `desugar` producing `rec`, `if__continue`,
tagged-record recursors). The CLI preflights the output AST: any created
(`span: None`) node whose kind has no grammar row and no leaf payload is
considered un-emittable and the command refuses with an error. Only
surface-preserving passes can be turned back into usable source text.

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
use of the transform engine. The exercise confirmed the engine generalizes as
a **pattern matcher**; with `AstNode::span` now populated by the Pratt parser,
the lint-to-fix chain is fully AST-derived for pattern-based rules. A
layout-preserving AST→source emitter is now available in
`handl-plugin-engine-fmt::PreserveEmitter` for use by the transform CLI and
can be adopted by the LSP lint fix path.

### Findings

The three fixable lint rules in the v0.1 rule set are all fundamentally
text-level edits:

- `fmt` — whole-file canonical reformatting.
- `unused-import` — delete a top-level statement line, including its trailing
  newline.
- `naming-convention` — rename an identifier token.

None of them is a structural rewrite that can be expressed with the current
`RewriteStep` vocabulary. `naming-convention` needs payload string
manipulation (`snake_case`); `unused-import` needs statement deletion;
`fmt` is a token-stream operation. There is still no span-preserving
AST→source renderer (the formatter emits canonical text, not the original
layout), so a pure `run_pass("lint")` + re-print would discard comments,
whitespace, and line breaks. The `simplify-paren` rule therefore stays a
text replacement, but the text is now cut directly from the matched node spans.

### Chosen bridge

The honest minimal win is to keep the existing text-fix pipeline and use
`RewriteEngine::find_matches` as the lint's structural matcher for new,
pattern-based rules. This satisfies the engine-genericity rule: dispatch is
`lang.transforms["lint"][kind]` and child positions resolve through
`AstShapeDef` roles; no new `kind ==` checks are added.

A demonstrative rule, `lint.simplify-paren`, lives in the embedded
`crates/plugin-ext-lint/data/lint_transforms.yaml` file and is merged into the
canonical `Language` at lint time. It matches `paren` nodes with a single
`var`/`value` child via `find_matches`; the source fix then replaces the
`paren` node's `span` with the source text under the matched child's `span`.
This proves the engine generalizes beyond desugar and that the match
→ span → edit chain is now genuinely AST-derived.

### Consequence

When an AST-level lint fix eventually becomes desirable (e.g. a structural
codemod), the path is: define a `lint` pass rule and use `find_matches` (or
`RewriteEngine::run_pass` once a formatting-preserving renderer exists). The
existing v0.1 rules remain text edits.

## LSP consumer: code actions

`handl-host-lsp` now offers transform-engine-driven quick fixes in
`textDocument/codeAction` (`crates/host-lsp/src/code_actions.rs`,
`lint_fix_actions`). The handler runs
`handl_plugin_ext_lint::lint_source` over the open document; every lint
diagnostic that carries a `Fix` and intersects the requested range becomes a
`quickfix` `CodeAction` whose `WorkspaceEdit` applies the fix's text edit.
For `lint.simplify-paren` — whose match sites come from
`RewriteEngine::find_matches` — this realizes the "find pattern → show diff →
apply at node" loop end to end: the action is titled "Remove redundant
parentheses" and its edit replaces the parenthesized span with the inner
atom's text.

The match comes from `RewriteEngine::find_matches`; the default fix is still
a span→text replacement for speed. The edit range is taken from the
`Fix`/`Span` the linter computes from `AstNode::span` (the matched `paren`
node and its inner child), so the match → span → edit chain is now fully
AST-derived.

A second `quickfix` action is also offered for `lint.simplify-paren`:
`RewriteEngine::find_matches` locates the matched `paren` site, then
`RewriteEngine::apply_rule` is run against a companion `lsp-rewrite` pass
rule and the transformed statement is re-emitted with
`handl-plugin-engine-fmt::PreserveEmitter`, producing a `WorkspaceEdit` that
replaces the whole statement. This exercises the `find_matches` →
`apply_rule` → `RewriteTrace` → `PreserveEmitter` bridge end-to-end while
leaving the existing text fix in place as a lighter-weight alternative.

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
`transforms.yaml`. It matches an `entry` node whose `key` child is a `string`
literal and rewrites it to use the canonical `var` literal key shape. This
normalizes the `var`/`string` object-key heuristic that the bytecode and
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
allocated `var` leaf, the rule is `Native`. The implementation lives in the
backend that opts in to lowering (`handl-plugin-engine-exec-bytecode` registers
`lower.object_key` and calls `RewriteEngine::run_pass(lang, ast, root,
"lower")` from `BytecodeEngine::eval`). This keeps the canonical `Language`
free of backend-specific payload-manipulation rules while proving that the
pass machinery, dispatch, and native seam are reusable beyond `desugar`.

### Backend normalization audit

An audit of the two codegen backends (`plugin-engine-exec-bytecode` and
`plugin-engine-exec-cranelift`) was performed to find `kind ==`/shape
heuristics that could be retired by the `lower` pass. The candidates flagged
in `AGENTS.md` were:

- **Object-entry key `var`/`string` heuristic** — found in
  `plugin-engine-exec-bytecode/src/compile.rs` (`try_extract_static_object_keys`),
  `plugin-engine-exec-bytecode/src/vm.rs` (`object` native op), and
  `plugin-engine-exec-cranelift/src/lib.rs` (object helper). This is the
  only heuristic that reaches the codegen backends in a form the `lower` pass
  can remove: the existing `transform.lower.entry` + `lower.object_key` native
  rewrite `string` keys to the canonical `var` shape before the backend sees
  the AST.
- **`match`/`arm`/`block`/`paren`/`type-annot` desugared-vs-raw assumptions** —
  no residual shape heuristics found in the codegen paths. `match` is
  dispatched as a native op, `paren` wrappers are already removed by the
  `desugar` pass, and `block`/`type-annot` handling is role-based.
- **`nullish`/`opt-chain` remnants** — none reach the backends un-desugared;
  the `desugar` pass rewrites them before evaluation.
- **Constant folding (e.g. `binary.add` of numeric literals)** — skipped. The
  canonical literal shape and eval scheme for `value` nodes are not
  unambiguous enough in the current data to make a semantics-preserving
  rewrite that does not duplicate backend arithmetic; the `binary.add` scheme
  is evaluated as the `add` native op. No rule was added for this.

The audit produced **no additional `lower` rules** beyond `transform.lower.entry`.
The object-key `var`/`string` fallback in the backends is therefore kept but
now explicitly commented as redundant under `BytecodeEngine::with_lowering()`;
removing it entirely would break the non-lowering path and any role-less
`entry` shapes. A unit test (`lower_object_key_bypasses_string_kind_heuristic`)
proves the `lower` pass rewrites `string` keys to `var` so the heuristic is
not exercised.

### Cranelift parity

`CraneliftEngine` does not yet opt in to the `lower` pass. Wiring it would
require adding `plugin-engine-transform-rewrite` as a dependency, which risks
introducing a dependency cycle: `handl-runtime` already depends on
`plugin-engine-exec-cranelift`, and `plugin-engine-transform-rewrite` has a
dev-dependency on `handl-runtime`. Cranelift therefore retains the same
`var`/`string` kind fallback with a cross-reference to this ADR section.

## Handl-authored transforms (the `handl:` native seam)

Symmetric to `Native(name)` reaching into Rust, a `TransformDef` can name a
`.hnl` function as its rewrite. The design keeps `TransformRule::Native` as
the single dispatch spelling — no `TransformRule::Handl` variant was added,
so `handl-language`, `validate_transforms`, and the serde schema are
untouched. The convention is a `handl:` name prefix:

```yaml
- id: transform.mypass.foo
  pass: mypass
  node_kind: foo
  rule: !Native "handl:mymod.myfn"
```

`handl:mymod.myfn` resolves to function `myfn` on the `HandlModule`
registered under `mymod` with
`handl_plugin_lib_handl::transform::HandlNativeRewrite` — a `NativeRewrite`
impl living in `plugin-lib-handl`, where `.hnl` evaluation is reachable
without a dependency cycle (the rewrite engine cannot depend on
`handl-runtime`/`plugin-lib-handl`). Invocation mirrors the `pred.hnl`
filter-arrow pattern: the node record crosses as an `__arg_0` global through
`HandlFunction::call` and the result returns across the serde_json boundary.
Non-`handl:` names delegate to an optional fallback registry, so one
`with_native_registry` call serves both handl rules and Rust natives.

**Value contract.** The `.hnl` function receives one argument — a recursive
serialization of the matched node:

```text
{ kind: "foo",
  span: { start: 0, end: 3 } | null,
  payload: { … },                    -- the node's payload minus `children`
  children: [ <node record>, … ] }   -- recursively serialized children
```

and returns either `null` (decline → the engine pushes the node through with
remapped children) or a **node spec** `{ kind, payload?, children? }` that the
engine realizes into the output arena. Each `children` element is itself a
node spec or the marker `{ "__keep__": n }`, which reuses the remapped output
of the _matched_ node's child `n` (the index always refers to the matched
node's `children` array, no matter how deeply nested the marker appears). A
bare `{ "__keep__": n }` return replaces the matched node with that child.
Realized nodes inherit the matched node's `span`, matching the documented
span policy.

**Rough edges.** The `.hnl` function cannot observe raw arena indices —
`__keep__` is the only handle onto existing subtrees; results must not
contain arrow values or other reserved-key (`__arrow__`/`__tag__`/…)
shapes; and an arrow body that is a record literal must be parenthesized
(`n ~> ({…})`) since a bare `{k: v}` record parses `k:` as a type
annotation. The bridge runtime evaluates `(source).fn(arg)` against its own
canonical `Language`, so user `handl:` rules never re-enter the transform
engine. End-to-end coverage: `plugin-lib-handl/tests/handl_transform.rs`.

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
