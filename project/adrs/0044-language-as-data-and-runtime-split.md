# ADR-0044: Language as Data and Runtime Split

## Status

Accepted

Implementation complete: 37 `handl-emitter-*` crates emit token, AST, grammar,
and eval fragments assembled by `handl-kit-data-canonical`; 84 old
`plugin-token-*`/`plugin-lexer-*`/`plugin-ast-*`/`plugin-grammar-*`/`plugin-eval-*`
crates deleted; `host-tooling` semantic tokens migrated to string-based kind
classification; `plugin-engine-lex-pattern` provides data-driven lexing;
`plugin-engine-exec-{tree-walk,bytecode,cranelift}` interpret eval schemes.
All 2900 workspace tests pass.

The lingering `handl-plugin-ast` crate (the trait-based `Node<K, V, M>` AST
from ADR-0023, later renamed from `handl-ast-common`) has been deleted. It
had no source-level consumers after the data-driven migration: the parser
and eval engines depend on `handl-language`'s `Ast` arena, not the trait
graph. Its only remaining footprint was stale cargo deps and doc references.
See "Supersession of the trait-based AST" below.

Refines [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md):
makes its `Language` value **pure data**, corrects its `eval` signature to
operate on an `Ast` rather than `Source`, and specifies the crate structure
that implements both. Builds on [ADR-0039](./0039-kits-are-curated-plugin-lists.md)
and [ADR-0040](./0040-vm-is-an-operation-not-a-subsystem.md).

## Context

ADR-0041 established the vision: `Language` is a first-class handl value,
`eval` is a handl function, `self` enables self-hosting, capability discipline
lives at the language level. It left three things unspecified, and one of its
signature choices is wrong:

1. **`eval : Language -> Source -> Value` couples the evaluator to lexing and
   parsing.** An evaluator that takes `Source` must lex and parse before it can
   evaluate. That forbids a runtime that only evaluates (a JIT/cranelift
   runtime that compiles a pre-parsed `Ast`), forbids evaluating the same
   program multiple times without re-parsing, and forbids AST-level
   transformation/analysis between parse and eval. The evaluator should consume
   the previous stage's output, not the original text.

2. **`Language` is described as a value but not as _data_.** ADR-0041's
   `language { lexer: ..., grammar: ..., eval: ... }` is illustrated with
   handl-level builder functions (`grammar.from-rules`, `eval.from-behaviors`).
   If those builders are Rust, the language definition is still Rust — just
   wrapped. The defining property of "language as data" is that the definition
   carries no Rust: a `Language` is serializable, inspectable, authorable as
   text, and loadable by a runtime that has no compile-time knowledge of it.
   That is what makes self-hosting real (a `Language` value can describe handl
   itself, loaded as data) and what makes `compile : Language -> Target` a
   function on data rather than on Rust trait objects.

3. **The rule crates are Rust, not data.** Today `plugin-grammar-arith` is
   `impl ParserBehavior` — a Rust struct with a `prefix`/`infix` method.
   `plugin-eval-call` is `impl EvalBehavior`. These Rust impls _are_ the
   language definition. If `Language` is data, these crates either stop being
   the definition (they become engines that _consume_ data) or are deleted in
   favor of data rows. ADR-0041 did not resolve this.

4. **No crate structure was specified** for separating the operation
   abstractions (`Lexer`/`Parser`/Evaluator traits) from the engines that
   implement them (Pratt, tree-walking, bytecode, cranelift). Today
   `plugin-ext-parser` bundles the `Parser` operation _and_ the Pratt engine in
   one crate. Swapping Pratt for chumsky means forking the crate.

### The composability constraint

The platform's most valued property is that tiny lex/grammar/eval pieces
compose naturally: include/exclude, kits, optional/requires, topological
ordering. Today this is _runtime_ composition — plugins register into a
`PluginRegistry`, kits manifest them, include/exclude filters them.

If `Language` becomes pure data, composition must become **data composition**
or that property regresses. A `Language` that is a single opaque blob cannot
be curated the way a kit curates plugins. The data model must preserve
fragment-level composition: a kit still selects tiny pieces, but the pieces are
data fragments that merge into a `Language`, not Rust plugins that register
into a registry.

## Decision

### 1. `Language` is pure serializable data, built by merging fragments

A `Language` is a mergeable store of typed fragments. Each fragment is one
tiny concern — one token kind, one grammar row, one eval scheme, one
capability declaration, one option schema. Composition is `Language::merge`,
applied in kit-defined order.

```rust
// handl-language
pub struct Language {
    pub tokens:    FragmentSet<TokenKindDef>,
    pub grammar:   FragmentSet<GrammarRow>,
    pub eval:      FragmentSet<EvalSchemeDef>,
    pub ast:       FragmentSet<AstShapeDef>,
    pub caps:      FragmentSet<CapabilityDecl>,
    pub options:   FragmentSet<OptionSchema>,
}
```

`FragmentSet<T>` is a mergeable collection with an explicit conflict policy
(see §5). A `Language` carries no Rust: no trait objects, no closures, no
`Box<dyn Behavior>`. It is `serde`-serializable end to end.

A kit is an ordered list of fragment sources plus include/exclude filters. A
fragment source is either a data file (the end state) or a Rust emitter that
produces fragments at load time (the migration scaffold, see §6). The kit
selects fragments, merges them in order, and hands the resulting `Language` to
a `Runtime`. **Selection and merge are the composition; the registry is
replaced by a `Language` built from merged fragments.**

| Today (runtime)            | After (data)                               |
| -------------------------- | ------------------------------------------ |
| `PluginRegistry`           | `Language` (merged fragments)              |
| `Kit` manifest             | ordered fragment sources + include/exclude |
| `Plugin::contribute`       | fragment emission at load → merge          |
| include/exclude plugin ids | include/exclude fragment ids               |
| `RuleAcceptor::accept`     | `Language::merge(fragment)`                |

#### Supersession of the trait-based AST

ADR-0023's `Node<K, V, M>` trait-based AST (later `handl-plugin-ast`) is
**fully superseded, not a trade-off**. The new `Ast` arena
(`AstNode { kind: String, span: Option<Span>, payload: Value }` + `AstRef`
indices) is strictly better for the platform's actual model. The two
advantages the trait design held — compile-time kind safety and zero-cost
metadata — do not survive once emitters stop being Rust code:

- **Static kind safety is recovered at the language's compile-time.**
  `AstShapeDef` carries a `payload_schema` (JSON Schema) keyed by
  `node_kind`, so unknown kinds and malformed payloads are caught when a
  `Language` is built/loaded. That _is_ the language's compile-time — it
  just runs in the host process. Since emitters are `Scheme` combinators
  and `DesugarRule`s declared as data in the `Language` value (not Rust
  code), there are no Rust emitter authors for the host typechecker to
  help. The "Rust author gets no typechecker help" concern is moot: there
  are no Rust authors of emitters.

- **Data dispatch is erased by compilation.** The tree-walk pays for
  per-node scheme lookup, but the bytecode and cranelift engines compile
  that lookup away at compile time — the compiled artifact has no hash
  lookups, no string matching, no `lang.eval` access. The arena's
  `kind: String` / `payload: Value` cost is paid once at compile; the hot
  path runs compiled bytecode or native code. This is a performance win
  the trait-based `Box<dyn DynNode>` design could not offer — trait-object
  dispatch does not lower cleanly to bytecode or cranelift IR.

The only thing genuinely unrecoverable would have been compile-time
enforcement _in Rust emitter code_ — but ADR-0044 §6 deleted all Rust
emitters in favor of authored data files, so that residual cost does not
exist. The trait-based AST is obsolete because the requirements changed
(serializable, host-agnostic, data-driven, JIT-able), and the new design
satisfies all of them without giving up anything that still matters.

### 2. Staged entry points; `eval` takes `Ast`, not `Source`

Each stage consumes only the previous stage's output plus the `Language` spec.
The evaluator never touches source text or tokens.

```rust
// handl-language
pub fn lex(lang: &Language, src: &Source)   -> Result<Tokens, LangError>;
pub fn parse(lang: &Language, toks: &Tokens) -> Result<Ast, LangError>;
pub fn eval(lang: &Language, ast: &Ast)     -> Result<Value, LangError>;

// convenience: parse(lang, src) = parse(lang, &lex(lang, src))
```

`Ast` is part of the `Language` data contract: the `ast` fragment set defines
node kinds and their payload schemas, so `parse` and `eval` share a
data-defined intermediate. A `Runtime` may offer `eval` alone (a JIT runtime
that compiles a pre-parsed `Ast`) without pulling in a lexer or parser.

This corrects ADR-0041's `eval : Language -> Source -> Value`. The handl-level
`eval` function exposed to programs is `eval : Language -> Ast -> Value`; a
separate `run : Language -> Source -> Value` convenience composes the three
stages.

### 3. Crate structure: `plugin-engine-*` for abstraction and engine

Split each `plugin-ext-*` into an abstraction crate (operation + trait, no
engine) and an engine crate (interprets `Language` data). Use the
`plugin-engine-*` prefix.

```
handl-language              Language data schema + lex/parse/eval dispatch
handl-runtime               binds a Language to engine impls

ABSTRACTION (operation + trait, no engine)
  plugin-engine-lex            from plugin-ext-lexer
  plugin-engine-parse          from plugin-ext-parser
  plugin-engine-exec           from plugin-ext-eval

ENGINE (interprets Language data)
  plugin-engine-parse-pratt    reads grammar table, drives Pratt loop
  plugin-engine-exec-tree-walk interprets schemes directly (tree-walk)
  plugin-engine-exec-bytecode  compiles schemes to bytecode, executes
  plugin-engine-exec-cranelift compiles schemes to native code
  (future: plugin-engine-lex-logos)
```

The Pratt engine no longer calls `rule.prefix(token, ctx)` on a Rust
`ParserBehavior` object. It looks up `lang.grammar[token]`, reads the
descriptor (prefix/infix, binding powers, child shape), and
executes. Same for eval engines: they look up `lang.eval[node.kind]`, read
the scheme, and interpret it — tree-walk executes directly, bytecode/cranelift
compile to their respective targets.

Swapping Pratt for chumsky means swapping `plugin-engine-parse-pratt` for
`plugin-engine-parse-chumsky`. The `Language` and every grammar fragment stay
untouched.

### 4. Evaluation schemes as data, engines as interpreters

Eval semantics are data, not Rust — and crucially, **not closed machine
instructions either**. The `Language`'s `eval` fragment set maps each `Ast`
node kind to an **evaluation scheme**: an engine-agnostic description of _what_
to evaluate, not _how_. This mirrors lex and parse, where the `Language` carries
data and any engine interprets it:

```
LEX:    Language says "these token kinds exist"        → any lexer engine produces tokens
PARSE:  Language says "these grammar rules exist"       → any parser engine produces AST
EVAL:   Language says "these evaluation schemes exist"  → any eval engine produces values
```

A scheme is built from a small set of **semantic combinators** — a description
language for evaluation, the eval equivalent of "binding powers + shape" in
grammar rows:

```rust
pub enum Scheme {
    Lit(Value),
    Var(String),
    Child(AstRef),                          // eval child[i]
    Bind { name: String, value: Box<Scheme>, body: Box<Scheme> },
    App { callee: Box<Scheme>, args: Vec<Scheme> },
    Branch { cond: Box<Scheme>, then: Box<Scheme>, else_: Box<Scheme> },
    Prim { op: String, args: Vec<Scheme> },
    Foreign { name: String, args: Vec<Scheme> },   // host function, capability-tracked
}
```

Each node kind maps to a scheme:

```
"binary.add" → Prim { op: "add", args: [Child(0), Child(1)] }
"call"       → App { callee: Child(0), args: [Child(1), Child(2), ...] }
"let"        → Bind { name: <from payload>, value: Child(0), body: Child(1) }
"if"         → Branch { cond: Child(0), then: Child(1), else_: Child(2) }
```

Engines interpret schemes — they are not the schemes:

- `plugin-engine-exec-tree-walk` — interprets schemes directly (tree-walk)
- `plugin-engine-exec-bytecode` — compiles schemes to bytecode, executes
- `plugin-engine-exec-cranelift` — compiles schemes to native code

Adding an engine does not require the `Language` to change. Same property as
swapping Pratt for chumsky. The scheme description is the stable contract; the
engine is the replaceable implementation.

#### Small core + desugar

Surface constructs (`Match`, `Handle`, `Pipe`, `Ternary`, …) are **desugar
rules** (data rewrites) that lower to core schemes, not separate scheme tags.
Adding a surface construct = adding a rewrite rule (data), not extending the
combinator set. The core stays small; the surface stays rich.

```
surface construct
    │  desugar (data rewrite)
    ▼
core scheme (combinators)
    │
    ├── tree-walk interprets       (plugin-engine-exec-tree-walk)
    ├── bytecode compiler emits    (plugin-engine-exec-bytecode)
    └── cranelift compiler emits   (plugin-engine-exec-cranelift)
```

#### Foreign as data, not Rust escape

When a scheme genuinely needs a host function (I/O, FFI), it uses
`Foreign(name)` — still data, still capability-tracked. A pure `Language` has
no `Foreign` tags; a sandboxed one has a vetted subset. The boundary is
explicit and auditable, never a hidden Rust impl.

#### Denotational semantics as a future interpretation

The small core is also small enough to map to a category (CCC for lambda
calculus). A denotational engine — mapping schemes to morphisms for
equational reasoning, optimization, type-checking — becomes possible because
of this design. It is a third interpretation of the same core, alongside the
operational engines. Not Phase 1, but enabled by keeping the core small.

### 5. Fragment merge with declared override, not silent last-wins

`FragmentSet<T>` merge distinguishes **override** from **collision** via
fragment metadata. Every fragment has an `id` and an optional
`replaces: [id]` field:

- **Override** = a fragment declares `replaces: [existing_id]`. Allowed,
  auditable, recorded in the merged `Language`.
- **Collision** = two fragments target the same key, neither declares
  `replaces`. **Error.**

Per-type identity keys on top:

| Fragment         | Identity key        | Collision without `replaces` |
| ---------------- | ------------------- | ---------------------------- |
| `TokenKindDef`   | `id`                | **Error**                    |
| `GrammarRow`     | `(token, position)` | **Error**                    |
| `EvalSchemeDef`  | `node.kind`         | **Error**                    |
| `AstShapeDef`    | `node.kind`         | **Error**                    |
| `CapabilityDecl` | `id`                | Union (idempotent)           |
| `OptionSchema`   | option key          | **Error**                    |

This turns composition into an **auditable override graph**: a kit's merged
`Language` carries the full history of what replaced what. "Why does this
`let` parse differently?" is answerable by reading the override chain —
impossible with silent last-wins. Include/exclude at the fragment level is a
real operation: excluding a fragment that others `replaces` is explicit, not a
silent no-op.

### 6. Rule migration: emitters as scaffold, data files as end state

The end state is **authored data files are canonical**. If Rust emitters stay
the permanent authoring layer, Rust is still the definition and self-hosting
is undermined. But hand-authoring ~40 constructs as data on day one is
error-prone. Use emitters as a temporary scaffold:

1. Each `plugin-grammar-*` / `plugin-eval-*` becomes a small Rust **emitter**
   that produces `Language` fragments. Lets you migrate one construct at a
   time and verify the engine reads emitted data correctly.
2. Once all constructs emit and verify, consolidate into authored data files
   (TOML/JSON/handl) and **delete the emitters**.

Kits merge fragments whether they come from emitters or files, so composition
is identical across the migration. The scaffold is invisible to the
composition model.

**Status: complete.** The 37 `handl-emitter-*` crates have been deleted. The
canonical language definition is now 7 YAML files (one per fragment type:
`tokens.yaml`, `grammar.yaml`, `eval.yaml`, `ast.yaml`, `caps.yaml`,
`options.yaml`, `lexer_rules.yaml`) in `crates/handl-kit-data-canonical/data/`,
embedded at compile time via `include_str!`. The `handl-emitters` crate
remains as a thin trait definition (`Emitter`) used by the kit assembly
framework, but no per-construct emitters exist.

### 7. Review refinements: composability, extensibility, replaceability, performance, safety

Five specifications left open by §1–§6, resolved here:

#### Fragment-level `requires` (composability)

Fragments carry an optional `requires: [id]` field, validated at merge time.
A `GrammarRow` for `call` declares `requires: ["token.lparen",
"token.rparen"]`; merging it into a `Language` that lacks those token kinds is
an error at merge time, not a confusing "unknown token" at parse time. This
preserves today's `Plugin::requires`/`optional` semantics as data. Missing
`requires` is a hard error; missing `optional` dependencies are silently
ignored (the fragment still merges, but may fail at runtime if it tries to
reference the absent dependency).

#### Scheme combinator versioning (extensibility)

The `Scheme` combinator set is versioned. A `Language` declares
`scheme_version: "v1"`; engines declare which versions they support. Extending
the core (adding a combinator that can't be expressed via desugar, e.g.
delimited continuations) ships as `v2`. A `v1` `Language` runs on a `v2`
engine (backward compatible); a `v2` `Language` on a `v1` engine is a
compile-time error with a clear message. Engines implement the versions they
support; a new engine can start at any version.

#### `Foreign` host-function registration (extensibility)

`Foreign(name)` in a scheme references a named host function. Host functions
are registered in the `Runtime` via the existing `Module`/`ModuleRuntime`
mechanism (from ADR-0040). A `Runtime` exposes its registered host-function
names; `eval` checks that every `Foreign(name)` in the `Language`'s schemes
has a matching registration. Unregistered `Foreign` at eval time is an error.
The `Language`'s capability set (see safety below) is the static projection;
the `Runtime`'s registration set is the dynamic availability.

#### Engine selection as data (replaceability)

A `Runtime` is configured with engine selections as data, not via host-code
linking. The kit manifest (or runtime configuration) specifies:

```toml
[runtime]
lex = "pratt"          # or "logos" (future)
parse = "pratt"        # or "chumsky" (future)
eval = "tree-walk"     # or "bytecode", "cranelift"
```

Swapping an engine is a config change, not a recompile. The `Runtime` resolves
engine names to crates at build time (feature flags or direct dependency), but
the _selection_ is data. A host loads a kit, reads the runtime config, and
binds the named engines.

#### Compilers erase data dispatch (performance)

The tree-walk pays for per-node scheme lookup (`lang.eval[node.kind]` →
`Scheme`). The bytecode and cranelift engines **compile this lookup away**: at
compile time, every node kind's scheme is resolved to a fixed instruction
sequence or native code. Compiled performance is independent of the
data-dispatch overhead — the compiled artifact has no hash lookups, no string
matching, no `lang.eval` access. The tree-walk is the reference interpreter;
the compilers are the performance paths, and their job is to erase the data
dispatch entirely.

#### Capability set as transitive closure of `Foreign` (safety)

A `Language`'s capability set is the **transitive closure of `Foreign` names
across all its eval schemes**, plus any explicitly declared capabilities. This
makes "does this `Language` touch the filesystem?" a static data query:

```rust
fn capabilities(lang: &Language) -> Set<CapabilityId> {
    lang.eval.fragments()
        .flat_map(|scheme| foreign_names(scheme))
        .chain(lang.caps.iter().map(|c| c.id.clone()))
        .collect()
}
```

A sandbox verifies `capabilities(mod_lang).subset_of(allowed)` at load time —
no runtime sandbox required. A pure `Language` (no `Foreign` tags) is safe to
eval on any source. This is the concrete realization of ADR-0041's
capability-discipline-at-the-language-level.

## Consequences

### Positive

- **`Language` is its own specification, as data.** No Rust defines the
  language; the `Language` value is the spec. Conformance is
  `eval(thisLanguage, ast) == expected`.
- **Self-hosting is real.** A `Language` value can describe handl itself,
  loaded as data. The bootstrap is `compile(handlLanguage, target)` — one
  function application on data.
- **Engines are swappable without touching the language.** Pratt ↔ chumsky,
  tree-walk ↔ bytecode ↔ cranelift: swap one `plugin-engine-*` engine crate.
  The `Language` and all fragments are unchanged.
- **`eval` on `Ast` enables AST-only runtimes.** A bytecode or cranelift
  runtime compiles a pre-parsed `Ast` without a lexer or parser. Re-evaluation
  without re-parsing. AST-level transformation between parse and eval.
- **Composability is preserved as data composition.** Kits still curate tiny
  pieces; include/exclude, ordering, and override all survive as fragment
  selection and merge. The valued property does not regress.
- **`compile : Language -> Target` is a function on data.** Futamura
  projections become currying, as ADR-0041 described — but now over data, not
  Rust trait objects.

### Negative

- **The scheme combinator set + desugar rules are the crux and the risk.**
  "Semantics as data" for a full language (call, match, handle, pipes, ternary,
  function literals, …) is genuinely hard. The core combinator set must be
  small enough to interpret/compile efficiently yet expressive enough that
  surface constructs desugar to it without escaping to Rust. An incomplete
  combinator set or desugar layer forces `Foreign` escapes, which undermine the
  pure-data model. The small-core + desugar approach bounds this risk but does
  not eliminate it.
- **Wide blast radius.** Splitting `plugin-ext-*`, extracting engines,
  migrating ~40 rule crates to data, and repointing kits touches the platform,
  every `plugin-grammar-*`/`plugin-eval-*`, every kit, and every host. This is
  a multi-phase migration, not a single PR.
- **Performance for the data-dispatched tree-walk.** Scheme interpretation over
  data is slower than a direct enum match on a static `CanonicalKind`. The
  bytecode and cranelift engines close this gap by compiling schemes ahead of
  execution; the tree-walk pays for data dispatch. Acknowledged tradeoff; the
  tree-walk is the reference interpreter, not the performance path.
- **Three eval engines to build and keep in sync.** tree-walk, bytecode, and
  cranelift must all interpret the same scheme vocabulary. A scheme that one
  engine handles but another doesn't is a silent divergence. Conformance
  testing across engines is required.
- **Two authoring paths during migration.** Emitters and data files coexist
  until migration completes. Kits must accept both; the framework carries the
  complexity until emitters are deleted.

## Testing strategy

Every phase ships with tests that must pass before the phase is considered
complete. Coverage is measured per-crate via `cargo-llvm-cov` (see
`.devin/AGENTS.md`). The target is **100% line coverage** for new code in each
phase; pre-existing code touched by a phase must not drop below its current
coverage. Tests follow the existing conventions: inline `#[cfg(test)]` modules
for unit tests, `tests/` directory for integration tests, mirroring source
structure.

### Cross-cutting: conformance testing

The single most important test artifact is a **conformance test suite** — a
shared corpus of `(Language, Ast, expected Value)` triples that every eval
engine must produce identical results for. This suite is authored once (in
Phase 1, seeded from existing `kit-canonical` tests) and extended every phase.
Every eval engine crate runs the full suite. A divergence is a test failure.

```
handl-test-utils (existing)
  └── conformance/
        ├── corpus/           # (Language, Ast, expected) triples as data
        └── run.rs            # harness: run corpus against any eval engine
```

Similarly, a **parse conformance suite** — `(Language, Tokens, expected Ast)`
triples — is shared across parser engines.

### Per-phase coverage requirements

#### Phase 1 — `Language` data schema

**Unit tests (inline in `handl-language/src/`):**

- Every fragment type: construction, serialization (serde round-trip),
  equality, `id()` and `replaces` field access.
- `FragmentSet<T>::merge`: empty merge, single fragment, multi-fragment merge,
  declared override (`replaces` succeeds and records the override chain),
  collision without `replaces` (returns error), union semantics for
  `CapabilityDecl`, every identity key from §5 table.
- `FragmentSet<T>::merge` with cyclic `replaces` chains (error).
- `Language::merge`: merging two `Language`s merges all six fragment sets
  independently; partial merge failure rolls back atomically.
- `Ast` arena: node insertion, `AstRef` resolution, out-of-bounds index
  (error), arena growth.
- `Scheme` combinators: every variant constructs, serializes, and round-trips.
- Desugar rule format: a rule matches a node kind and produces a core
  `Scheme`; unmatched kind returns `None`.
- `lex`/`parse`/`eval` dispatch signatures: return `NotImplemented` when no
  engine is bound (Phase 1 dispatches to existing Rust engines, so these
  delegate correctly).
- Fragment `requires` declarations: a fragment declaring `requires: [id]`
  fails merge if the required fragment is absent (the composability gap from
  the review). Missing `optional` dependencies merge successfully.
- Scheme versioning: a `Language` with `scheme_version: "v1"` serializes and
  deserializes with the version field; an engine declaring support for `v1`
  accepts it, an engine declaring only `v2` rejects it with a clear error.
- Capability set derivation: `capabilities(lang)` returns the transitive
  closure of `Foreign` names across all eval schemes plus declared
  capabilities. A `Language` with no `Foreign` tags returns only declared
  capabilities. `subset_of` check passes/fails correctly.

**Integration tests (`handl-language/tests/`):**

- `merge_semantics.rs` — full kit-like merge sequences: base language + 5
  fragments, with overrides and excludes, produces the expected `Language`.
- `serde_roundtrip.rs` — a non-trivial `Language` serializes to JSON/TOML and
  deserializes to an equal `Language`.
- `conflict_policy.rs` — every conflict scenario from §5 table, positive and
  negative.

**Coverage gate:** `cargo llvm-cov -p handl-language --fail-under 100`.

#### Phase 2 — `plugin-engine-*` abstractions and engines

**Unit tests (inline in each crate):**

- `plugin-engine-lex`: `Lexer` operation marker, `LexerRequest`
  construction, trait dispatch to engine.
- `plugin-engine-parse`: `Parser` operation marker, `ParserRequest`
  construction, `ParserContext` API (`peek`, `advance`, `expression`).
- `plugin-engine-exec`: `Eval` operation marker, `EvalRequest`
  construction, `Scheme` interpretation entry point.
- `plugin-engine-parse-pratt`: reads `Language.grammar` data, drives Pratt
  loop for each grammar row type (prefix-only, infix-only, both), binding
  power precedence, EOF handling, unexpected token error, empty input.
- `plugin-engine-exec-tree-walk`: interprets every `Scheme` combinator
  (`Lit`, `Var`, `Child`, `Bind`, `App`, `Branch`, `Prim`, `Foreign`),
  scope extension for `Bind`, recursion via `Child`, `Foreign` dispatch to
  registered host function, `Foreign` for unregistered name (error).

**Integration tests (`tests/` per crate):**

- `plugin-engine-parse-pratt/tests/parse_conformance.rs` — runs the parse
  conformance suite against the Pratt engine.
- `plugin-engine-exec-tree-walk/tests/eval_conformance.rs` — runs the eval
  conformance suite against the tree-walk engine.

**Coverage gate:** `cargo llvm-cov -p handl-plugin-engine-lex -p
handl-plugin-engine-parse -p handl-plugin-engine-exec -p
handl-plugin-engine-parse-pratt -p handl-plugin-engine-exec-tree-walk
--fail-under 100`.

#### Phase 3 — `handl-runtime`

**Unit tests (inline):**

- `Runtime` construction with engine selection (lex engine, parse engine,
  eval engine — each selectable independently).
- `Runtime::lex`/`parse`/`eval` dispatch to the bound engines.
- `Runtime` with a missing engine (e.g. eval-only runtime without lex/parse)
  — `lex` returns error, `eval` works.
- Engine selection from data (kit/runtime configuration), not hardcoded
  linking (the replaceability gap from the review). A runtime config
  specifying `eval = "tree-walk"` binds the tree-walk engine; specifying
  `eval = "bytecode"` (once Phase 6 lands) binds the bytecode engine.
  Unknown engine name is a config error.

**Integration tests:**

- `runtime_pipeline.rs` — full `lex → parse → eval` over a `Language` +
  `Source`, asserts correct `Value`.
- `runtime_eval_only.rs` — pre-parsed `Ast` fed to `eval` without lex/parse
  engines bound.

**Coverage gate:** `cargo llvm-cov -p handl-runtime --fail-under 100`.

#### Phase 4 — Migrate rules to data

**Per-construct migration test (one per `plugin-grammar-*` / `plugin-eval-*`):**

- Emitter produces a `Language` fragment.
- Engine (Pratt or tree-walk) reads the fragment and produces the same
  output as the **existing Rust `Behavior` impl** for the same input.
- This is a **parity test**: old Rust impl vs new data-driven engine, same
  input, same output. Every construct gets one. Failure = the data migration
  changed semantics.

**Integration tests:**

- `full_language_parity.rs` — the complete migrated `Language` (all
  constructs) produces identical parse + eval results as the pre-migration
  `kit-canonical` across the existing test corpus (`sugars.rs`, `runtime.rs`
  patterns).

**Coverage gate:** every emitter crate at 100%; parity tests all pass.

#### Phase 5 — Kits select `Language` + `Runtime`

**Unit tests:**

- Kit manifest parsing: fragment sources (file paths, emitter references),
  include/exclude filters, `extends` chains.
- Kit assembly: merge fragments in declared order, apply include/exclude,
  produce a `Language` + `Runtime` selection.
- Exclude a fragment that others `replaces` → override chain updates
  correctly (the fragment is gone, dependents either find their target or
  error).
- Kit `extends` chain: `kit-server` extends `kit-config` extends
  `kit-canonical` — merged `Language` is the union with correct overrides.

**Integration tests:**

- `kit_assembly.rs` — assemble each existing kit (`kit-canonical`,
  `kit-config`, `kit-server`, `kit-cli`) from fragment sources, verify the
  resulting `Language` matches expectations.
- `kit_swap_engine.rs` — same kit, swap tree-walk for bytecode (once Phase 6
  lands), verify identical results.

**Coverage gate:** `cargo llvm-cov -p handl-kit-canonical -p handl-kit-config
-p handl-kit-server -p handl-kit-cli --fail-under 100`.

#### Phase 6 — `plugin-engine-exec-bytecode`

**Unit tests:**

- `Scheme` → bytecode compilation: every combinator compiles to the expected
  bytecode instruction sequence.
- Bytecode VM execution: every instruction executes correctly.
- Compilation errors: unsupported combinator (if scheme version mismatch),
  malformed scheme.

**Integration tests:**

- `bytecode_conformance.rs` — runs the full eval conformance suite against
  the bytecode engine. **Every triple must match the tree-walk engine's
  output.** This is the cross-engine conformance gate.

**Coverage gate:** `cargo llvm-cov -p handl-plugin-engine-exec-bytecode
--fail-under 100`.

#### Phase 7 — `plugin-engine-exec-cranelift`

**Unit tests:**

- `Scheme` → cranelift IR compilation: every combinator compiles.
- Native execution: every compiled scheme produces correct output.
- Compilation errors: unsupported combinator, scheme version mismatch.

**Integration tests:**

- `cranelift_conformance.rs` — runs the full eval conformance suite against
  the cranelift engine. **Every triple must match tree-walk and bytecode.**
  Three-way conformance gate.

**Coverage gate:** `cargo llvm-cov -p handl-plugin-engine-exec-cranelift
--fail-under 100`.

#### Phase 8 — Self-hosting

**Integration tests:**

- `self_host.rs` — load `handl-defining-handl` as a `Language` value,
  `eval(handlLanguage, ast)` produces the same result as the host-built
  handl for a corpus of programs.
- `bootstrap.rs` — `compile(handlLanguage, native)` produces a binary that,
  when run on a corpus, produces the same results as the host-built handl.
- `futamura.rs` — specializing `eval(handlLanguage, ast)` by fixing
  `handlLanguage` and leaving `ast` dynamic produces a compiled program
  (the Futamura projection sanity check).

**Coverage gate:** self-hosting test crates at 100%; all conformance suites
pass under the self-hosted language.

### Continuous integration

Every phase's coverage gate runs in CI (`.github/workflows/coverage.yml`
already exists). A phase is not merged until:

1. `cargo test --workspace` passes.
2. Per-phase `cargo llvm-cov --fail-under 100` passes for new/touched crates.
3. Cross-engine conformance suites pass (tree-walk vs bytecode vs cranelift,
   once each engine exists).
4. Parity tests pass (data-driven engine vs old Rust impl, during Phase 4).

## Implementation phases

### Phase 1 — `Language` data schema and `Ast` contract

Design the fragment types (`TokenKindDef`, `GrammarRow`, `EvalSchemeDef`,
`AstShapeDef`, `CapabilityDecl`, `OptionSchema`), the `FragmentSet` merge with
declared-override policy (§5), the `Ast` arena + `AstRef` shape, the `Scheme`
combinator set, and the desugar rule format. Create `handl-language` with the
schema and the `lex`/`parse`/`eval` dispatch signatures (initially dispatching
to the existing Rust engines, so nothing breaks). _The scheme combinator set
and desugar format are the crux; everything depends on them._

**Testing:** see [Phase 1 coverage requirements](#phase-1--language-data-schema)
above.

### Phase 2 — `plugin-engine-*` abstractions and engines

Split `plugin-ext-lexer` → `plugin-engine-lex` + (engine stays in
`plugin-engine-parse-pratt`'s lexer half, or a dedicated lexer engine later).
Split `plugin-ext-parser` → `plugin-engine-parse` (abstraction) +
`plugin-engine-parse-pratt` (engine reading `Language` grammar data). Split
`plugin-ext-eval` → `plugin-engine-exec` (abstraction) +
`plugin-engine-exec-tree-walk` (engine interpreting `Scheme` data). Engines
consume `Language` data, not Rust `Behavior` impls.

**Testing:** see [Phase 2 coverage requirements](#phase-2--plugin-engine--
abstractions-and-engines) above.

### Phase 3 — `handl-runtime`

Binds a `Language` to engine impls; owns the lexer/parser/evaluator engine
selection; exposes `lex`/`parse`/`eval` over a `Language` + stage inputs.

**Testing:** see [Phase 3 coverage requirements](#phase-3--handl-runtime)
above.

### Phase 4 — Migrate rules to data

Convert each `plugin-grammar-*` / `plugin-eval-*` into an emitter producing
`Language` fragments. Verify each engine reads emitted data correctly, one
construct at a time. Once all emit and verify, consolidate into authored data
files and delete the emitters.

**Testing:** see [Phase 4 coverage requirements](#phase-4--migrate-rules-to-data)
above.

### Phase 5 — Kits select `Language` + `Runtime`

Kits assemble a `Language` from fragment sources (files or emitters during
migration, files after) and select a `Runtime`, instead of registering
`BasicParser` + Rust rules into a `PluginRegistry`.

**Testing:** see [Phase 5 coverage requirements](#phase-5--kits-select-
language--runtime) above.

### Phase 6 — `plugin-engine-exec-bytecode`

Build the bytecode engine: compiles `Scheme` data to a bytecode
representation, executes via a bytecode VM. Shares the same scheme vocabulary
as the tree-walk; conformance tests verify both produce identical results.
The tree-walk remains the reference interpreter; bytecode is the first
performance path.

**Testing:** see [Phase 6 coverage requirements](#phase-6--plugin-engine-
eval-bytecode) above.

### Phase 7 — `plugin-engine-exec-cranelift`

Build the cranelift engine: compiles `Scheme` data to native code via
cranelift. Shares the same scheme vocabulary; conformance tests verify
identical results across tree-walk, bytecode, and cranelift. Enables the
self-hosting bootstrap as a compilation step (`compile(handlLanguage, native)`).

**Testing:** see [Phase 7 coverage requirements](#phase-7--plugin-engine-
eval-cranelift) above.

### Phase 8 — Self-hosting

`Language` carries an `eval` capability; a `Language` value describes handl
itself. Ship a `handl-defining-handl` data file loaded as a `Language` value,
evaluated under itself. The bootstrap becomes
`compile(handlLanguage, native)` via the cranelift engine.

**Testing:** see [Phase 8 coverage requirements](#phase-8--self-hosting)
above.

## Related work

- [ADR-0041](./0041-languages-as-first-class-values.md) — the vision this ADR
  refines and makes concrete. This ADR corrects 0041's `eval` signature and
  specifies the data model and crate structure 0041 left open.
- [ADR-0039](./0039-kits-are-curated-plugin-lists.md) — kits as curation. This
  ADR preserves curation as fragment selection + merge.
- [ADR-0040](./0040-vm-is-an-operation-not-a-subsystem.md) — eval as an
  operation. This ADR makes that operation a data-driven engine over a
  `Language` value.
- Futamura projections — specialization of interpreters into compilers,
  enabled by `eval`/`compile` as functions on `Language` data.
- Racket `#lang` — languages as modules (module-level, not runtime data).
