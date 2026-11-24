---
title: Decisions
type: index
---

# Decisions

One line per decision rule, generated from each ADR's `## Decision`
section with `host/scripts/gen-decisions.py`. The ADR is the argument;
this is the recall surface (ADR-0161 §8). Superseded records are excluded.

### Host Bundles

- **DefaultHost** — built-ins only: add, mul, concat, len, var, resolve, apply, array. Interface-agnostic base layer — [ADR-0001](adrs/0001-host-bundles.md)
- **SandboxHost** — pure library modules + resource limits (max_depth, max_calls). No I/O, no import(). For untrusted… — [ADR-0001](adrs/0001-host-bundles.md)
- **TypeHost** — type-level evaluation. Types are JSON values; is_compatible is an handl function. Meta-circular — [ADR-0001](adrs/0001-host-bundles.md)
- **MetaHost** — reflection: typeof, keys, eval, parse, AST inspection — [ADR-0001](adrs/0001-host-bundles.md)
- **CacheHost** — decorator that memoizes any inner host. Cross-machine resumability via CallCache — [ADR-0001](adrs/0001-host-bundles.md)
- **ResolverHost** — decorator that adds runtime import() backed by a ModuleResolver — [ADR-0001](adrs/0001-host-bundles.md)

### ADR-0002 (Remaining): Plugin Library Conversion and Module Abstraction

- _*Should plugin-lib-* crates use Behavior<Op> or is Module/Function the appropriate abstraction for host-exported… — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Current state: ~20+ plugin-lib-* crates use Module trait and expose Function values for handl code to call — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Option A: Convert to Behavior<Op> (one operation per library capability) — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Option B: Keep Module/Function as the abstraction for "host functions that handl code calls" (distinct from rule… — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Option C: Hybrid — use Module as an adapter that contributes Behavior<Op> to the appropriate operations — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- **How should library dependencies be expressed?** — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Can a plugin-lib-* declare that it requires or provides specific module capabilities? — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Should this be done via Plugin::depends_on() trait or via a separate dependency manifest? — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- **What about library options?** — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- How do plugin-lib-* register options (e.g., compression level, timeout) if not via the legacy Library::options()? — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)
- Should the OptionsPlugin be extended to support library-scoped options? — [ADR-0002](adrs/0002-adr-0002-remaining-plugin-library-conversion-and-module-abstraction.md)

### Library crates are exposed as Platform plugins

- The crate name is prefixed with plugin-lib- (e.g. plugin-lib-json, plugin-lib-crypto) to make the plugin role explicit… — [ADR-0003](adrs/0003-library-crates-are-exposed-as-platform-plugins.md)
- Each crate contains a plugin module that exposes a minimal Plugin implementation — [ADR-0003](adrs/0003-library-crates-are-exposed-as-platform-plugins.md)
- The plugin's id is lib.<name> (e.g. lib.json), and its contribute method registers the concrete library… — [ADR-0003](adrs/0003-library-crates-are-exposed-as-platform-plugins.md)
- The plugin does not declare capabilities of its own unless the library is directly invokable; its purpose is to add the… — [ADR-0003](adrs/0003-library-crates-are-exposed-as-platform-plugins.md)
- plugin-lib-type keeps its Plugin module behind a plugin feature so that consumers that only need the type definitions… — [ADR-0003](adrs/0003-library-crates-are-exposed-as-platform-plugins.md)

### Platform is agnostic of language-specific plugins

- The plugin-lib-* crates that define the Plugin contract and any shared, language-neutral primitives — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- Utility crates for serialization, collections, etc. that have no language-specific knowledge — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-ast-* (AST node plugins) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-parser (parser) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-grammar-* (grammar rules) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-typecheck-core (typechecking) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-runtime-* (runtime rules) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-vm-* and plugin-vm-values-* (VM and value representations) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-eval-* (eval rules) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)
- plugin-fmt-*, plugin-lint-*, plugin-diag-* (formatting, linting, diagnostics) — [ADR-0004](adrs/0004-platform-is-agnostic-of-language-specific-plugins.md)

### ADR-0009: Strategy C — Runtime Module Loading as Handler-Interpreted Ops

- **Strategy C is not a separate strategy.** module_ref and project are — [ADR-0009](adrs/0009-adr-0009-strategy-c-runtime-module-loading-as-handler-interpreted-ops.md)
- **The handler's operation table is the opt-in.** A handler without the — [ADR-0009](adrs/0009-adr-0009-strategy-c-runtime-module-loading-as-handler-interpreted-ops.md)
- **Governance is handler configuration**: path allowlist, load-depth limit, — [ADR-0009](adrs/0009-adr-0009-strategy-c-runtime-module-loading-as-handler-interpreted-ops.md)
- **Two static passes work without executing**: cycle detection over the — [ADR-0009](adrs/0009-adr-0009-strategy-c-runtime-module-loading-as-handler-interpreted-ops.md)
- **Graduate to real Scheme variants only** when a second handler needs — [ADR-0009](adrs/0009-adr-0009-strategy-c-runtime-module-loading-as-handler-interpreted-ops.md)

### Package Manager: Content-Addressed, Diff-Based, Composable Stores

- Re-installing the same hash is a no-op — [ADR-0010](adrs/0010-package-manager-content-addressed-diff-based-composable-stores.md)
- Updating v1.2.0 → v1.3.0 fetches only the diff between the two graph — [ADR-0010](adrs/0010-package-manager-content-addressed-diff-based-composable-stores.md)
- Docs, source maps, and derived artifacts are content-addressed blobs — [ADR-0010](adrs/0010-package-manager-content-addressed-diff-based-composable-stores.md)

### Versioning for Language Artifacts

- version is optional. Documents without it default to "0.0.0" — [ADR-0012](adrs/0012-versioning-for-language-artifacts.md)
- The VM ignores version — it has no runtime cost — [ADR-0012](adrs/0012-versioning-for-language-artifacts.md)
- The module resolver uses version to validate that an imported module — [ADR-0012](adrs/0012-versioning-for-language-artifacts.md)
- annotations.since and annotations.deprecated (ADR-0011) can refer to — [ADR-0012](adrs/0012-versioning-for-language-artifacts.md)

### handl Formatter

- **Two-space indentation** (YAML, .handl). JSON and TOML follow their — [ADR-0014](adrs/0014-handl-formatter.md)
- **A single trailing newline** at the end of each file — [ADR-0014](adrs/0014-handl-formatter.md)
- **No trailing whitespace** on any line — [ADR-0014](adrs/0014-handl-formatter.md)
- **Canonical key order for mappings.** Unknown keys are kept after known — [ADR-0014](adrs/0014-handl-formatter.md)
- Document top-level: annotations, version, import, fields, — [ADR-0014](adrs/0014-handl-formatter.md)
- Field mapping: type, expr, arg, schema, inheritable, — [ADR-0014](adrs/0014-handl-formatter.md)
- **Array order is preserved.** Authors may group items intentionally — [ADR-0014](adrs/0014-handl-formatter.md)
- **Comments are preserved** where the underlying document model supports — [ADR-0014](adrs/0014-handl-formatter.md)
- The formatter operates on the parsed document, re-orders mapping keys with — [ADR-0014](adrs/0014-handl-formatter.md)
- --check compares the emitted text to the original and exits 1 when — [ADR-0014](adrs/0014-handl-formatter.md)
- --write overwrites the source file atomically — [ADR-0014](adrs/0014-handl-formatter.md)
- The formatter does not resolve imports or merge included files — it — [ADR-0014](adrs/0014-handl-formatter.md)

### ADR-0015: Linter

- **handl lint checks documents against an extensible rule set**; paths may — [ADR-0015](adrs/0015-adr-0015-linter.md)
- **A rule is a pure function** from the parsed document + file metadata to — [ADR-0015](adrs/0015-adr-0015-linter.md)
- **Severity is error / warn / info**; exit 0 unless an error is — [ADR-0015](adrs/0015-adr-0015-linter.md)
- **The initial rule set** is naming-convention, missing-annotations, — [ADR-0015](adrs/0015-adr-0015-linter.md)
- **Rules are namespaced and extensible** — hosts register their own — [ADR-0015](adrs/0015-adr-0015-linter.md)

### Lexer Architecture: Self-Hosted Streaming with Runtime Config

- The canonical lexer already processes char-by-char, so yielding tokens incrementally is a natural extension — not a… — [ADR-0022](adrs/0022-lexer-architecture-self-hosted-streaming-with-runtime-config.md)
- LexerConfig provides runtime keyword/delimiter configurability that logos cannot offer without a two-pass approach — [ADR-0022](adrs/0022-lexer-architecture-self-hosted-streaming-with-runtime-config.md)
- The Lexer trait is the abstraction boundary: the parser consumes Vec<Token> regardless of whether tokens came from… — [ADR-0022](adrs/0022-lexer-architecture-self-hosted-streaming-with-runtime-config.md)
- Building a lexer framework would be over-engineering at this scale. The trait + config + streaming method covers the… — [ADR-0022](adrs/0022-lexer-architecture-self-hosted-streaming-with-runtime-config.md)

### Modular AST: ast-common and handl-ast

- **NodeMetadata** — a marker trait (Default + Clone + Debug) for pluggable node metadata. () is the zero-sized "no… — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- **Node<K, V, M = ()>** — a single AST node parameterized by its **kind** K, its **value type** V, and its **metadata** M — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- metadata is not a fixed Option<Span>/attribute bag; it is whatever M the consumer chooses — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- The runtime uses Node<ExprKind, V, ()> and pays zero bytes for metadata. The parser uses Node<ExprKind, V, Span>. The… — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- **NodeKind traits** — ExprKind, StmtKind, TypeKind marker traits that language-specific enums implement — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- **Generic factory traits** — ExprFactory, StmtFactory, TypeFactory — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- These let callers construct nodes through a trait interface rather than directly calling enum constructors. This is the… — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- **Traversal helpers** — Walk, Fold, and Visit traits that work for any Node<K, V, M> where K implements the kind trait — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- Keeps handl-ast from accumulating ad-hoc visitor code — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- Expr<V> stays generic over the value representation, preserving the rule that the kernel is dependency-free — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- Stmt<V> is added as a top-level program element; statements can contain expressions but not the other way around — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- Type<V> moves here from typecheck-core and typegen-core so the parser, typechecker, and runtime all share the same type… — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- The existing hand-rolled constructors on Expr<V> are kept as convenience methods but re-implemented through… — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- handl-parser continues to return Expr<NonFunctionValue> but can eventually expose a Stmt-based entry point — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- handl-compiler-pass-parse may return Program<Stmt<NonFunctionValue>> once statements exist — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- handl-typecheck-core and handl-typegen-core import Type from handl-ast instead of defining it locally — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)
- handl-vm and handl-runtime keep depending on handl-ast only; they never touch handl-ast-common directly — [ADR-0023](adrs/0023-modular-ast-ast-common-and-handl-ast.md)

### Agnostic parser engine: handl-parser-common and handl-parser-canonical on top

- **ParseError** — a span-aware error value. Carries a message and an optional Span — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- **Rule<K, N>** — one token kind's behavior in the grammar — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- left_bp and right_bp for Pratt precedence — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- nud (null denotation): parse a token with no left context — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- led (left denotation): parse a token with an already-parsed left operand — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- **Grammar<K, N> trait** and **SimpleGrammar<K, N>** — the factory configuration — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- rules: mapping from token kind to Rule — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- eof: an optional end-of-input kind to verify — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- parse_root: start a parse using the ParserContext — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- **ParserContext<K, N>** — the mutable parser state and primitive combinators — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- peek, advance, is_at_end, match_kind, expect for recursive descent — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- expression(min_bp) for Pratt operator-precedence parsing — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- **Parser** — the runtime — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- parse(&self, tokens: &[Token<K>], source: &str) -> Result<Ast<N>, ParseError> — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- Ast { source, root } is generic over N, so a dialect can use Node<ExprKind, V, Span> from handl-ast-common or any other… — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- Define Rules for CanonicalKind::String, CanonicalKind::Number, CanonicalKind::Bool, CanonicalKind::Null,… — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- The recursive structure (calls, arrays, objects, handler clauses) is built using the ParserContext combinators and… — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- It continues to produce Expr<NonFunctionValue> from handl-ast — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- handl-parser keeps the Parser trait as the public boundary for now, but may eventually become a thin re-export over… — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)
- handl-ast and handl-runtime are not touched; they stay downstream of the parser — [ADR-0024](adrs/0024-agnostic-parser-engine-handl-parser-common-and-handl-parser-canonical-on-top.md)

### VM Crate Restructure

- handl-vm is the only abstraction crate. It defines Value, Host, Context, VirtualMachine, Eval, Operation, Error. It is… — [ADR-0027](adrs/0027-vm-crate-restructure.md)
- handl-plugin-vm-values-json is one concrete Value implementation. It depends on handl-vm and serde_json — [ADR-0027](adrs/0027-vm-crate-restructure.md)
- handl-plugin-vm-backend-* crates implement the handl-vm traits. They are allowed to be generic over V but may choose a… — [ADR-0027](adrs/0027-vm-crate-restructure.md)
- lib-* modules become generic over V: Value. They may use handl-plugin-vm-values-json as their default test and… — [ADR-0027](adrs/0027-vm-crate-restructure.md)
- handl-plugin-vm-common holds generic host-building blocks, including Module<V> and ModuleHost<V> (currently… — [ADR-0027](adrs/0027-vm-crate-restructure.md)
- handl-plugin-vm-config, handl-plugin-vm-type, handl-plugin-vm-server, and handl-plugin-vm-transform are generic over V:… — [ADR-0027](adrs/0027-vm-crate-restructure.md)
- The handl-runtime-* package namespace is retired. The runtime and runtime-* directory names are retired — [ADR-0027](adrs/0027-vm-crate-restructure.md)

### ADR-0028: Pluggable Module Composition

- **Replace the static frontend_for(format) and monolithic Dialect** with — [ADR-0028](adrs/0028-adr-0028-pluggable-module-composition.md)
- **Every module folder may carry module.toml** (language kit + per-stage — [ADR-0028](adrs/0028-adr-0028-pluggable-module-composition.md)
- **Naming is handl-*** for kits, lexers, parsers, typecheckers, formatters, — [ADR-0028](adrs/0028-adr-0028-pluggable-module-composition.md)
- **A Composer assembles the pipeline** from the manifest plus each plugin's — [ADR-0028](adrs/0028-adr-0028-pluggable-module-composition.md)
- **All passes are pluggable** — lexer, parser, resolve, typecheck, format, — [ADR-0028](adrs/0028-adr-0028-pluggable-module-composition.md)
- **AST strategy**: plugins may produce different ASTs; a lower/canonicalize — [ADR-0028](adrs/0028-adr-0028-pluggable-module-composition.md)

### ADR 0030: lib-lang — Expose the Handl language as a reusable library

- lang.lexer — tokenization and LexerConfig composition — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lang.parser — expression and statement parsing into the canonical AST — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lang.ast — the canonical AST and traversal utilities — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lang.typecheck — type inference and schema validation, where stable — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lang.compiler — lowering to the canonical VM format — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lang.vm — a configurable, sandboxed VM instance — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lib-template — template parsing and rendering is a consumer of lib-lang — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lib-escaping — context-specific escaping (HTML, URL, CSS, JS, etc.) is a — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)
- lib-utils — generic pure utility functions (math, string transforms, — [ADR-0030](adrs/0030-adr-0030-lib-lang-expose-the-handl-language-as-a-reusable-library.md)

### ADR-0031: Platform Abstractions

- **One crate, handl-platform**, holds the core contracts as modules: — [ADR-0031](adrs/0031-adr-0031-platform-abstractions.md)
- **CapabilityMetadata is opaque serde_json::Value**; the platform — [ADR-0031](adrs/0031-adr-0031-platform-abstractions.md)
- **Default capabilities ship as handl-ext-* plugins** (fmt, lint, typecheck, — [ADR-0031](adrs/0031-adr-0031-platform-abstractions.md)
- **Hosts generate their surface from capability metadata**; the CLI does not — [ADR-0031](adrs/0031-adr-0031-platform-abstractions.md)
- **Existing crates migrate**: host-cli-plugin empties into — [ADR-0031](adrs/0031-adr-0031-platform-abstractions.md)
- **Out of scope** until the layer is stable: Framework, Middleware, — [ADR-0031](adrs/0031-adr-0031-platform-abstractions.md)

### ADR-0035: Loaders as Abstract Plugins

- Create handl-plugin-load — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Define the PluginLoader trait (moved from handl-loader) — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Define the Load operation and a LoaderRule contribution type — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Provide LoaderHostPlugin, a RuleAcceptor that collects LoaderRules and exposes a load capability — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- The load capability accepts a PluginManifest payload, tries each collected PluginLoader, and registers the resulting… — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Rename the existing loader crates to the plugin-loader-* namespace: — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- loader → plugin-load (the host-side rule/operation definitions) — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- loader-native → plugin-loader-native — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- loader-wasm → plugin-loader-wasm — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- loader-web → plugin-loader-web — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Each plugin-loader-* crate implements both Plugin and PluginLoader — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- In contribute, it ensures a LoaderHostPlugin is registered, then contributes a LoaderRule wrapping itself — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Because contribute takes &self, the loader must be Clone or wrap a cheaply-cloneable handle — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- DefaultHost gains no new field or special-case API — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Host code that needs to load an external plugin calls the load capability like any other: — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- handl.toml selects loaders through ordinary [[plugin]] entries, not a custom [[loader]] section: — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)
- Remove handl-loader as a separate framework crate. Its concerns are absorbed by handl-plugin-load and the… — [ADR-0035](adrs/0035-adr-0035-loaders-as-abstract-plugins.md)

### ADR-0036: Host Applier as a Plugin

- **Create handl-plugin-applier** (or a similarly named default applier plugin crate) — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- Provide DefaultApplierPlugin, a Plugin that reads a host config file and applies it to the host — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- In contribute, read .handl.toml (or another supported config source), parse it into HostConfig, and register the… — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- Kits and plugins are loaded by running capabilities (kit/load and load) on the same host, not by calling hard-coded… — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- **Turn BasicKitLoader into a KitLoaderPlugin** or move its behavior into handl-plugin-applier — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- The default applier provides a kit/load capability — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- kit/load accepts a kit name or path and returns a Kit that the applier then registers with the host — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- **The CLI only bootstraps** — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- host-cli gains --applier <id> (default applier.default) and --config <path> (default .handl.toml) — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- It creates a DefaultHost, registers the default applier plugin (and the default kit/loader plugins if they are not… — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- host.load() runs contribute to a fixed point; the applier reads the config and registers the rest — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- **The existing HostApplier trait and ConfigHostApplier struct can be deprecated** — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- HostConfig and KitReference/PluginReference remain in handl-platform as the in-memory model — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)
- The actual _act_ of applying the config moves from handl-platform to a plugin — [ADR-0036](adrs/0036-adr-0036-host-applier-as-a-plugin.md)

### ADR-0037: Plugin Resolution and Excludes

- Collects PluginManifest-like data from the live registry (id, version, capabilities, requires, conflicts, exclusive) — [ADR-0037](adrs/0037-adr-0037-plugin-resolution-and-excludes.md)
- Calls resolve::resolve() with the collected manifests — [ADR-0037](adrs/0037-adr-0037-plugin-resolution-and-excludes.md)
- Returns the resolved composition or an error — [ADR-0037](adrs/0037-adr-0037-plugin-resolution-and-excludes.md)

### ADR-0038: Ignored Test Inventory and Triage

- **Workstream 3 (format/diagnostics)** — the formatter and linter pipelines are already implemented and registered in… — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)
- **Workstream 2 (LSP plugins)** — the LSP plugins are implemented and registered. ToolingHost needs to parse the source,… — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)
- **Workstream 1 (VM eval)** — the largest workstream. Unblocks workstream 6 as a side effect. Requires implementing the… — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)
- **Workstream 7 (loader/CLI)** — small effort once ToolingHost exposes a load entry point — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)
- **Workstream 4 (network/TLS)** — move behind a tls feature flag so they don't run by default. No implementation work… — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)
- **Workstream 5 (concurrent stress)** — gate behind a stress feature flag. Fix the scheduler race separately — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)
- **Workstream 6 (HTTP server function values)** — unblocked once workstream 1 is done — [ADR-0038](adrs/0038-adr-0038-ignored-test-inventory-and-triage.md)

### ADR-0039: Kits Are Curated Plugin Lists, Not Implementations

- **A kit is a curated list of plugins, not an implementation.** It selects and — [ADR-0039](adrs/0039-adr-0039-kits-are-curated-plugin-lists-not-implementations.md)
- **Composition is dynamic**: a kit is resolved when the host loads it, not — [ADR-0039](adrs/0039-adr-0039-kits-are-curated-plugin-lists-not-implementations.md)
- **Plugins offer capabilities; kits only curate them** — the capability — [ADR-0039](adrs/0039-adr-0039-kits-are-curated-plugin-lists-not-implementations.md)
- **Naming principle**: a kit is named for the surface it curates — [ADR-0039](adrs/0039-adr-0039-kits-are-curated-plugin-lists-not-implementations.md)
- **Migration is four phases**: thin kit-canonical, separate kits from hosts, — [ADR-0039](adrs/0039-adr-0039-kits-are-curated-plugin-lists-not-implementations.md)

### ADR-0040: VM is an Operation, Not a Subsystem

- **Dissolve plugin-vm-* and plugin-runtime-*.** Evaluation uses the — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **No VirtualMachine trait, no Value trait, no Runtime operation, no V — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **Eval handlers are granular Behavior<Eval> plugins**, one per AST kind, — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **EvalRequest carries EvalContext** (scopes, evaluator handle, modules, — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **EvalContext lives in plugin-ext-eval**, EvalRequest in platform; — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **Function registration collapses to Module/ModuleRuntime** with a — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **There is no bundled interpreter target**: the 11 plugin-eval-* plugins are — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)
- **One cutover.** The old VM/runtime crates and lego.rs — [ADR-0040](adrs/0040-adr-0040-vm-is-an-operation-not-a-subsystem.md)

### ADR-0041: Languages as First-Class Values

- **Language is a handl-level type** — a language definition is a value the — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)
- **eval is a handl function** — evaluating an AST is a function call, not a — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)
- **self is the running language**, handed to the program, so a program can — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)
- **Language combinators are handl functions** — composing languages is — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)
- **The bootstrap is a compilation step, not interpretation** — the initial — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)
- **Structured by default; vau is a typed escape** for the cases structure — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)
- **Capability discipline at the language level** — a language declares what — [ADR-0041](adrs/0041-adr-0041-languages-as-first-class-values.md)

### ADR-0044: Language as Data and Runtime Split

- **Language is pure serializable data**, built by merging typed fragments — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)
- **Staged entry points**: each stage consumes only the previous stage's output — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)
- **Crate structure**: each plugin-engine-* stage splits into an abstraction — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)
- **Evaluation schemes are data** — the eval fragment set maps each Ast — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)
- **Fragment merge distinguishes override from collision** via id + — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)
- **Authored data files are canonical.** Rust emitters are a temporary — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)
- **Five review properties are required**: composability (requires), — [ADR-0044](adrs/0044-adr-0044-language-as-data-and-runtime-split.md)

### ADR-0046: Capability Discipline at the Language Level

- **CapabilitySet summarizes a Language statically** — host function — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)
- **It is computed by scanning eval schemes**: Foreign adds a name, Vau — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)
- **CapabilityPolicy declares what a context allows** — host fns, vau, — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)
- **A static check at the eval/run/compile boundary** denies before — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)
- **LanguageRegistry carries policies**, attached at registration time by — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)
- **Sub-runtimes inherit a narrowed policy** — a sandboxed language cannot — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)
- **capabilities(handle) is a read-only host function** for handl programs, — [ADR-0046](adrs/0046-adr-0046-capability-discipline-at-the-language-level.md)

### ADR-0047: Meta Functions as plugin-lib-* Crates

- **Five new leaf crates, one operation each** — eval, lex, parse, — [ADR-0047](adrs/0047-adr-0047-meta-functions-as-plugin-lib-crates.md)
- **handl-runtime becomes the abstraction + Runtime impl**; engines — [ADR-0047](adrs/0047-adr-0047-meta-functions-as-plugin-lib-crates.md)
- **compile / run_compiled are leaf-able** via CompiledHandle and — [ADR-0047](adrs/0047-adr-0047-meta-functions-as-plugin-lib-crates.md)
- **run is dropped** — it is composition of the other operations, not a — [ADR-0047](adrs/0047-adr-0047-meta-functions-as-plugin-lib-crates.md)
- **self stays an ambient global**, handed to the program — [ADR-0047](adrs/0047-adr-0047-meta-functions-as-plugin-lib-crates.md)
- **Scope types re-export from plugin-lib-eval**, so there is one type per — [ADR-0047](adrs/0047-adr-0047-meta-functions-as-plugin-lib-crates.md)

### ADR-0048: Modules as First-Class Values

- a **name** (the namespace prefix, e.g. "math"), and — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- a **function table**: a map from function name to a callable value — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- a **closure handle** — a tagged value {"__closure__": id} referencing — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- a **foreign name** — a tagged value {"__foreign__": "math.add"} — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- module.define(name, functions) → a Module value. functions is a — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- module.register(mod) → registers a Module value into the _current_ — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- module.lookup(qualified_name) → the function value (a closure handle — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)
- module.functions(mod) → the function table as a handl object, for — [ADR-0048](adrs/0048-adr-0048-modules-as-first-class-values.md)

### ADR-0049: Modules are Evaluated Environments

- **Environments are first-class values** — a module is the environment its — [ADR-0049](adrs/0049-adr-0049-modules-are-evaluated-environments.md)
- **import / export / module / field-access move off Foreign** and — [ADR-0049](adrs/0049-adr-0049-modules-are-evaluated-environments.md)
- **The import resolver is the loader** — resolution and evaluation are one — [ADR-0049](adrs/0049-adr-0049-modules-are-evaluated-environments.md)
- **The stdlib is .hnl files** evaluated like user modules — [ADR-0049](adrs/0049-adr-0049-modules-are-evaluated-environments.md)
- **The module system is written in handl**, on top of the environment value — [ADR-0049](adrs/0049-adr-0049-modules-are-evaluated-environments.md)
- **Per-language module systems**: each language defines its own import/export — [ADR-0049](adrs/0049-adr-0049-modules-are-evaluated-environments.md)

### ADR-0050: Plugin System in Handl

- **Two permanent Rust layers; everything above is handl.** The FFI/binding — [ADR-0050](adrs/0050-adr-0050-plugin-system-in-handl.md)
- **The plugin system is a handl module, not a Rust crate** — registry, — [ADR-0050](adrs/0050-adr-0050-plugin-system-in-handl.md)
- **fmt and lint are handl rule acceptors** with one rule file per AST — [ADR-0050](adrs/0050-adr-0050-plugin-system-in-handl.md)
- **handle fmt|lint dispatches to the handl tooling kit**, not to Rust — [ADR-0050](adrs/0050-adr-0050-plugin-system-in-handl.md)
- **Build order**: binding façade → ast.parse/ast.lex access → handl — [ADR-0050](adrs/0050-adr-0050-plugin-system-in-handl.md)

### ADR-0051: ffi-rust — Generic Rust Binding Façade

- **Async functions.** A variant AsyncHostFn (Box<dyn AsyncFunction>) and a — [ADR-0051](adrs/0051-adr-0051-ffi-rust-generic-rust-binding-facade.md)
- **Stateful modules.** A ModuleBinding may carry &'static dyn Module directly (an — [ADR-0051](adrs/0051-adr-0051-ffi-rust-generic-rust-binding-facade.md)
- **Existing plugin-lib-* crates are not deleted.** They continue to work unchanged — [ADR-0051](adrs/0051-adr-0051-ffi-rust-generic-rust-binding-facade.md)

### ADR-0052: Arrow Calculus Kernel

- **app is the primitive; interpretation is the only execution mechanism.** — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Semantic principle: arrows are values** and computation is composition — — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Kernel representation**: Term/Value with the arrow constructors — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **The lowering table (surface construct → arrow form) is normative**: every — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Derived forms** (let, if, loops) are sugar over the kernel, not — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **proc notation and x = y binding are the authoring surface**; they — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Big-bang migration**: one cutover, the old Scheme removed, no dual path — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Handlers are outside the calculus**; execution is one interpretation of — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Feedback semantics** (causal/strict vs lazy/fixpoint) are specified — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)
- **Scope**: kernel + bootstrap + canonical proc surface; everything else — [ADR-0052](adrs/0052-adr-0052-arrow-calculus-kernel.md)

### ADR-0053: Generic Pratt Parser — Grammar Data Replaces Per-Construct Branches

- **TokenKindDef.literal: Option<LiteralKind>** replaces the — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **AstShapeRef.child_separators: Vec<ChildSeparator>** replaces the — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **DelimitedShape.open_is_operator_token: bool** — the open delimiter is — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **DelimitedShape.flatten: bool** splices elements into the parent's — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **DelimitedShape.rest_token: Option<String>** replaces the — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **The infix path consults delimited**, mirroring the prefix path — the — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **Parser desugars move to a data-driven post-parse pass.** if becomes a — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)
- **Contextual forms (match, data, from) are closed by grammar data** — — [ADR-0053](adrs/0053-adr-0053-generic-pratt-parser-grammar-data-replaces-per-construct-branches.md)

### ADR-0054: Generic Typecheck Engine (plugin-tool-typecheck + plugin-tool-typecheck-nbe)

- **Type rules reuse Scheme — lang.types.** A rule synthesizes a node's — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)
- **Judgment ops are Arr ops** (unify, quote, normalize, meta-solve, — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)
- **plugin-tool-typecheck is the abstraction crate** — marker, — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)
- **plugin-tool-typecheck-nbe implements TypecheckEngine.** NBE — — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)
- **Typecheck is wired on RuntimeHandle (pure path).** Types are values, so — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)
- **plugin-lib-lang-typecheck is the meta leaf** exposing — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)
- **Rules are authored data** in crates/handl-kit-data-canonical/data/types.yaml, — [ADR-0054](adrs/0054-adr-0054-generic-typecheck-engine-plugin-tool-typecheck-plugin-tool-typecheck-nbe.md)

### ADR-0055: Tagged Values

- **Tag registries** define the legal tags per host; a tag is registered once — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)
- **tag / match_tag / tag_registry are host capabilities**, not syntax — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)
- **Representation**: a tagged value is a serde_json::Value carrying its tag — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)
- **Two equality flavors, one primitive**: structural equality and — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)
- **Forgery is impossible** — a tag can only be minted through the registered — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)
- **Sugar**: type (and future unit) declares tags; the primitive is the — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)
- **Untagged values are unchanged**; tagging is additive — [ADR-0055](adrs/0055-adr-0055-tagged-values.md)

### ADR-0056: Import / Export and Multi-File Programs

- **A module is an environment value** — evaluating a file produces the — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)
- **import resolves source and caches the resulting environment** (resolve → — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)
- **export defines a module's public surface**; it is not a visibility — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)
- **field-access is the module access mechanism** — module.name reads the — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)
- **Import capability is a policy boundary**, not a language concept: the host — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)
- **The stdlib becomes .hnl files** evaluated into environments — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)
- **The runtime exposes environments as values**, so modules are inspectable — [ADR-0056](adrs/0056-adr-0056-import-export-and-multi-file-programs.md)

### ADR-0058: Generic Data-Driven Engines

- Looks up a node's eval scheme from Language.eval (already data-driven) — [ADR-0058](adrs/0058-adr-0058-generic-data-driven-engines.md)
- Interprets the scheme's Arr/Seq/First/App combinators (already — [ADR-0058](adrs/0058-adr-0058-generic-data-driven-engines.md)
- Dispatches native ops to **data-specified behavior**, not hardcoded — [ADR-0058](adrs/0058-adr-0058-generic-data-driven-engines.md)
- **Desugar rules** (for syntactic rewrites — [k]: → computed key, — [ADR-0058](adrs/0058-adr-0058-generic-data-driven-engines.md)
- **Scheme descriptions** (for eval-time behavior — new native ops are — [ADR-0058](adrs/0058-adr-0058-generic-data-driven-engines.md)

### ADR-0059: Generic Object Key Evaluation

- **var or string** — use the payload string directly (literal key) — [ADR-0059](adrs/0059-adr-0059-generic-object-key-evaluation.md)
- **anything else** — evaluate the expression and use its value's string — [ADR-0059](adrs/0059-adr-0059-generic-object-key-evaluation.md)

### ADR-0060: Generic Call Dispatch

- id: eval.call — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)
- !Arr { op: proj_child, args: [], source: Native, lit: 0 } — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)
- !Arr { op: proj_child, args: [], source: Native, lit: 1 } — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)
- **typeof** — a native op under arr that returns a type tag string — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)
- **route_on_type** — a native op under arr that takes a value and — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)
- **call_host** — a native op that takes a string callee and an args — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)
- **ArrSource::DynamicForeign** — a new variant of ArrSource — [ADR-0060](adrs/0060-adr-0060-generic-call-dispatch.md)

### ADR-0061: Generic Pattern Match

- **define_binding** — bind a name to a value in the current scope — [ADR-0061](adrs/0061-adr-0061-generic-pattern-match.md)
- **eval_pattern_node** — evaluate a pattern AST node as a runtime — [ADR-0061](adrs/0061-adr-0061-generic-pattern-match.md)
- **Desugar patterns into match-op AST nodes.** Each pattern form — [ADR-0061](adrs/0061-adr-0061-generic-pattern-match.md)
- **Express match/handle as Loop + Choice.** The arm-by-arm — [ADR-0061](adrs/0061-adr-0061-generic-pattern-match.md)
- **The shared try_pattern becomes the try_pattern_arm native op.** — [ADR-0061](adrs/0061-adr-0061-generic-pattern-match.md)

### ADR-0063: Scheme-Core Completeness for Self-Hosting

- **Genuine host effect** (I/O, FFI, capability-tracked) — keep — [ADR-0063](adrs/0063-adr-0063-scheme-core-completeness-for-self-hosting.md)
- **Meta-operation masquerading as host function** (eval, parse, — [ADR-0063](adrs/0063-adr-0063-scheme-core-completeness-for-self-hosting.md)

### ADR-0065: Derived-Artifact Cache and Compression

- Every fragment source (YAML file content hash, or handl program content — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- The kit manifest (ordered fragment-source list + include/exclude filters) — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- The extends chain (transitive: parent kits' manifests, recursively) — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- Engine selections (lex/parse/eval engine names + versions, per ADR-0044 — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- The scheme_version declared by the Language (ADR-0044 §7) — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- NoCache — always misses (default for compile-time-embedded kits) — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- InMemoryCache — HashMap<CacheKey, CachedLanguage>, process-lifetime — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- OnDiskCache — files in a cache directory, persistent across runs — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)
- (future) NetworkCache — shared cache for a fleet of hosts — [ADR-0065](adrs/0065-adr-0065-derived-artifact-cache-and-compression.md)

### ADR-0066: Dependency-Graph Chunking

- Its fragment data (serde-serializable, per ADR-0044) — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Its dependencies: references to other chunks (by CacheKey, per — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Its replaces targets (so the override graph can be resolved across — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Decompress the chunk — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Resolve dependencies: load each dependency chunk (recursively, depth-first) — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Merge the loaded chunks' fragments (ADR-0044 §5 merge semantics) — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Typecheck the loaded subset (ADR-0044 §1 AstShapeDef payload schemas, — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)
- Eval — [ADR-0066](adrs/0066-adr-0066-dependency-graph-chunking.md)

### ADR-0067: Chunk Certificates

- **A certificate is an Ed25519 detached signature** over the canonical — [ADR-0067](adrs/0067-adr-0067-chunk-certificates.md)
- **Verification happens before load**: recompute the hash, verify the — [ADR-0067](adrs/0067-adr-0067-chunk-certificates.md)
- **Dependency certificates are required** and pin each dependency's content — [ADR-0067](adrs/0067-adr-0067-chunk-certificates.md)
- **Author keys are distributed out of band** — key server, pinned host — [ADR-0067](adrs/0067-adr-0067-chunk-certificates.md)
- **Certificates complement capability checks**: ADR-0046 inspects what the — [ADR-0067](adrs/0067-adr-0067-chunk-certificates.md)

### ADR-0069: Arrow-Native Modules

- **A file is an arrow.** A module is the arrow its file evaluates to, not a — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)
- **Import normalizes to projection**: importing a name is selecting a field — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)
- **Using a name is App** — a reference is application of a projection, not — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)
- **Two caches: graphs and contracts.** Normalized module graphs and their — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)
- **Normalization is one abstraction with many implementations**; the — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)
- **No export, no visibility, no environments.** Those concepts disappear — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)
- **Multi-file programs are arrow composition** — linking is composition — [ADR-0069](adrs/0069-adr-0069-arrow-native-modules.md)

### ADR-0070: Refinement Inference Scope — Deterministic Dataflow

- **List operations**: sort, map, filter, fold, insert, take, drop, zip, — [ADR-0070](adrs/0070-adr-0070-refinement-inference-scope-deterministic-dataflow.md)
- **Parsers (recursive descent, no backtracking)**: Pratt parsing, — [ADR-0070](adrs/0070-adr-0070-refinement-inference-scope-deterministic-dataflow.md)
- **State machines (data invariants)**: counters, bounded buffers, — [ADR-0070](adrs/0070-adr-0070-refinement-inference-scope-deterministic-dataflow.md)
- **Parsers with backtracking**: need failure as a first-class effect — [ADR-0070](adrs/0070-adr-0070-refinement-inference-scope-deterministic-dataflow.md)
- **Concurrent systems**: need non-deterministic scheduling and — [ADR-0070](adrs/0070-adr-0070-refinement-inference-scope-deterministic-dataflow.md)
- **Non-deterministic computation**: the inference assumes the graph — [ADR-0070](adrs/0070-adr-0070-refinement-inference-scope-deterministic-dataflow.md)

### ADR-0072: Concurrent Arrow Calculus

- receive requires: channel non-empty (precondition) — [ADR-0072](adrs/0072-adr-0072-concurrent-arrow-calculus.md)
- send guarantees: channel non-empty after (postcondition) — [ADR-0072](adrs/0072-adr-0072-concurrent-arrow-calculus.md)

### ADR-0077: Stdlib Migration to Handl — op Declarations, Axioms, and the Elimination of Per-Library Rust

- The stdlib is handl source. Every plugin-lib-* library is a .hnl file — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- Every public arrow carries a --- doc comment (the API contract in prose) — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- A pure-handl arrow needs no proof file: the contract cache infers each arrow — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- op declarations are the FFI surface — op "name" : A -> B guarantee: … — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- Every migrated file declares its capabilities and follows the — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- The six-phase port plan, the per-file requirements and the per-library — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- Native bodies are handl over direct C/POSIX FFI (ADR-0083) — no Rust — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)
- Each library's public API is preserved across its migration; a breaking — [ADR-0077](adrs/0077-adr-0077-stdlib-migration-to-handl-op-declarations-axioms-and-the-elimination-of-per-library-rust.md)

### ADR-0081: Data-Driven AST Child Roles — Closing the Last Engine Hardcoding

- id: ast.assign — [ADR-0081](adrs/0081-adr-0081-data-driven-ast-child-roles-closing-the-last-engine-hardcoding.md)
- role: target — [ADR-0081](adrs/0081-adr-0081-data-driven-ast-child-roles-closing-the-last-engine-hardcoding.md)
- role: value — [ADR-0081](adrs/0081-adr-0081-data-driven-ast-child-roles-closing-the-last-engine-hardcoding.md)
- id: ast.block — [ADR-0081](adrs/0081-adr-0081-data-driven-ast-child-roles-closing-the-last-engine-hardcoding.md)
- role: statements — [ADR-0081](adrs/0081-adr-0081-data-driven-ast-child-roles-closing-the-last-engine-hardcoding.md)

### ADR-0083: FFI-Native Stdlib — Handl over Direct C/POSIX Bindings

- **ffi_bind is a host-exposed arrow op.** A binding is data — library, — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **Marshalling is a closed ABI vocabulary**: scalars, pointers, structs, — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **Errors are declared per binding.** An optional error clause maps errno — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **A Rust FFI dispatcher does only the mechanics**: load the library and — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **The abstraction layer is handl**: POSIX wrappers, constants and the — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **Platform variants are abstracted from day one** (Linux first) and chosen — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **Rust keeps only the kernel**: dispatcher, module loader, engines, host and — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **What is eliminated**: per-library Rust Function bodies and the — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **The async runtime is handl-authored from day one**: epoll_wait for — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)
- **Kernel freedom**: the kernel knows nothing of POSIX, libc or any library — — [ADR-0083](adrs/0083-adr-0083-ffi-native-stdlib-handl-over-direct-c-posix-bindings.md)

### ADR-0085: Bootstrap Tooling Strategy — Rust Tooling on the Generic Substrate

- **The substrate stays and is the implementation.** The plugin system, — [ADR-0085](adrs/0085-adr-0085-bootstrap-tooling-strategy-rust-tooling-on-the-generic-substrate.md)
- **Best practices in Rust, now.** Composability, extensibility, — [ADR-0085](adrs/0085-adr-0085-bootstrap-tooling-strategy-rust-tooling-on-the-generic-substrate.md)
- **Canonical style/rules are hardcoded; the machinery is generic.** What — [ADR-0085](adrs/0085-adr-0085-bootstrap-tooling-strategy-rust-tooling-on-the-generic-substrate.md)
- **Minimal decency, not prototyping quality.** Small module boundaries — [ADR-0085](adrs/0085-adr-0085-bootstrap-tooling-strategy-rust-tooling-on-the-generic-substrate.md)
- **The CLI stays thin.** host-cli gains/keeps subcommands that dispatch — [ADR-0085](adrs/0085-adr-0085-bootstrap-tooling-strategy-rust-tooling-on-the-generic-substrate.md)
- **Port to handl in the future, improve in place until then.** When handl — [ADR-0085](adrs/0085-adr-0085-bootstrap-tooling-strategy-rust-tooling-on-the-generic-substrate.md)

### ADR-0086: Bootstrap Formatter (plugin-ext-fmt)

- **Input/output:** surface source text → parse (via — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- **Hardcoded style, no configuration.** Canonical preferences: — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- Two-space indentation; continuation lines align with enclosing arrows — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- One statement per line inside graph blocks; a blank line between — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- Single trailing newline; no trailing whitespace; no more than one — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- >>> chains break _before_ the operator, one segment per line, when a — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- No spaces inside brackets; single spaces around binary operators per — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- **Idempotence:** fmt(fmt(src)) == fmt(src) — tested exhaustively — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- **API surface (minimal):** — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- format_source(src: &str) -> Result<String, FormatError> — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- check_source(src: &str) -> Result<bool, FormatError> (true = already — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- **CLI contract:** hnl fmt [--check] [--write] [paths...]. --check — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)
- **Testing:** 100% coverage. Golden-file tests under tests/golden/ (one — [ADR-0086](adrs/0086-adr-0086-bootstrap-formatter-plugin-ext-fmt.md)

### ADR-0087: Bootstrap Linter (plugin-ext-lint)

- **Core types:** re-exported from plugin-ext-diag (ADR-0091) — this ADR — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- **Initial rule set** (all v0.1-surface-specific): — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- unused-import (Warning, fixable) — import projection never referenced — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- unbound-wire (Error) — reference to a name not bound in the enclosing — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- shadowed-binding (Warning) — re-binding a name already in scope — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- dead-branch (Warning) — choice arm guarded by a constant condition — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- naming-convention (Warning, fixable) — bindings are snake_case, — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- missing-module-doc (Info) — a module lacks a leading doc comment — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- fmt (Warning, fixable) — file is not canonical per ADR-0086; the fix — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- **Exit contract:** exit 1 iff any Error diagnostic; --fail-on-warn — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- **Rule architecture:** rules are pure functions — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)
- **Testing:** 100% coverage; every rule has positive/negative/fix cases; — [ADR-0087](adrs/0087-adr-0087-bootstrap-linter-plugin-ext-lint.md)

### ADR-0088: Bootstrap Typechecker (plugin-tool-typecheck)

- **Scope (v0.1):** — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- Arrow kind checking of composed graphs (arr/>>>/first/choice — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- Structural type inference for literals, records, positional — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- rec region well-formedness: feedback wires are consumed exactly — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- Module-level checks: every import projection used, every exported — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- **Not in scope (deferred to RFC-0004):** trait/impl checking (ADR-0075), — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- **API surface:** — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- **Architecture:** single pass composition — collect bindings → infer — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)
- **Testing:** 100% coverage. Table-driven tests/ per typing rule from — [ADR-0088](adrs/0088-adr-0088-bootstrap-typechecker-plugin-tool-typecheck.md)

### ADR-0089: Bootstrap Test Runner (plugin-tool-test)

- **A new plugin-tool-test crate owns the runner** — discovery, index, — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **plugin-lib-test becomes the framework library** (assert/property/ — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **Stable pipeline contracts**: Providers → Discovery → Index/Cache → Filter — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **Core types are hard to change and versioned**; internal contracts are — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **Test declaration is a merged language fragment**, not the test_* — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **Providers are extensible from day one**; discovery/index and filtering — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **Planner and scheduler are separate; the scheduler is generic**; the — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)
- **Determinism is a hard invariant**; observability emits structured events — [ADR-0089](adrs/0089-adr-0089-bootstrap-test-runner-plugin-tool-test.md)

### ADR-0090: Bootstrap CLI (host-cli)

- **Command surface (v0.1):** — [ADR-0090](adrs/0090-adr-0090-bootstrap-cli-host-cli.md)
- **No _new_ plugin system.** No capability registry, no dynamic loading, — [ADR-0090](adrs/0090-adr-0090-bootstrap-cli-host-cli.md)
- **Coupling policy:** the CLI is allowed to know about every tool crate, — [ADR-0090](adrs/0090-adr-0090-bootstrap-cli-host-cli.md)
- **Exit codes:** 0 success; 1 check/lint/test failure; 2 usage or — [ADR-0090](adrs/0090-adr-0090-bootstrap-cli-host-cli.md)
- **Diagnostics:** all subcommands print through the shared renderer in — [ADR-0090](adrs/0090-adr-0090-bootstrap-cli-host-cli.md)
- **Testing:** 100% coverage of argument parsing, dispatch, exit-code — [ADR-0090](adrs/0090-adr-0090-bootstrap-cli-host-cli.md)

### ADR-0091: Shared Diagnostics Crate (plugin-ext-diag)

- **Types:** Diagnostic { rule, severity, span, message, fix? }, — [ADR-0091](adrs/0091-adr-0091-shared-diagnostics-crate-plugin-ext-diag.md)
- **Rendering:** one shared renderer (human `file:line:col: severity — [ADR-0091](adrs/0091-adr-0091-shared-diagnostics-crate-plugin-ext-diag.md)
- **Fix application:** span-ordered, idempotent text-edit application with — [ADR-0091](adrs/0091-adr-0091-shared-diagnostics-crate-plugin-ext-diag.md)
- **Relationship to plugin-diag-expression:** it keeps its — [ADR-0091](adrs/0091-adr-0091-shared-diagnostics-crate-plugin-ext-diag.md)
- **Generic, not surface-coupled:** the crate knows spans and text, not the — [ADR-0091](adrs/0091-adr-0091-shared-diagnostics-crate-plugin-ext-diag.md)

### ADR-0092: Test Framework in Handl

- **Target layout**: the framework lives in handl-packages/test/ as handl — [ADR-0092](adrs/0092-adr-0092-test-framework-in-handl.md)
- **Migration per op**: each framework operation moves from Rust to a handl — [ADR-0092](adrs/0092-adr-0092-test-framework-in-handl.md)
- **Crate boundary**: plugin-lib-test is deleted; the runner — [ADR-0092](adrs/0092-adr-0092-test-framework-in-handl.md)
- **Relationship to ADR-0089**: the runner stays Rust; this ADR moves only the — [ADR-0092](adrs/0092-adr-0092-test-framework-in-handl.md)
- **Relationship to ADR-0077**: the framework is part of the stdlib migration; — [ADR-0092](adrs/0092-adr-0092-test-framework-in-handl.md)
- **What stays Rust**: the test runner only — [ADR-0092](adrs/0092-adr-0092-test-framework-in-handl.md)

### ADR-0093: String / Char / Unicode Layering

- str-handl calls only char.* semantic atoms. It never indexes bytes, — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- char names are _semantic_: is_whitespace, is_alpha, to_upper, etc — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- Indices in char and str are character (scalar) indices, not byte offsets — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- char.chars and char.from_chars are the bulk conversion atoms. They make — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- unicode-c provides the subset computable from char.* alone exactly, and — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- kit-canonical resolves the module name "str" to str-handl through its — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- Non-canonical kits that do not wire the char resolver still get the FFI — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- "char" resolves to CharModule (plugin-lib-char-native) directly in — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- "unicode_c" resolves to the unicode_c table module (unicode-c.hnl), — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)
- The module name is unicode_c, **not** unicode (a deliberate surface — [ADR-0093](adrs/0093-adr-0093-string-char-unicode-layering.md)

### ADR-0094: Generic AST Transform Engine (plugin-tool-transform + plugin-tool-transform-rewrite)

- **Two crates, mirroring the exec split.** plugin-tool-transform is the — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)
- **TransformDef is a first-class Fragment in handl-language.** Its — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)
- **The rewrite vocabulary generalizes AstDesugarChild.** Keep, KeepPath, — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)
- **Traversal is part of the pass definition** — BottomUp, TopDown or — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)
- **RuleSource is the seam.** The default reads lang.transforms; an — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)
- **Desugar migrates onto the engine.** handl-runtime::desugar_pass becomes a — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)
- **Crate layout has no cycles**: contract/vocabulary, rewrite implementation, — [ADR-0094](adrs/0094-adr-0094-generic-ast-transform-engine-plugin-tool-transform-plugin-tool-transform-rewrite.md)

### ADR-0095: Formatter as a Production Pipeline

- **Crate boundary**: the formatter keeps the abstraction/engine split — — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Pipeline and stage contracts**: parse → layout → emit, with stable — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Rule seam**: FormattingRule is replaceable and composable; formatting — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Config seam**: formatting options are resolved once and passed as a — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Engine seam**: NodeEmitter stays the swap point for a different layout — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Generic capability**: platform/format.rs is either wired to the real — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Language-as-data style**: no hardcoded kind strings; the formatter reads — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)
- **Performance**: parallel --write/--check across files and a — [ADR-0095](adrs/0095-adr-0095-formatter-as-a-production-pipeline.md)

### ADR-0096: Module Resolution — Path-Based Imports + .handl.toml Path Dependencies

- **Dependency specifier.** If the first /-separated segment of <spec> — [ADR-0096](adrs/0096-adr-0096-module-resolution-path-based-imports-handl-toml-path-dependencies.md)
- **Relative path.** Otherwise <spec> resolves against the directory of — [ADR-0096](adrs/0096-adr-0096-module-resolution-path-based-imports-handl-toml-path-dependencies.md)

### ADR-0097: Lint Rules as Language Data

- **LintRuleDef fragment** (handl-language::fragments::lint_rule), — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)
- **Two check kinds**, mirroring ADR-0094's declarative/native split: — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)
- check: !Match { node_kind, when? } — node-local matching. when is a — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)
- check: !Native "name" — the disciplined escape hatch, resolved — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)
- **Module-level engine API** in plugin-tool-lint: — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)
- **Canonical natives** ship in plugin-tool-lint — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)
- **Canonical rules live in handl-kit-data-canonical/data/lints.yaml**, — [ADR-0097](adrs/0097-adr-0097-lint-rules-as-language-data.md)

### ADR-0098: One Language, Multiple Interpreters — A Graded Arrow Calculus

- **Everything is an arrow, and the arrow language is graded.** Grades are — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **The graded-theory map is the grade vocabulary** — each grade names its — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **plugin-engine is the base Term language plus the Model trait** that — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **All primitives and combinators are optional per runtime**; a model — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **The arrow-handler algebra is data-driven** and interprets Term; handlers — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **Scheme/Term is the calculus**, with a fixed shape; interpreters — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **Surface arrow combinators are Term sugar**; bounded repetition is a — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **Sorts are grade records**; the data keyword is removed and inductive — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **Generics are Generic tagged applications over grades**, not a separate — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)
- **Effects stay structural and handlers composable**: an op declares its — [ADR-0098](adrs/0098-adr-0098-one-language-multiple-interpreters-a-graded-arrow-calculus.md)

### ADR-0099: Principled Boundaries Between plugin-engine-* and plugin-ext-* Crates

- plugin-engine-<name> crates are **generic, modular, composable engines** — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- They operate on handl_language::Language data and are not tied to the canonical handl v0.1 grammar — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- They contain no canonical rule sets, no embedded handl-kit-data-canonical data, and no hnl subcommand wiring — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- They may expose operation markers, traits, and abstract request/response types (e.g. Typecheck, TypecheckEngine,… — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- A single engine crate may contain multiple internal modules/algorithms. The typechecker engine, for example, is one… — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- plugin-ext-<name> crates are the **canonical handl bootstrap extension** for a tool — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- They depend on the matching plugin-engine-<name> crate and on handl-kit-data-canonical — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- They supply the canonical rules, language assembly, and the hnl <name> capability wiring — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- They are the only crates that may embed canonical handl rule data or register a hnl subcommand — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- Existing violations must be remediated: — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- Create plugin-ext-typecheck and move the canonical check_module entry point out of plugin-tool-typecheck.… — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- Merge plugin-tool-typecheck-nbe into plugin-tool-typecheck as an internal nbe module — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- Convert the arrow-contract inference algorithm from plugin-ext-refinement into a refinement module inside… — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- Add a composite (or pipeline) module in plugin-tool-typecheck that runs nbe then refinement on the appropriate fragments — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)
- No plugin-ext-* crate may implement a generic algorithm. No plugin-engine-* crate may expose a bin target or a hnl… — [ADR-0099](adrs/0099-adr-0099-principled-boundaries-between-plugin-engine-and-plugin-ext-crates.md)

### ADR-0100: Canonical Term Arrows

- **The canonical internal form is the arrow-calculus Term** — no — [ADR-0100](adrs/0100-adr-0100-canonical-term-arrows.md)
- **The constructor set is fixed**: Arr, Const, Op, Seq, First, — [ADR-0100](adrs/0100-adr-0100-canonical-term-arrows.md)
- **x ~> body lowers point-free** using arr and the product combinators; — [ADR-0100](adrs/0100-adr-0100-canonical-term-arrows.md)
- **Calls lower to App** when the callee is dynamic and to Seq when it is — [ADR-0100](adrs/0100-adr-0100-canonical-term-arrows.md)
- **Records, operators and literals lower to Op/Arr/Const**; there are — [ADR-0100](adrs/0100-adr-0100-canonical-term-arrows.md)
- **The engine interprets Term directly**; eval.yaml describes how each — [ADR-0100](adrs/0100-adr-0100-canonical-term-arrows.md)

### ADR-0101: The Directed-HoTT Limit of the Term Calculus

- **Term is a 2-category**: objects are sorts, 1-cells are arrows, 2-cells — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Language is a presented 2-category** — its data (sorts, arrows, rewrites) — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **A Model is a 2-functor** from the presented theory to a semantic — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **The model family** (value, type, exec, meta, transform) are 2-functors — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Syntax for rewrites and paths** is data (law, rewrite, Path), not — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Univalence and transport** are 1-cell operators derived from the — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Grades are propositions** — a grade is a proposition about a cell, and — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Directed paths**: Term is a directed (∞,1)-category; the 2-cells are — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Implementation**: add Term::Rewrite, law/rewrite declarations, — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)
- **Keep the 1-categorical engine**: evaluation never requires 2-cells; a — [ADR-0101](adrs/0101-adr-0101-the-directed-hott-limit-of-the-term-calculus.md)

### Wiring .hnl modules into HandlNativeLint and HandlNativeRewrite

- Extend handl-kit-canonical to track every .hnl-backed HandlModule it registers, alongside the existing ModuleRegistry.… — [ADR-0102](adrs/0102-wiring-hnl-modules-into-handlnativelint-and-handlnativerewrite.md)
- Expose Kit::handl_lint_adapter() and Kit::handl_rewrite_adapter() methods that build fully populated HandlNativeLint… — [ADR-0102](adrs/0102-wiring-hnl-modules-into-handlnativelint-and-handlnativerewrite.md)
- Wire the lint adapter into plugin-ext-lint::lint_engine and host-tooling::diagnose by adding it as a resolver on the… — [ADR-0102](adrs/0102-wiring-hnl-modules-into-handlnativelint-and-handlnativerewrite.md)
- Wire the rewrite adapter into host-cli transform by layering it over the desugar.* native registry — [ADR-0102](adrs/0102-wiring-hnl-modules-into-handlnativelint-and-handlnativerewrite.md)
- Add a RewriteEngine::with_desugar_and_handl_natives constructor that installs both the handl-native registry and the… — [ADR-0102](adrs/0102-wiring-hnl-modules-into-handlnativelint-and-handlnativerewrite.md)

### ADR-0103: The Kernel Is the Evaluator, Not the Calculus

- **The kernel is the evaluator, not the calculus**: a bytecode VM over values, — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **The ISA is small and structural**; anything expressible as data is not a — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **Computation is represented as data** (TermData over serde_json::Value), — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **exec_model.hnl** is the scheme interpreter over VM primitives — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **arrow_model.hnl** implements all Term tags of the arrow calculus — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **type_model.hnl** implements the HoTT type judgments — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **meta_model.hnl** implements module loading and evaluation — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **The categorical picture**: composition and the arrow primitives are the — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)
- **Rust keeps the VM and the model adapters**; the models are handl data — [ADR-0103](adrs/0103-adr-0103-the-kernel-is-the-evaluator-not-the-calculus.md)

### ADR-0105: Typechecker in Handl

- **The typechecker's models are handl**; Rust keeps only the engine/driver — [ADR-0105](adrs/0105-adr-0105-typechecker-in-handl.md)
- **What stays in Rust**: the engine, VM primitives and the adapter that loads — [ADR-0105](adrs/0105-adr-0105-typechecker-in-handl.md)
- **What moves to handl**: the type model (judgment ops) plus the scope and — [ADR-0105](adrs/0105-adr-0105-typechecker-in-handl.md)
- **Bootstrap**: Rust embeds the .hnl model with include_str!, loads it into — [ADR-0105](adrs/0105-adr-0105-typechecker-in-handl.md)
- **The adapter is thin** — the model is the implementation, not a wrapper — [ADR-0105](adrs/0105-adr-0105-typechecker-in-handl.md)
- **Parity is proven against the Rust model** by the existing type-model — [ADR-0105](adrs/0105-adr-0105-typechecker-in-handl.md)

### ADR-0106: The Object-Arrow Kernel

- **One semantic world.** Values, objects, types and arrows share one value — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **sym() / sym(A) is the nominal identity primitive** — names are values, — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Arrows are the fundamental computational primitive.** There is no binding — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Products and composition are the structural core**; do is sugar over — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Dimensions <> specialize any value or arrow**, and generic objects use — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Graded arrows** carry effects in the grade, not in special forms — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Algebra, capabilities and ordering** use the same objects and arrows — no — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Proofs, refinements and dependent types are ordinary objects/arrows.** — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Polymorphism and Liskov are sym-based**; higher-order arrows and — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)
- **Expected-type inference and runtime/static convergence** follow from the — [ADR-0106](adrs/0106-adr-0106-the-object-arrow-kernel.md)

### ADR-0107: Handl SDK Generator

- **Port-adapter model**: a generated *port* is the abstract Handl interface; — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **New crates**: plugin-sdkgen (shared IR), a backend per source format, an — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **Multi-backend by three dimensions** — source format, target ABI, emitter — — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **The C backend uses libclang**, not a hand-rolled parser — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **File output mirrors the module tree**; generated packages are ordinary — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **Output form**: abstract export in the port, concrete — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **All FFI arrows carry the FFI grade**, so effects are visible in the — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **Mappings to Handl types/grades are data**, not backend code — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **Doc comments are emitted** from the source into the port — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)
- **Type/grade composition and the CLI surface** are part of the generator: — [ADR-0107](adrs/0107-adr-0107-handl-sdk-generator.md)

### ADR-0114: Handl Compile Plugin Architecture

- **A plugin-compile crate defines Compiler and CompilerInput** (a parsed, — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)
- **handl-emitters keeps its role** — emitting Language data fragments; it — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)
- **hnl compile <target> <file> dispatches by target name**, not file — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)
- **The JS target exposes** compile_to_string, emit_module and — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)
- **Value lowering** maps Handl primitives to JS, Arr/Obj to arrays/objects, — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)
- **Foreign calls lower to js.call(...)** and inline to the native call when — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)
- **v1 emits no source maps**; # sourceURL may be added in esm/inline — [ADR-0114](adrs/0114-adr-0114-handl-compile-plugin-architecture.md)

### ADR-0120: Paper/Implementation Alignment

- **Phase 1 — arrows as values**: a function used as a value lowers to — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Phase 1 — no self-application**: desugar.data's recursor becomes — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Phase 2 — composition axes**: Term::Seq gains an axis slot (default — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Phase 3 — dim unification**: ArrSource::Foreign(grade) merges into the — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Phase 3 — effects as cells**: op is a cell and handle is a cell — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Phase 4 — dependent structure**: Family<A>, Π, Σ enter via — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Phase 5 — laws as M[2] terms**: f ~[2]~> g parses to a Term and — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)
- **Ordering is load-bearing**: 1 → 2 → 3 → (4, 5 in parallel), with the — [ADR-0120](adrs/0120-adr-0120-paper-implementation-alignment.md)

### ADR-0123: AST shape faces — closing the last kind-dispatch sites

- id: ast.assign — [ADR-0123](adrs/0123-implementation-finding.md)
- id: ast.bind — [ADR-0123](adrs/0123-implementation-finding.md)
- id: ast.function — [ADR-0123](adrs/0123-implementation-finding.md)
- id: ast.call — [ADR-0123](adrs/0123-implementation-finding.md)
- id: ast.dim_arrow — [ADR-0123](adrs/0123-implementation-finding.md)

### ADR-0124: Handing a Language to capability plugins

- A host serializes its language **once** and reuses it (an Arc<Value> or — [ADR-0124](adrs/0124-adr-0124-handing-a-language-to-capability-plugins.md)
- A plugin that needs a language reads payload["language"], deserializes it, — [ADR-0124](adrs/0124-adr-0124-handing-a-language-to-capability-plugins.md)
- This path is **portable**: it works for in-process hosts, WASM plugins, and — [ADR-0124](adrs/0124-adr-0124-handing-a-language-to-capability-plugins.md)
- an Arc<Language> placed directly on the request (fast path, exact); — [ADR-0124](adrs/0124-adr-0124-handing-a-language-to-capability-plugins.md)
- the request's LanguageResolver, asked for payload["kit"] (or the — [ADR-0124](adrs/0124-adr-0124-handing-a-language-to-capability-plugins.md)
- otherwise PlatformError::InvalidMetadata naming *both* ways to supply a — [ADR-0124](adrs/0124-adr-0124-handing-a-language-to-capability-plugins.md)

### ADR-0126: Kernel boundary, and the idea ledger for retired reifications

- **The boundary rule.** Rust holds the **kernel** (the ADR-0040 primitives) — [ADR-0126](adrs/0126-adr-0126-kernel-boundary-and-the-idea-ledger-for-retired-reifications.md)
- **Unwired is a declared state, not an accident.** Every workspace member is — [ADR-0126](adrs/0126-adr-0126-kernel-boundary-and-the-idea-ledger-for-retired-reifications.md)
- **project/parked.yaml is the idea ledger.** Each retired entry carries the — [ADR-0126](adrs/0126-adr-0126-kernel-boundary-and-the-idea-ledger-for-retired-reifications.md)
- **Retirement protocol.** Classify (aspirational → proposed; replaced → — [ADR-0126](adrs/0126-adr-0126-kernel-boundary-and-the-idea-ledger-for-retired-reifications.md)
- **The gate is a test, not an audit.** crates/platform-tests/tests/wiring.rs — [ADR-0126](adrs/0126-adr-0126-kernel-boundary-and-the-idea-ledger-for-retired-reifications.md)

### ADR-0127: TransformRule::Replace — a rule form for root-eliminating rewrites

- path is interpreted exactly as RewriteStep::KeepPath's — a non-empty — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)
- A path that does not resolve to an existing descendant (out of range, or — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)
- The rule stays data-shaped: dispatch is still `(pass, node_kind, — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)
- It does **not** create, splice, or wrap anything. Replace returns a node — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)
- It does **not** change KeepPath's existing behaviour (a missed path in a — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)
- It is **not** a general subterm-selection language. There is one path, from — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)
- It does **not** make Native redundant. Native remains the escape hatch — [ADR-0127](adrs/0127-adr-0127-transformrule-replace-a-rule-form-for-root-eliminating-rewrites.md)

### ADR-0128: Target Repos — Repo = Target, Module = One Port Implementation

- **Repo = target/ecosystem; module = one implementation of one port.** A — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **Module directories carry the specific name** (modules/<port>-<specific>), — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **Compiler targets are one repo each** — dot/, term/, sexp/, handl/, — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **Platform keeps** ports, core crates, kit machinery, engines, wiring, — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **The per-crate extraction map** (26 rows of crate → repo/module) is the — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **The platform repo is renamed host/**; handl/ means the language target; — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **Hosts sit at the top of the fan**: host-cli and host-lsp declare — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **Git deps bypass registries**: consumers set — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)
- **The lang/ cycle is broken by injection**: consumers take &Language/a — [ADR-0128](adrs/0128-adr-0128-target-repos-repo-target-module-one-port-implementation.md)

### ADR-0129: Project-docs topology for the target-repo split

- **Every repo carries its own docs on a project orphan branch, mounted — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **ADRs live with the repo that owns the decision.** — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **Ports, kernel, composition, engines, tooling, runtime** are — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **Repo-local decisions** (e.g. 0113 WebIDL/JS, 0116 .hfc, 0109 — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **Cross-cutting ADRs are split.** The original number stays with the — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **project/adrs/INDEX.md is the global ADR registry** — — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **principles.md is hosted once, in the docs/ repo**, and linked from — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **History is preserved with git filter-repo --paths-from-file**, one — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **platform adopts the worktree model too**, and its project branch — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)
- **Until docs/ exists as a repo, platform is the hub.** Each repo's — [ADR-0129](adrs/0129-adr-0129-project-docs-topology-for-the-target-repo-split.md)

### ADR-0147: Lower the while state fold to Loop + explicit protocol ops

- **current** = st == null ? in : st, expressed as — [ADR-0147](adrs/0147-adr-0147-lower-the-while-state-fold-to-loop-explicit-protocol-ops.md)
- The step arrow applies to current destructured **by param name** — [ADR-0147](adrs/0147-adr-0147-lower-the-while-state-fold-to-loop-explicit-protocol-ops.md)
- A new loop_outcome op maps the step result: {state: v} → right(v) — [ADR-0147](adrs/0147-adr-0147-lower-the-while-state-fold-to-loop-explicit-protocol-ops.md)

### ADR-0148: Lower call/dimension application to App terms

- **var callee bound in the compile-time env** — substitute the bound — [ADR-0148](adrs/0148-adr-0148-lower-call-dimension-application-to-app-terms.md)
- **var callee bound as a parameter** — project it out of the input — [ADR-0148](adrs/0148-adr-0148-lower-call-dimension-application-to-app-terms.md)
- **var callee unknown to the env** — Op{name, args}: a native-op — [ADR-0148](adrs/0148-adr-0148-lower-call-dimension-application-to-app-terms.md)
- **call-face callee** (a nested call/dimension, per the ADR-0123 — [ADR-0148](adrs/0148-adr-0148-lower-call-dimension-application-to-app-terms.md)
- **any other callee** — sequential Apps, preserving curried — [ADR-0148](adrs/0148-adr-0148-lower-call-dimension-application-to-app-terms.md)

### ADR-0149: host-cli is a dumb host; ext-* extensions load dynamically

- **The host knows nothing about tools.** host-cli exposes exactly one — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- **Extensions are dynamic.** An extension is a shared library — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- **The ABI is JSON over a C boundary.** Three exported symbols per — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- handl_ext_manifest() -> char* — {"name","version","commands":[{name,usage}]} — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- handl_ext_dispatch(cmd: *const c_char, input: *const c_char) -> char* — — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- handl_ext_free(char*) — extension-owned strings freed by the — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- **One repo per extension** (ext-fmt, ext-lint, ext-typecheck, — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)
- **Discovery**: --ext-dir <dir> flag, else $HANDL_EXT_DIR, else — [ADR-0149](adrs/0149-adr-0149-host-cli-is-a-dumb-host-ext-extensions-load-dynamically.md)

### ADR-0150: Repo visibility is derived from the dep closure, not chosen per-repo

- **Edges point private → public.** An adapter or tool binding lives — [ADR-0150](adrs/0150-adr-0150-repo-visibility-is-derived-from-the-dep-closure-not-chosen-per-repo.md)
- **The closure is checked, not remembered.** — [ADR-0150](adrs/0150-adr-0150-repo-visibility-is-derived-from-the-dep-closure-not-chosen-per-repo.md)
- **handl-kit-prelude-lang** (public, new repo kit-prelude-lang) — — [ADR-0150](adrs/0150-adr-0150-repo-visibility-is-derived-from-the-dep-closure-not-chosen-per-repo.md)
- **handl-kit-data-prelude** (private) — the composition wrapper: — [ADR-0150](adrs/0150-adr-0150-repo-visibility-is-derived-from-the-dep-closure-not-chosen-per-repo.md)

### ADR-0151: Exec erases type annotations — signatures are typecheck-owned metadata

- **Signature exports erase to null.** is_signature_rhs(rhs) — [ADR-0151](adrs/0151-adr-0151-exec-erases-type-annotations-signatures-are-typecheck-owned-metadata.md)
- **The mirror stays in lockstep.** The transform is applied to every — [ADR-0151](adrs/0151-adr-0151-exec-erases-type-annotations-signatures-are-typecheck-owned-metadata.md)
- **eval.type-annot evaluates the subject only.** The canonical — [ADR-0151](adrs/0151-adr-0151-exec-erases-type-annotations-signatures-are-typecheck-owned-metadata.md)

### ADR-0153: ~[n]~> is a dimension index — n-cells as one indexed former

- The semiring-grade reading of ~[R]~> (ADR-0106 §10, — [ADR-0153](adrs/0153-adr-0153-n-is-a-dimension-index-n-cells-as-one-indexed-former.md)
- sort/grade/op declaration forms remain, but their graded-arrow — [ADR-0153](adrs/0153-adr-0153-n-is-a-dimension-index-n-cells-as-one-indexed-former.md)

### ADR-0154: Typecheck semantics — identity, equality, universes, literals, erasure

- values carry a type link → their type's sym; — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- types carry a parent link → their supertype sym; — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- x : T reads x.type; T <: U walks parent; instanceof = — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- Type is an indexed former: Type(i) : Type(i+1); bare Type = — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- Pi's level is max(domain, codomain) — the existing — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- Levels infer like other indices — invisible in ordinary code; only — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- Elaboration consumes all type-level content — metas solved, indices — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- **Invariant**: the final runnable artifact contains only 1-arrows + — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)
- Runtime-minted cells (eval/resolve constructing a law or type at — [ADR-0154](adrs/0154-adr-0154-typecheck-semantics-identity-equality-universes-literals-erasure.md)

### ADR-0155: Scope-Bound Arrow Environments

- **A dependency binding is a callable value** — one payload kind, **arrows**: — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)
- **Name resolution does not change per engine.** proj_env's scope-first — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)
- **ModuleEnv (ADR-0156) becomes the builder** of the lexical environment — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)
- **Bootstrap is unchanged**: HandlLoader::new builds the loader raw over — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)
- **Caching keys on (module_id, env_id)**: minted arrows capture the env, so — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)
- **Staged plan**: callv node + wrap helpers → engine switch → cache keys, — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)
- **Rejected: TaggedValue::NativeFn(Rc<NativeFnHandle>)** — a second callable — [ADR-0155](adrs/0155-adr-0155-scope-bound-arrow-environments.md)

### ADR-0156: Module environments are parameters, not construction state

- ModuleEnv owns `{ dependencies: Vec<Arc<dyn Module>>, — [ADR-0156](adrs/0156-adr-0156-module-environments-are-parameters-not-construction-state.md)
- instantiate produces a sibling module sharing the prepared source, — [ADR-0156](adrs/0156-adr-0156-module-environments-are-parameters-not-construction-state.md)
- with_dependency* / with_global_lookup / with_import_resolver — [ADR-0156](adrs/0156-adr-0156-module-environments-are-parameters-not-construction-state.md)
- mod.fn keeps dispatching through ModuleLookup — dep *functions* — [ADR-0156](adrs/0156-adr-0156-module-environments-are-parameters-not-construction-state.md)

### ADR-0159: Optimizer domains dissolve into ext tools, on a trigger

- **Domains stay labs for now.** opt-graph and opt-kernel keep their — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- **Dissolve when all three triggers hold:** — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- a **caller that is not the researcher** (CI job, product, another — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- the domain's public surface has been **unchanged for four weeks**; — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- the **referee is shared** (bench/benchkit usable by any domain), — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- **When triggered, wrap — do not rewrite.** The Python core is exposed — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- **Naming:** an opt-* family (opt-graph, opt-kernel). A third — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)
- **The generic core stays a library.** egraph, discovery, eqsat — [ADR-0159](adrs/0159-adr-0159-optimizer-domains-dissolve-into-ext-tools-on-a-trigger.md)

### ADR-0160: lib-* repos are language packages, not crates

- **Repo shape is fixed** (all 113): — [ADR-0160](adrs/0160-adr-0160-lib-repos-are-language-packages-not-crates.md)
- **License is MIT only**, one LICENSE file. LICENSE-APACHE/LICENSE-MIT — [ADR-0160](adrs/0160-adr-0160-lib-repos-are-language-packages-not-crates.md)
- **CI is bupha-only**: root ci.sh (the repo's gate) + a committed — [ADR-0160](adrs/0160-adr-0160-lib-repos-are-language-packages-not-crates.md)
- **The shell is a bug, not a feature.** It is removed in the same change that — [ADR-0160](adrs/0160-adr-0160-lib-repos-are-language-packages-not-crates.md)
- **Consumers move to packages**: tool-test (and any other Rust consumer) — [ADR-0160](adrs/0160-adr-0160-lib-repos-are-language-packages-not-crates.md)

### ADR-0161: An ADR is a one-page decision record

- An ADR is **at most ~8 KB and ten numbered rules**. Algorithms, tables, — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- ## Why is mandatory: the failure the decision prevents, in ≤8 lines — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- ## Not doing is mandatory: each rejected alternative on one line with its — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- ## Enforcement is mandatory: the file or test where the decision is true — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- The status vocabulary is closed and lives in the frontmatter (ADR-0163): — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- A dead ADR keeps its **number** as a tombstone row (successor + one-line — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- Authority is single. When a decision replaces another, the replacement — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- project/DECISIONS.md is generated from ## Decision sections — one line — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)
- scripts/check-adrs.py gates 1–7. Report-only for the legacy corpus; — [ADR-0161](adrs/0161-adr-0161-an-adr-is-a-one-page-decision-record.md)

### ADR-0162: project/ holds process artifacts; published docs live in the org docs/

- **project/ holds process artifacts only.** Explanations, tutorials, — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **The top-level set is closed**, identical in every repo: — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **One artifact answers one question.** A document that answers two is — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **Dated vs living.** notes/, reports/, retros/ are dated artifacts and — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **design/** is either named after the ADR it elaborates (NNNN-slug.md) — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **A rejected design produces an ADR, never a note.** notes/ is a — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **No copies across repos.** Cross-repo references are ids + URLs — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- **One project/ per repo, on its project branch** (ADR-0129 §1), mounted — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- scripts/check-project-layout.py enforces 1, 2, 7 and 8; — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)
- project/README.md states this map in one screen and links this ADR — [ADR-0162](adrs/0162-adr-0162-project-holds-process-artifacts-published-docs-live-in-the-org-docs.md)

### ADR-0163: Every project/ artifact declares its metadata in YAML frontmatter

- Every Markdown artifact under project/ opens with a YAML header. Three — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- type names the ADR-0162 taxonomy entry that owns the file — adr, — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- date is ISO-8601 (YYYY-MM-DD, or YYYY-MM when only the month is — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- ADRs carry status from the closed vocabulary `proposed | accepted | — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- A supersession is structured, never prose: status: superseded plus — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- **Status lives only in the header.** ADRs and RFCs use status (the enum in — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- refs is a list of ADR ids the document elaborates or relies on — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- The body never carries status. Template bullets (- Status:, - Date:, — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- title is the H1 without its identifier prefix (ADR-NNNN: , — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)
- scripts/project-frontmatter.py enforces the schema (--write migrates), — [ADR-0163](adrs/0163-adr-0163-every-project-artifact-declares-its-metadata-in-yaml-frontmatter.md)

### ADR-0165: The kernel IR is a Term plus kit-declared ops

- A kernel body is a **Term**: an op name applied to arguments, where — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)
- The **op set is closed at nine** — tile_load, unpack_i4, dequant, — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)
- Every op declares four things: a **signature**, a **grade** — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)
- Two terms are equal iff their **denotations** are equal, checked by — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)
- The IR carries **no kernel combinators**. It enters the platform as a — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)
- Cost is a **hard bound only** — roofline floor and legality pruning — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)
- The denotation is the *meaning*, not the *schedule*. Two equal terms — [ADR-0165](adrs/0165-adr-0165-the-kernel-ir-is-a-term-plus-kit-declared-ops.md)

### ADR-0166: The IR gains pipeline(depth), and the schedule programme stops there

- pipeline(depth) is the **tenth op**. It denotes the **identity** and — [ADR-0166](adrs/0166-adr-0166-the-ir-gains-pipeline-depth-and-the-schedule-programme-stops-there.md)
- It is priced at **zero** (no extra traffic, no launch). The cost model — [ADR-0166](adrs/0166-adr-0166-the-ir-gains-pipeline-depth-and-the-schedule-programme-stops-there.md)
- stage_shared is **not** added. num_stages already stages loads — [ADR-0166](adrs/0166-adr-0166-the-ir-gains-pipeline-depth-and-the-schedule-programme-stops-there.md)
- **The schedule-vocabulary programme is closed.** Measured: depth — [ADR-0166](adrs/0166-adr-0166-the-ir-gains-pipeline-depth-and-the-schedule-programme-stops-there.md)
- The 47 % → 86 % gap to Marlin is therefore **not load latency**. It is — [ADR-0166](adrs/0166-adr-0166-the-ir-gains-pipeline-depth-and-the-schedule-programme-stops-there.md)

### ADR-NNNN: <the decision, stated as a fact — "X is Y", not "About X">

- Up to ten numbered, normative, testable statements — [ADR-0170](adrs/0170-adr-nnnn-the-decision-stated-as-a-fact-x-is-y-not-about-x.md)
- State what *is*, not what was tried, not what will be done next — [ADR-0170](adrs/0170-adr-nnnn-the-decision-stated-as-a-fact-x-is-y-not-about-x.md)
- No code listings, no crate trees, no phase tables — those go to — [ADR-0170](adrs/0170-adr-nnnn-the-decision-stated-as-a-fact-x-is-y-not-about-x.md)

### ADR-0171: The kernel IR gains layout and pointwise ops

- **Eleven ops join the set** (ten → twenty-one): the layout ops — [ADR-0171](adrs/0171-adr-0171-the-kernel-ir-gains-layout-and-pointwise-ops-the-mlopt-bridge-s-blocking-gap.md)
- transpose/permute are **views**: they denote torch relayouts — [ADR-0171](adrs/0171-adr-0171-the-kernel-ir-gains-layout-and-pointwise-ops-the-mlopt-bridge-s-blocking-gap.md)
- Pointwise costs are honest elementwise prices: binary ops read — [ADR-0171](adrs/0171-adr-0171-the-kernel-ir-gains-layout-and-pointwise-ops-the-mlopt-bridge-s-blocking-gap.md)
- This extension exists **because the bridge measured the gap**: — [ADR-0171](adrs/0171-adr-0171-the-kernel-ir-gains-layout-and-pointwise-ops-the-mlopt-bridge-s-blocking-gap.md)
- permute takes its dims as **positional int args** (the wire — [ADR-0171](adrs/0171-adr-0171-the-kernel-ir-gains-layout-and-pointwise-ops-the-mlopt-bridge-s-blocking-gap.md)

### ADR-0172: Tensor programs are Handl Terms

- **Tensor programs ARE Handl Terms — full convergence is the — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **Phase 0 — the Term kernel extension (own ADR).** The spike — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **DAG sharing**: Terms are trees; neural nets are hash-consed — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **Typed attrs slot**: Term::Op has no attrs field; the spike — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **Non-finite literals**: serde_json rejects NaN/±inf; needs — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **.hnl / Term serialization is the canonical wire format — — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **mlopt_core.ir remains the search IR until Phase 0 lands.** — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)
- **Then the migration**: tensor Language fragment → Term-native — [ADR-0172](adrs/0172-adr-0172-tensor-programs-are-handl-terms-the-e-graph-is-a-search-interpretation-not-a-second-ir.md)

### ADR-0173: Term attrs are op metadata, program sharing is derived-form data, and non-finite floats are a codec tag

- **Term::Op gains attrs: Option<Value>** — an optional, — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **Term stays a tree. DAG sharing is expressed at three existing — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **Weights/parameters → the environment.** Shared leaves are — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **Semantic sharing → engines.** Hash-consing/e-classing is a — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **Wire compactness + direct DAG interpretation → a derived — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **No Ref{name}/let variant in the closed Term enum.** A — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **Non-finite floats are a codec-profile convention:** the — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)
- **The Phase-0 kernel diff is one optional field.** Everything else — [ADR-0173](adrs/0173-adr-0173-term-attrs-are-op-metadata-program-sharing-is-derived-form-data-and-non-finite-floats-are-a-codec-tag.md)

### ADR-0174: The end-state architecture

- **Verify before ship.** The *operative verify verdict* — — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- **Declines are named.** Unsupported op, OOM, unverifiable, — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- **Measured or it didn't happen.** Speed claims come from the — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- **One codec.** .hnl/Term serialization is the only — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- **Substrate layering.** The *gamekit kernel modules* — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- **Ops are data.** Tensor ops live in a Language fragment; — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- No claim that Sanada completeness yields e-graphs — search — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- No torch-free claim until kernel-ir coverage is measured, not — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- No Ref/let/vars in the pointless core — sharing is engine — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)
- No parallel IR proliferation — mlopt_core.ir is the *temporary* — [ADR-0174](adrs/0174-adr-0174-the-end-state-architecture-one-term-many-interpretations-verified-delivery.md)

### ADR-0176: The language end-state

- **Rust adapters for declarative formats.** ONNX (protobuf — — [ADR-0176](adrs/0176-adr-0176-the-language-end-state-python-only-at-ecosystem-edges-rust-and-data-in-the-middle.md)
- **Architectures-as-data.** A model family becomes a fragment — [ADR-0176](adrs/0176-adr-0176-the-language-end-state-python-only-at-ecosystem-edges-rust-and-data-in-the-middle.md)
- **sdkgen for residual FFI.** Where a backend exposes only a — [ADR-0176](adrs/0176-adr-0176-the-language-end-state-python-only-at-ecosystem-edges-rust-and-data-in-the-middle.md)
- **HF architecture discovery** for *new* model families — — [ADR-0176](adrs/0176-adr-0176-the-language-end-state-python-only-at-ecosystem-edges-rust-and-data-in-the-middle.md)
- **Meta-tier drivers** (gamekit/discoverykit/benchkit — [ADR-0176](adrs/0176-adr-0176-the-language-end-state-python-only-at-ecosystem-edges-rust-and-data-in-the-middle.md)

### ADR-0177: engine-eqsat search contracts

- **Law format: JSON records** mirroring laws.serialize — [ADR-0177](adrs/0177-adr-0177-engine-eqsat-search-contracts-laws-guards-and-costs-as-kit-data.md)
- **Guards: port the cond/dspec DSL to Rust** as a — [ADR-0177](adrs/0177-adr-0177-engine-eqsat-search-contracts-laws-guards-and-costs-as-kit-data.md)
- **Costs: a serializable CostSpec** — named kinds — [ADR-0177](adrs/0177-adr-0177-engine-eqsat-search-contracts-laws-guards-and-costs-as-kit-data.md)
- **Certificates: no proof.py port in P2.** ProofEdge/ — [ADR-0177](adrs/0177-adr-0177-engine-eqsat-search-contracts-laws-guards-and-costs-as-kit-data.md)
- **Engine surface: a SearchEngine trait minted inside — [ADR-0177](adrs/0177-adr-0177-engine-eqsat-search-contracts-laws-guards-and-costs-as-kit-data.md)

### ADR-0178: P3 codec consolidation and mlopt_core.ir demotion

- **Two canonical artifacts, distinct authority:** the tensor — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **Direction flips:** vocab/ stops being generated *from* Python — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **One term codec:** hnl_codec's kernel-Term envelope — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **Unguarded-table closure:** mlopt_onnx opmap/liftmap get a — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **D1 parity corpus** — differential extracted-term comparison vs — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **D2 opt-in** — Term bridge callable from Python (PyO3 preferred; — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **D3 feature catch-up** — stage-2 guarded-law batches, — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **D4 measured default flip** — only on differential parity with no — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **D5 adapter Term-native output** — Sources/Sinks produce/consume — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)
- **D6 retirements** — mlopt_native (superseded by engine-eqsat), — [ADR-0178](adrs/0178-adr-0178-p3-codec-consolidation-and-mlopt-core-ir-demotion-sequencing.md)

### Options Object Over Named Arguments

- The options argument is always the **last positional argument** — [ADR-0179](adrs/0179-options-object-over-named-arguments.md)
- Missing keys are treated as absent (not errors) — [ADR-0179](adrs/0179-options-object-over-named-arguments.md)
- null or an empty object {} is equivalent to omitting the argument — [ADR-0179](adrs/0179-options-object-over-named-arguments.md)
- Verb shorthands (http.get, http.post, etc.) remain positional for — [ADR-0179](adrs/0179-options-object-over-named-arguments.md)

### Expression Object and Array Literals

- **Array literal:** [expr, expr, ...] — [ADR-0180](adrs/0180-expression-object-and-array-literals.md)
- **Object literal:** { key: expr, key: expr, ... }, where a key is a bare identifier or a quoted string and a value is… — [ADR-0180](adrs/0180-expression-object-and-array-literals.md)
- Treat { and } as nesting tokens alongside ( and [ in argument splitting and depth tracking (split_args,… — [ADR-0180](adrs/0180-expression-object-and-array-literals.md)
- Add parse_array_literal and parse_object_literal to the ExprParser trait. Implementations will parse the content… — [ADR-0180](adrs/0180-expression-object-and-array-literals.md)
- Use the existing Expr::Value variant for object and array results; no new AST node is required because V is already… — [ADR-0180](adrs/0180-expression-object-and-array-literals.md)
- Update the runtime eval_arg and the typechecker parse_expr to recognize [] and {} as value expressions, evaluate/parse… — [ADR-0180](adrs/0180-expression-object-and-array-literals.md)

### handl-native s.* Schema Vocabulary

- name: a — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- name: b — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- Strings: s.min_length(n), s.max_length(n), s.length(n), s.regex(pattern) — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- Numbers/Integers: s.min_value(n), s.max_value(n), s.range(min, max), s.multiple_of(n) — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- Arrays: s.min_items(n), s.max_items(n), s.length(n) — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- Generic: s.brand(name) (for branded types; typegen can preserve the name as metadata) — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- **handl-dialect-canonical** provides SSchemaVocabulary and SSchema as the canonical implementations of… — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- **handl-typecheck-schema-s** (new crate) provides SSchemaInterpreter, implementing SchemaInterpreter by mapping s.*… — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- **handl-typegen-backend-s** (new crate) provides an SBackend that emits the canonical s schema as a JSON-serializable… — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- **handl-typegen-backend-json-schema** is rewritten as a translator from s to JSON Schema. It becomes an optional… — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- **handl-fields** removes the JSON-Schema-specific check_arg_type in computed.rs and instead compiles arg schemas… — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)
- **handl-compiler** default configs use SSchemaInterpreter and SBackend instead of the JSON Schema implementations — [ADR-0181](adrs/0181-handl-native-s-schema-vocabulary.md)

### JSON Schema 2020-12 Reference & Composition Support in the Typechecker

- Per-field $defs: pass the field's own schema as root — [ADR-0182](adrs/0182-json-schema-2020-12-reference-composition-support-in-the-typechecker.md)
- Document-level $defs: the caller builds a synthetic root object (e.g., a — [ADR-0182](adrs/0182-json-schema-2020-12-reference-composition-support-in-the-typechecker.md)

### ADR-0183: Type Aliases in $defs

- **Type aliases live in $defs inside $:**, with an optional typeParams — [ADR-0183](adrs/0183-adr-0183-type-aliases-in-defs.md)
- **Aliases are type-only declarations**: they are not data fields, are not — [ADR-0183](adrs/0183-adr-0183-type-aliases-in-defs.md)
- **References use standard $ref plus a handl args list**; args — [ADR-0183](adrs/0183-adr-0183-type-aliases-in-defs.md)
- **No new Type variant**: the resolved $ref produces a Type (possibly — [ADR-0183](adrs/0183-adr-0183-type-aliases-in-defs.md)

### ADR-0184: Annotations

- **An optional annotations object is allowed on any document**; the VM — [ADR-0184](adrs/0184-adr-0184-annotations.md)
- **Conventional keys** are title, description, author, tags, since, — [ADR-0184](adrs/0184-adr-0184-annotations.md)
- **Annotations cost nothing at runtime or typecheck**; the linter may validate — [ADR-0184](adrs/0184-adr-0184-annotations.md)
- **Hosts may define additional conventional keys**, namespaced to avoid — [ADR-0184](adrs/0184-adr-0184-annotations.md)

### ADR-0185: Custom Fields

- **User-defined fields are declared in *.field.yml files**; the core keeps — [ADR-0185](adrs/0185-adr-0185-custom-fields.md)
- **Core keys are** type, expr, arg, schema, inheritable, mutable — [ADR-0185](adrs/0185-adr-0185-custom-fields.md)
- **Field types are sugar and handler**: sugar expands to a core-language — [ADR-0185](adrs/0185-adr-0185-custom-fields.md)
- **A field file declares its schema and expansion**, and the resolver loads — [ADR-0185](adrs/0185-adr-0185-custom-fields.md)

### ADR-0186: The dependencies Field

- **A built-in dependencies field lists the custom field names a document — [ADR-0186](adrs/0186-adr-0186-the-dependencies-field.md)
- **It is allowed** at the document top level, on a field declaration, in a — [ADR-0186](adrs/0186-adr-0186-the-dependencies-field.md)
- **Names are dotted**: bare (delay) for project-local fields, namespaced — [ADR-0186](adrs/0186-adr-0186-the-dependencies-field.md)
- **Resolution collects** dependencies from the document, field declarations — [ADR-0186](adrs/0186-adr-0186-the-dependencies-field.md)
- **Tooling**: handl info shows the list; handl lint may warn when a custom — [ADR-0186](adrs/0186-adr-0186-the-dependencies-field.md)

### Parameter Naming Convention

- Parameter names use snake_case — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- Names are nouns, not verbs (prefix not do_prefix) — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- Boolean parameters use is_, has_, or should_ prefixes — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- No abbreviations (config not cfg, instance not inst) — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- self — reserved for future method-like dispatch — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- host — reserved for host reflection — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- args — reserved for variadic argument access — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- name: prefix — [ADR-0187](adrs/0187-parameter-naming-convention.md)
- name: length — [ADR-0187](adrs/0187-parameter-naming-convention.md)

### ADR-0188: Modality as Projection

- **A modality system built on one primitive**: project(x, K) extracts the — [ADR-0188](adrs/0188-adr-0188-modality-as-projection.md)
- **Six moods are uses of that projection**: indicative (strict), potential — [ADR-0188](adrs/0188-adr-0188-modality-as-projection.md)
- **x not K is complement projection, not Boolean negation**; Boolean — [ADR-0188](adrs/0188-adr-0188-modality-as-projection.md)
- **Each mood is a module** (mood.ind, mood.pot, mood.neg, mood.int, — [ADR-0188](adrs/0188-adr-0188-modality-as-projection.md)
- **Strict moods error on non-membership**; soft moods return null, the — [ADR-0188](adrs/0188-adr-0188-modality-as-projection.md)
- **Projection composes with grades and sorts** — a mood is not a separate — [ADR-0188](adrs/0188-adr-0188-modality-as-projection.md)

### ADR-0189 (Remaining): Types are Values — Advanced Type System Roadmap

- **data declarations for inductive types** — Define algebraic datatypes with sum-of-products structure, generating… — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- **rec / termination checking** — Ensure recursive type-level definitions are total and terminating via structural… — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- **Inductive families** — Enable types parameterized by values, like Vec(A, n) where n is a length index — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- **Dependent pattern matching with a motive** — Pattern match on indexed types, refining the goal type based on… — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- **Type inference and unification** — Infer types in expressions with _ holes; solve constraints via unification — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- **Proof erasure at runtime** — ~~Remove proof terms before evaluation to reduce runtime overhead while preserving type… — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- _Completed._ Built-in equality/membership proofs (type.refl, type.is_evidence) and the uninhabited type.empty are… — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)
- **Tactics** — Provide tactic-assisted proof construction (auto, induction, rewrite, simp) for interactive type-level… — [ADR-0189](adrs/0189-adr-0189-remaining-types-are-values-advanced-type-system-roadmap.md)

### ADR-0190: vau as a Typed Escape

- **Add Scheme::Vau { params, body }**: children are bound as unevaluated — [ADR-0190](adrs/0190-adr-0190-vau-as-a-typed-escape.md)
- **The body may** eval a bound expression, inspect the AST, or transform it — [ADR-0190](adrs/0190-adr-0190-vau-as-a-typed-escape.md)
- **Add a vau keyword and grammar rule** in canonical language data; the — [ADR-0190](adrs/0190-adr-0190-vau-as-a-typed-escape.md)
- **vau is the only escape** from structured compositional evaluation; every — [ADR-0190](adrs/0190-adr-0190-vau-as-a-typed-escape.md)
- **Capability discipline covers it**: a language with vau advertises — [ADR-0190](adrs/0190-adr-0190-vau-as-a-typed-escape.md)

### ADR-0191: Computed Object Keys via Bracket Syntax

- An array literal is never followed by : in expression position (the — [ADR-0191](adrs/0191-adr-0191-computed-object-keys-via-bracket-syntax.md)
- The entry node's key slot is the only place this reinterpretation — [ADR-0191](adrs/0191-adr-0191-computed-object-keys-via-bracket-syntax.md)

### ADR-0192: Predicates as Types — Unified Dependent Typing Without a Sublanguage

- To verify x: T = e, normalize T(e) — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)
- If normalization succeeds and yields true, accept statically — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)
- If normalization yields false, reject — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)
- If normalization is blocked (effect, non-termination, opaque value), — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)
- It cannot infer A and B from an opaque function body soundly — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)
- It reintroduces a type-level operation as a special form, breaking the — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)
- It does not resolve the type-of-type regress (it just renames it) — [ADR-0192](adrs/0192-adr-0192-predicates-as-types-unified-dependent-typing-without-a-sublanguage.md)

### ADR-0193: Failure-Tracking Arrows for Backtracking Parsers

- inl branch: wp assumes the success invariant and propagates the — [ADR-0193](adrs/0193-adr-0193-failure-tracking-arrows-for-backtracking-parsers.md)
- inr branch: wp assumes the error invariant and propagates into — [ADR-0193](adrs/0193-adr-0193-failure-tracking-arrows-for-backtracking-parsers.md)

### ADR-0194: State Machine Invariants via Fixpoint Discovery

- Each transition atom has a **conditional contract**: — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- state.dec : Int → Int — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- requires: Le(0, input) — can only decrement if ≥ 0 — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- guarantee: Le(0, output) — result is ≥ 0 — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- The state machine is a loop: — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- The inference engine runs the fixpoint: — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- I₀ = body's guarantee (includes Le(0, output) from state.dec) — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- wp(body, I₀) = the precondition needed to preserve I₀ — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- For state.dec: wp = Le(0, input) (the precondition) — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- I₁ = I₀ ∧ wp(body, I₀) = Le(0, output) ∧ Le(0, input) — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- Fixpoint: the invariant Le(0, count) (Count ≥ 0) is discovered — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- The guarantee includes the invariant: Le(0, output) — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- **Data invariants**: properties of the state value itself (Count ≥ 0, — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- **Conditional transitions**: atoms with preconditions (can only — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- **Termination**: if the event stream is finite (consumed via the side — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- **Concurrency invariants**: "the buffer is never accessed by two — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- **Liveness properties**: "the counter eventually reaches zero" — this — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)
- **Global invariants**: properties spanning multiple state machines — — [ADR-0194](adrs/0194-adr-0194-state-machine-invariants-via-fixpoint-discovery.md)

### ADR-0195: Arrow Surface Syntax v0.1 — Redesign

- **Correction 1 — Destructuring renaming.** A pattern element may use source: local to bind a source field/position to a… — [ADR-0195](adrs/0195-adr-0195-arrow-surface-syntax-v0-1-redesign.md)
- **Correction 2 — rec as a recursive region (Paterson-style).** rec **does not define a recursive arrow**. It introduces… — [ADR-0195](adrs/0195-adr-0195-arrow-surface-syntax-v0-1-redesign.md)
- **Addendum — Control-flow and construction sugar.** The v0.1 draft specified if, match, and rec as the only… — [ADR-0195](adrs/0195-adr-0195-arrow-surface-syntax-v0-1-redesign.md)
- **What is accepted unchanged from the draft.** The following from the v0.1 draft are accepted as-is: — [ADR-0195](adrs/0195-adr-0195-arrow-surface-syntax-v0-1-redesign.md)

### ADR-0196: Traits as Arrow Signatures — Graph-Based Interfaces and Contracts

- **Primary mode: trait inference (default).** The user writes a graph. The system derives: — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Secondary mode: explicit declarations (optional).** When the user wants to **name** a law pattern — for… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **The open trait catalog.** Trait inference does not magically derive unknown laws from nothing. It indexed catalog.… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Catalog storage: abstract interface, SQLite implementation.** The trait catalog is persisted in a storage backend.… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Type parameter syntax: angle brackets, not square brackets.** Type application uses **angle brackets** F<A>, not… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Impl declaration (graph layer — annotation-free).** An impl is a graph assignment for each operation in the trait's… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Generic code with trait bounds.** A function may declare trait bounds on its type parameters. The bound is a… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Supertraits as signature inclusion.** A supertrait bound trait T : U means Σ_T requires Σ_U to be satisfied first.… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Associated types via dependent types.** A trait may declare an associated type using the dependent-type machinery… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)
- **Blanket impls via ArrowApply; Law verification: the CutElim connection; Trait resolution: catalog search (fallback);… — [ADR-0196](adrs/0196-adr-0196-traits-as-arrow-signatures-graph-based-interfaces-and-contracts.md)

### ADR-0197: Decoupled Trait Layer — Laws, Bundles, and Handlers

- **Law on an arrow.** A law is a standalone declaration that an arrow satisfies an equation. It attaches to any arrow —… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **Bundle.** A **bundle** is a named record of arrows plus laws over them. It is only needed when a law spans multiple… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **Handler.** A **handler** injects an arrow or bundle at a scope boundary. This is dependency injection over effect… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **How they compose.** The three features are independent. No combination is mandatory. The "trait" pattern emerges when… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **= in laws.** Inside a law context, = means extensional equality of arrows (both sides produce the same output for all… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **Resolution: handlers primary, catalog fallback.** 1. **Primary: explicit handle ... with** — names the impl,… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **What this replaces from ADR-0196.** with laws attached). No keyword. installation). No keyword. attachable to any… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)
- **What this preserves from ADR-0196.** set. Unchanged. default). handle site. Unchanged in mechanism. normalization,… — [ADR-0197](adrs/0197-adr-0197-decoupled-trait-layer-laws-bundles-and-handlers.md)

### ADR-0198: Value Types as Data — Composable Capabilities and Target Representations

- **Three new data fragment types.** Extend the Language data schema (ADR-0044) with three new fragment sets, each loaded… — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Value Types (value_types.yaml).** A value type declares: — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Capabilities (capabilities.yaml).** A capability declares: — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Target Representations (representations.yaml).** A representation declares how a value type is concretely represented… — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **How the compiler uses this.** The compilation pipeline becomes: — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Type inference: gradual + runtime profiling, gated by language policy.** To specialize, the compiler needs to know… — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Monomorphization.** Yes — the compiler monomorphizes generic operations. When sum<T: Addable + Zero> is called with T… — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Relationship to TaggedValue (the handl-value crate).** TaggedValue is the **dynamic fallback representation** — the… — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)
- **Relationship to ADR-0197 (Decoupled Trait Layer).** ADR-0197 decoupled traits into laws + bundles + handlers. This… — [ADR-0198](adrs/0198-adr-0198-value-types-as-data-composable-capabilities-and-target-representations.md)

### ADR-0199: Unify Arrow Dialects — Graph Blocks, No let, No in

- **Graph blocks replace let..in and x = y in body:** — [ADR-0199](adrs/0199-adr-0199-unify-arrow-dialects-graph-blocks-no-let-no-in.md)
- **Contextual {} parsing (ADR-0195 ID.11):** { after ~>, if cond, else, match val parses as a _block_… — [ADR-0199](adrs/0199-adr-0199-unify-arrow-dialects-graph-blocks-no-let-no-in.md)
- **eval_block binds assigns:** The tree-walk evaluator's eval_block pushes a scope, binds each assign child (x = y) as a… — [ADR-0199](adrs/0199-adr-0199-unify-arrow-dialects-graph-blocks-no-let-no-in.md)
- **Unified grammar (overlay approach):** The arrow DSL language is built on top of the canonical language: arrow_lang.rs… — [ADR-0199](adrs/0199-adr-0199-unify-arrow-dialects-graph-blocks-no-let-no-in.md)
- **>>> semantics:** Canonical uses App (host-call) for >>>; the arrow overlay replaces it with Seq (arrow composition)… — [ADR-0199](adrs/0199-adr-0199-unify-arrow-dialects-graph-blocks-no-let-no-in.md)
- **Remove let and in:** Delete grammar.let.prefix and grammar.arrow.in.infix after all files are converted — [ADR-0199](adrs/0199-adr-0199-unify-arrow-dialects-graph-blocks-no-let-no-in.md)

### ADR-0200: Inductive Types as Desugared Predicates

- **A predicate** (the type) that recognizes values built by the constructors — [ADR-0200](adrs/0200-adr-0200-inductive-types-as-desugared-predicates.md)
- **Constructors** as ordinary values (tagged records or functions) — [ADR-0200](adrs/0200-adr-0200-inductive-types-as-desugared-predicates.md)
- **A recursor** as an ordinary library function the checker recognizes for — [ADR-0200](adrs/0200-adr-0200-inductive-types-as-desugared-predicates.md)

### ADR-0201: Arrow Macros — Type-Safe Compile-Time Metaprogramming

- **Macros are arrows that transform arrows.** A macro is an arrow Ast ~> Ast (or more generally Scheme A B ~> Scheme A'… — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **The macro declaration form.** A new top-level declaration, alongside op/handl/cap/const: — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Hygiene.** Macro hygiene prevents variable capture: a macro that introduces a variable x must not capture a caller's… — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Type safety.** Macros are typechecked in two phases: — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Compile-time execution.** Macros run at compile time, not runtime. The execution model: — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Quotation and splicing.** Two syntax forms for building AST in macros: — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Pattern matching on AST.** Macros pattern match on AST structure. With ADR-0081 (child roles) and ADR-0200 (inductive… — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **What macros cannot do.** runtime with no fs/net/libc capabilities. A macro cannot read a file, make a network call,… — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Relationship to const (ADR-0083 §5.1).** const is the simplest macro — a nullary macro that produces a literal value: — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)
- **Relationship to data (ADR-0200).** data declarations (inductive types) are desugared to predicates, constructors, and… — [ADR-0201](adrs/0201-adr-0201-arrow-macros-type-safe-compile-time-metaprogramming.md)

### ADR-0202: The Frontier Calculus

- **Term gains the frontier constructors** — CellId, CellCompV, — [ADR-0202](adrs/0202-adr-0202-the-frontier-calculus.md)
- **Surface syntax exposes them as data** (the Option B grammar), not as — [ADR-0202](adrs/0202-adr-0202-the-frontier-calculus.md)
- **Engine semantics**: cells compose vertically and horizontally, whiskers — [ADR-0202](adrs/0202-adr-0202-the-frontier-calculus.md)
- **Serialization is the same as every Term** — the constructors are JSON — [ADR-0202](adrs/0202-adr-0202-the-frontier-calculus.md)
- **Relation to existing ADRs**: this is the concrete term layer for ADR-0100 — [ADR-0202](adrs/0202-adr-0202-the-frontier-calculus.md)
- **Frontier support is optional**: a model declares whether it interprets — [ADR-0202](adrs/0202-adr-0202-the-frontier-calculus.md)

### ADR-0203: Session Types as Grades

- **Introduce grade Session** with !T (send), ?T (receive) and End; — [ADR-0203](adrs/0203-adr-0203-session-types-as-grades.md)
- **Duality dual : Session -> Session** flips every action (!T ↔ ?T, — [ADR-0203](adrs/0203-adr-0203-session-types-as-grades.md)
- **Sequential composition is grade multiplication** — !Int ; ?Bool ; End — [ADR-0203](adrs/0203-adr-0203-session-types-as-grades.md)
- **Channel operations are graded arrows** whose grade is a Session object — [ADR-0203](adrs/0203-adr-0203-session-types-as-grades.md)
- **Sessions are checked like any grade** — no separate session type system — [ADR-0203](adrs/0203-adr-0203-session-types-as-grades.md)

### ADR-0204: Type-Level Total Evaluation of handl Expressions

- Substitute the concrete call-site type and value arguments into the function's declared return type — [ADR-0204](adrs/0204-adr-0204-type-level-total-evaluation-of-handl-expressions.md)
- Reduce the resulting handl expressions by evaluating them in a **total** interpreter: no effects, no while/do-while… — [ADR-0204](adrs/0204-adr-0204-type-level-total-evaluation-of-handl-expressions.md)
- Compare the reduced indices during unification. If the reduced values differ and neither side is a metavariable, report… — [ADR-0204](adrs/0204-adr-0204-type-level-total-evaluation-of-handl-expressions.md)
- Leave expressions that cannot be reduced (because they contain unresolved variables or metavariables) in normal form… — [ADR-0204](adrs/0204-adr-0204-type-level-total-evaluation-of-handl-expressions.md)

### ADR-0206: Language data assembly in the lang repo

- crates/handl-kit-data-canonical — the fragment merger and the canonical — [ADR-0206](adrs/0206-adr-0206-language-data-assembly-in-the-lang-repo.md)
- the .hnl packages it pins (query, command, event, …) — [ADR-0206](adrs/0206-adr-0206-language-data-assembly-in-the-lang-repo.md)

### ADR: Generic Arrow Calculus for Any Dimension

- **Arrows are primitive; functions emerge.** There is no lambda calculus, no λ, no primitive functions. The only… — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Two fundamental operators: () and <>.** The surface has exactly two application-like operators: — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Dimensions contain any expression.** A dimension is not restricted to types. It can contain any expression: — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Types are expressions; type constructors are arrows.** Types are ordinary expressions, and type constructors are… — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Generic arrows and objects are arrow families.** Generic arrows are arrow families — arrows over dimensions: — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Dependent types: dependency is not restricted to types.** Expressions can depend on other expressions: — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Proofs are ordinary typed expressions (Curry–Howard).** P : Type p : P — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Graded arrows.** Grades annotate arrows: — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Grade algebras are ordinary structures.** Grade systems are ordinary structures rather than hard-coded language… — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
- **Arrow combinators are the computational vocabulary; Quantification is consistent with dimension binding.** See the… — [ADR-0207](adrs/0207-adr-generic-arrow-calculus-for-any-dimension.md)
