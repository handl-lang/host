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

### Library crates are exposed as Platform plugins

- The crate name is prefixed with plugin-lib- (e.g. plugin-lib-json, plugin-lib-crypto) to make the plugin role explicit… — [ADR-0001](adrs/0001-library-crates-are-platform-plugins.md)
- Each crate contains a plugin module that exposes a minimal Plugin implementation — [ADR-0001](adrs/0001-library-crates-are-platform-plugins.md)
- The plugin's id is lib.<name> (e.g. lib.json), and its contribute method registers the concrete library… — [ADR-0001](adrs/0001-library-crates-are-platform-plugins.md)
- The plugin does not declare capabilities of its own unless the library is directly invokable; its purpose is to add the… — [ADR-0001](adrs/0001-library-crates-are-platform-plugins.md)
- plugin-lib-type keeps its Plugin module behind a plugin feature so that consumers that only need the type definitions… — [ADR-0001](adrs/0001-library-crates-are-platform-plugins.md)

### ADR-0002 (Remaining): Plugin Library Conversion and Module Abstraction

- _*Should plugin-lib-* crates use Behavior<Op> or is Module/Function the appropriate abstraction for host-exported… — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Current state: ~20+ plugin-lib-* crates use Module trait and expose Function values for handl code to call — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Option A: Convert to Behavior<Op> (one operation per library capability) — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Option B: Keep Module/Function as the abstraction for "host functions that handl code calls" (distinct from rule… — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Option C: Hybrid — use Module as an adapter that contributes Behavior<Op> to the appropriate operations — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- **How should library dependencies be expressed?** — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Can a plugin-lib-* declare that it requires or provides specific module capabilities? — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Should this be done via Plugin::depends_on() trait or via a separate dependency manifest? — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- **What about library options?** — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- How do plugin-lib-* register options (e.g., compression level, timeout) if not via the legacy Library::options()? — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)
- Should the OptionsPlugin be extended to support library-scoped options? — [ADR-0002](adrs/0002-handl-platform-composable-host-remaining.md)

### Platform is agnostic of language-specific plugins

- The plugin-lib-* crates that define the Plugin contract and any shared, language-neutral primitives — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- Utility crates for serialization, collections, etc. that have no language-specific knowledge — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-ast-* (AST node plugins) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-parser (parser) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-grammar-* (grammar rules) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-typecheck-core (typechecking) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-runtime-* (runtime rules) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-vm-* and plugin-vm-values-* (VM and value representations) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-eval-* (eval rules) — [ADR-0002](adrs/0002-platform-is-agnostic.md)
- plugin-fmt-*, plugin-lint-*, plugin-diag-* (formatting, linting, diagnostics) — [ADR-0002](adrs/0002-platform-is-agnostic.md)

### Strategy C — Runtime Module Loading as Handler-Interpreted Ops

- **Strategy C is not a separate strategy.** module_ref and project are — [ADR-0009](adrs/0009-strategy-c-runtime-module-loading.md)
- **The handler's operation table is the opt-in.** A handler without the — [ADR-0009](adrs/0009-strategy-c-runtime-module-loading.md)
- **Governance is handler configuration**: path allowlist, load-depth limit, — [ADR-0009](adrs/0009-strategy-c-runtime-module-loading.md)
- **Two static passes work without executing**: cycle detection over the — [ADR-0009](adrs/0009-strategy-c-runtime-module-loading.md)
- **Graduate to real Scheme variants only** when a second handler needs — [ADR-0009](adrs/0009-strategy-c-runtime-module-loading.md)

### Package Manager: Content-Addressed, Diff-Based, Composable Stores

- Re-installing the same hash is a no-op — [ADR-0010](adrs/0010-package-manager.md)
- Updating v1.2.0 → v1.3.0 fetches only the diff between the two graph — [ADR-0010](adrs/0010-package-manager.md)
- Docs, source maps, and derived artifacts are content-addressed blobs — [ADR-0010](adrs/0010-package-manager.md)

### handl Formatter

- **Two-space indentation** (YAML, .handl). JSON and TOML follow their — [ADR-0014](adrs/0014-formatter.md)
- **A single trailing newline** at the end of each file — [ADR-0014](adrs/0014-formatter.md)
- **No trailing whitespace** on any line — [ADR-0014](adrs/0014-formatter.md)
- **Canonical key order for mappings.** Unknown keys are kept after known — [ADR-0014](adrs/0014-formatter.md)
- Document top-level: annotations, version, import, fields, — [ADR-0014](adrs/0014-formatter.md)
- Field mapping: type, expr, arg, schema, inheritable, — [ADR-0014](adrs/0014-formatter.md)
- **Array order is preserved.** Authors may group items intentionally — [ADR-0014](adrs/0014-formatter.md)
- **Comments are preserved** where the underlying document model supports — [ADR-0014](adrs/0014-formatter.md)
- The formatter operates on the parsed document, re-orders mapping keys with — [ADR-0014](adrs/0014-formatter.md)
- --check compares the emitted text to the original and exits 1 when — [ADR-0014](adrs/0014-formatter.md)
- --write overwrites the source file atomically — [ADR-0014](adrs/0014-formatter.md)
- The formatter does not resolve imports or merge included files — it — [ADR-0014](adrs/0014-formatter.md)

### Linter

- **handl lint checks documents against an extensible rule set**; paths may — [ADR-0015](adrs/0015-linter.md)
- **A rule is a pure function** from the parsed document + file metadata to — [ADR-0015](adrs/0015-linter.md)
- **Severity is error / warn / info**; exit 0 unless an error is — [ADR-0015](adrs/0015-linter.md)
- **The initial rule set** is naming-convention, missing-annotations, — [ADR-0015](adrs/0015-linter.md)
- **Rules are namespaced and extensible** — hosts register their own — [ADR-0015](adrs/0015-linter.md)

### Lexer Architecture: Self-Hosted Streaming with Runtime Config

- The canonical lexer already processes char-by-char, so yielding tokens incrementally is a natural extension — not a… — [ADR-0022](adrs/0022-lexer-architecture.md)
- LexerConfig provides runtime keyword/delimiter configurability that logos cannot offer without a two-pass approach — [ADR-0022](adrs/0022-lexer-architecture.md)
- The Lexer trait is the abstraction boundary: the parser consumes Vec<Token> regardless of whether tokens came from… — [ADR-0022](adrs/0022-lexer-architecture.md)
- Building a lexer framework would be over-engineering at this scale. The trait + config + streaming method covers the… — [ADR-0022](adrs/0022-lexer-architecture.md)

### Agnostic parser engine: `handl-parser-common` and `handl-parser-canonical` on top

- **ParseError** — a span-aware error value. Carries a message and an optional Span — [ADR-0024](adrs/0024-parser-common.md)
- **Rule<K, N>** — one token kind's behavior in the grammar — [ADR-0024](adrs/0024-parser-common.md)
- left_bp and right_bp for Pratt precedence — [ADR-0024](adrs/0024-parser-common.md)
- nud (null denotation): parse a token with no left context — [ADR-0024](adrs/0024-parser-common.md)
- led (left denotation): parse a token with an already-parsed left operand — [ADR-0024](adrs/0024-parser-common.md)
- **Grammar<K, N> trait** and **SimpleGrammar<K, N>** — the factory configuration — [ADR-0024](adrs/0024-parser-common.md)
- rules: mapping from token kind to Rule — [ADR-0024](adrs/0024-parser-common.md)
- eof: an optional end-of-input kind to verify — [ADR-0024](adrs/0024-parser-common.md)
- parse_root: start a parse using the ParserContext — [ADR-0024](adrs/0024-parser-common.md)
- **ParserContext<K, N>** — the mutable parser state and primitive combinators — [ADR-0024](adrs/0024-parser-common.md)
- peek, advance, is_at_end, match_kind, expect for recursive descent — [ADR-0024](adrs/0024-parser-common.md)
- expression(min_bp) for Pratt operator-precedence parsing — [ADR-0024](adrs/0024-parser-common.md)
- **Parser** — the runtime — [ADR-0024](adrs/0024-parser-common.md)
- parse(&self, tokens: &[Token<K>], source: &str) -> Result<Ast<N>, ParseError> — [ADR-0024](adrs/0024-parser-common.md)
- Ast { source, root } is generic over N, so a dialect can use Node<ExprKind, V, Span> from handl-ast-common or any other… — [ADR-0024](adrs/0024-parser-common.md)
- Define Rules for CanonicalKind::String, CanonicalKind::Number, CanonicalKind::Bool, CanonicalKind::Null,… — [ADR-0024](adrs/0024-parser-common.md)
- The recursive structure (calls, arrays, objects, handler clauses) is built using the ParserContext combinators and… — [ADR-0024](adrs/0024-parser-common.md)
- It continues to produce Expr<NonFunctionValue> from handl-ast — [ADR-0024](adrs/0024-parser-common.md)
- handl-parser keeps the Parser trait as the public boundary for now, but may eventually become a thin re-export over… — [ADR-0024](adrs/0024-parser-common.md)
- handl-ast and handl-runtime are not touched; they stay downstream of the parser — [ADR-0024](adrs/0024-parser-common.md)

### Pluggable Module Composition

- **Replace the static frontend_for(format) and monolithic Dialect** with — [ADR-0028](adrs/0028-pluggable-module-composition.md)
- **Every module folder may carry module.toml** (language kit + per-stage — [ADR-0028](adrs/0028-pluggable-module-composition.md)
- **Naming is handl-*** for kits, lexers, parsers, typecheckers, formatters, — [ADR-0028](adrs/0028-pluggable-module-composition.md)
- **A Composer assembles the pipeline** from the manifest plus each plugin's — [ADR-0028](adrs/0028-pluggable-module-composition.md)
- **All passes are pluggable** — lexer, parser, resolve, typecheck, format, — [ADR-0028](adrs/0028-pluggable-module-composition.md)
- **AST strategy**: plugins may produce different ASTs; a lower/canonicalize — [ADR-0028](adrs/0028-pluggable-module-composition.md)

### Platform Abstractions

- **One crate, handl-platform**, holds the core contracts as modules: — [ADR-0031](adrs/0031-platform-abstractions.md)
- **CapabilityMetadata is opaque serde_json::Value**; the platform — [ADR-0031](adrs/0031-platform-abstractions.md)
- **Default capabilities ship as handl-ext-* plugins** (fmt, lint, typecheck, — [ADR-0031](adrs/0031-platform-abstractions.md)
- **Hosts generate their surface from capability metadata**; the CLI does not — [ADR-0031](adrs/0031-platform-abstractions.md)
- **Existing crates migrate**: host-cli-plugin empties into — [ADR-0031](adrs/0031-platform-abstractions.md)
- **Out of scope** until the layer is stable: Framework, Middleware, — [ADR-0031](adrs/0031-platform-abstractions.md)

### Loaders as Abstract Plugins

- Create handl-plugin-load — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Define the PluginLoader trait (moved from handl-loader) — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Define the Load operation and a LoaderRule contribution type — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Provide LoaderHostPlugin, a RuleAcceptor that collects LoaderRules and exposes a load capability — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- The load capability accepts a PluginManifest payload, tries each collected PluginLoader, and registers the resulting… — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Rename the existing loader crates to the plugin-loader-* namespace: — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- loader → plugin-load (the host-side rule/operation definitions) — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- loader-native → plugin-loader-native — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- loader-wasm → plugin-loader-wasm — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- loader-web → plugin-loader-web — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Each plugin-loader-* crate implements both Plugin and PluginLoader — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- In contribute, it ensures a LoaderHostPlugin is registered, then contributes a LoaderRule wrapping itself — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Because contribute takes &self, the loader must be Clone or wrap a cheaply-cloneable handle — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- DefaultHost gains no new field or special-case API — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Host code that needs to load an external plugin calls the load capability like any other: — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- handl.toml selects loaders through ordinary [[plugin]] entries, not a custom [[loader]] section: — [ADR-0035](adrs/0035-loaders-as-plugins.md)
- Remove handl-loader as a separate framework crate. Its concerns are absorbed by handl-plugin-load and the… — [ADR-0035](adrs/0035-loaders-as-plugins.md)

### Host Applier as a Plugin

- **Create handl-plugin-applier** (or a similarly named default applier plugin crate) — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- Provide DefaultApplierPlugin, a Plugin that reads a host config file and applies it to the host — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- In contribute, read .handl.toml (or another supported config source), parse it into HostConfig, and register the… — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- Kits and plugins are loaded by running capabilities (kit/load and load) on the same host, not by calling hard-coded… — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- **Turn BasicKitLoader into a KitLoaderPlugin** or move its behavior into handl-plugin-applier — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- The default applier provides a kit/load capability — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- kit/load accepts a kit name or path and returns a Kit that the applier then registers with the host — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- **The CLI only bootstraps** — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- host-cli gains --applier <id> (default applier.default) and --config <path> (default .handl.toml) — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- It creates a DefaultHost, registers the default applier plugin (and the default kit/loader plugins if they are not… — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- host.load() runs contribute to a fixed point; the applier reads the config and registers the rest — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- **The existing HostApplier trait and ConfigHostApplier struct can be deprecated** — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- HostConfig and KitReference/PluginReference remain in handl-platform as the in-memory model — [ADR-0036](adrs/0036-host-applier-as-plugin.md)
- The actual _act_ of applying the config moves from handl-platform to a plugin — [ADR-0036](adrs/0036-host-applier-as-plugin.md)

### Plugin Resolution and Excludes

- Collects PluginManifest-like data from the live registry (id, version, capabilities, requires, conflicts, exclusive) — [ADR-0037](adrs/0037-plugin-resolution-and-excludes.md)
- Calls resolve::resolve() with the collected manifests — [ADR-0037](adrs/0037-plugin-resolution-and-excludes.md)
- Returns the resolved composition or an error — [ADR-0037](adrs/0037-plugin-resolution-and-excludes.md)

### Ignored Test Inventory and Triage

- **Workstream 3 (format/diagnostics)** — the formatter and linter pipelines are already implemented and registered in… — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)
- **Workstream 2 (LSP plugins)** — the LSP plugins are implemented and registered. ToolingHost needs to parse the source,… — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)
- **Workstream 1 (VM eval)** — the largest workstream. Unblocks workstream 6 as a side effect. Requires implementing the… — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)
- **Workstream 7 (loader/CLI)** — small effort once ToolingHost exposes a load entry point — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)
- **Workstream 4 (network/TLS)** — move behind a tls feature flag so they don't run by default. No implementation work… — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)
- **Workstream 5 (concurrent stress)** — gate behind a stress feature flag. Fix the scheduler race separately — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)
- **Workstream 6 (HTTP server function values)** — unblocked once workstream 1 is done — [ADR-0038](adrs/0038-ignored-test-inventory-and-triage.md)

### Kits Are Curated Plugin Lists, Not Implementations

- **A kit is a curated list of plugins, not an implementation.** It selects and — [ADR-0039](adrs/0039-kits-are-curated-plugin-lists.md)
- **Composition is dynamic**: a kit is resolved when the host loads it, not — [ADR-0039](adrs/0039-kits-are-curated-plugin-lists.md)
- **Plugins offer capabilities; kits only curate them** — the capability — [ADR-0039](adrs/0039-kits-are-curated-plugin-lists.md)
- **Naming principle**: a kit is named for the surface it curates — [ADR-0039](adrs/0039-kits-are-curated-plugin-lists.md)
- **Migration is four phases**: thin kit-canonical, separate kits from hosts, — [ADR-0039](adrs/0039-kits-are-curated-plugin-lists.md)

### VM is an Operation, Not a Subsystem

- **Dissolve plugin-vm-* and plugin-runtime-*.** Evaluation uses the — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **No VirtualMachine trait, no Value trait, no Runtime operation, no V — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **Eval handlers are granular Behavior<Eval> plugins**, one per AST kind, — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **EvalRequest carries EvalContext** (scopes, evaluator handle, modules, — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **EvalContext lives in plugin-ext-eval**, EvalRequest in platform; — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **Function registration collapses to Module/ModuleRuntime** with a — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **There is no bundled interpreter target**: the 11 plugin-eval-* plugins are — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)
- **One cutover.** The old VM/runtime crates and lego.rs — [ADR-0040](adrs/0040-vm-is-an-operation-not-a-subsystem.md)

### Languages as First-Class Values

- **Language is a handl-level type** — a language definition is a value the — [ADR-0041](adrs/0041-languages-as-first-class-values.md)
- **eval is a handl function** — evaluating an AST is a function call, not a — [ADR-0041](adrs/0041-languages-as-first-class-values.md)
- **self is the running language**, handed to the program, so a program can — [ADR-0041](adrs/0041-languages-as-first-class-values.md)
- **Language combinators are handl functions** — composing languages is — [ADR-0041](adrs/0041-languages-as-first-class-values.md)
- **The bootstrap is a compilation step, not interpretation** — the initial — [ADR-0041](adrs/0041-languages-as-first-class-values.md)
- **Structured by default; vau is a typed escape** for the cases structure — [ADR-0041](adrs/0041-languages-as-first-class-values.md)
- **Capability discipline at the language level** — a language declares what — [ADR-0041](adrs/0041-languages-as-first-class-values.md)

### Language as Data and Runtime Split

- **Language is pure serializable data**, built by merging typed fragments — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)
- **Staged entry points**: each stage consumes only the previous stage's output — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)
- **Crate structure**: each plugin-engine-* stage splits into an abstraction — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)
- **Evaluation schemes are data** — the eval fragment set maps each Ast — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)
- **Fragment merge distinguishes override from collision** via id + — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)
- **Authored data files are canonical.** Rust emitters are a temporary — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)
- **Five review properties are required**: composability (requires), — [ADR-0044](adrs/0044-language-as-data-and-runtime-split.md)

### Capability Discipline at the Language Level

- **CapabilitySet summarizes a Language statically** — host function — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)
- **It is computed by scanning eval schemes**: Foreign adds a name, Vau — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)
- **CapabilityPolicy declares what a context allows** — host fns, vau, — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)
- **A static check at the eval/run/compile boundary** denies before — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)
- **LanguageRegistry carries policies**, attached at registration time by — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)
- **Sub-runtimes inherit a narrowed policy** — a sandboxed language cannot — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)
- **capabilities(handle) is a read-only host function** for handl programs, — [ADR-0046](adrs/0046-capability-discipline-at-the-language-level.md)

### Meta Functions as `plugin-lib-*` Crates

- **Five new leaf crates, one operation each** — eval, lex, parse, — [ADR-0047](adrs/0047-meta-functions-as-plugin-lib-crates.md)
- **handl-runtime becomes the abstraction + Runtime impl**; engines — [ADR-0047](adrs/0047-meta-functions-as-plugin-lib-crates.md)
- **compile / run_compiled are leaf-able** via CompiledHandle and — [ADR-0047](adrs/0047-meta-functions-as-plugin-lib-crates.md)
- **run is dropped** — it is composition of the other operations, not a — [ADR-0047](adrs/0047-meta-functions-as-plugin-lib-crates.md)
- **self stays an ambient global**, handed to the program — [ADR-0047](adrs/0047-meta-functions-as-plugin-lib-crates.md)
- **Scope types re-export from plugin-lib-eval**, so there is one type per — [ADR-0047](adrs/0047-meta-functions-as-plugin-lib-crates.md)

### Modules are Evaluated Environments

- **Environments are first-class values** — a module is the environment its — [ADR-0049](adrs/0049-modules-are-evaluated-environments.md)
- **import / export / module / field-access move off Foreign** and — [ADR-0049](adrs/0049-modules-are-evaluated-environments.md)
- **The import resolver is the loader** — resolution and evaluation are one — [ADR-0049](adrs/0049-modules-are-evaluated-environments.md)
- **The stdlib is .hnl files** evaluated like user modules — [ADR-0049](adrs/0049-modules-are-evaluated-environments.md)
- **The module system is written in handl**, on top of the environment value — [ADR-0049](adrs/0049-modules-are-evaluated-environments.md)
- **Per-language module systems**: each language defines its own import/export — [ADR-0049](adrs/0049-modules-are-evaluated-environments.md)

### Plugin System in Handl

- **Two permanent Rust layers; everything above is handl.** The FFI/binding — [ADR-0050](adrs/0050-plugin-system-in-handl.md)
- **The plugin system is a handl module, not a Rust crate** — registry, — [ADR-0050](adrs/0050-plugin-system-in-handl.md)
- **fmt and lint are handl rule acceptors** with one rule file per AST — [ADR-0050](adrs/0050-plugin-system-in-handl.md)
- **handle fmt|lint dispatches to the handl tooling kit**, not to Rust — [ADR-0050](adrs/0050-plugin-system-in-handl.md)
- **Build order**: binding façade → ast.parse/ast.lex access → handl — [ADR-0050](adrs/0050-plugin-system-in-handl.md)

### Arrow Calculus Kernel

- **app is the primitive; interpretation is the only execution mechanism.** — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Semantic principle: arrows are values** and computation is composition — — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Kernel representation**: Term/Value with the arrow constructors — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **The lowering table (surface construct → arrow form) is normative**: every — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Derived forms** (let, if, loops) are sugar over the kernel, not — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **proc notation and x = y binding are the authoring surface**; they — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Big-bang migration**: one cutover, the old Scheme removed, no dual path — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Handlers are outside the calculus**; execution is one interpretation of — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Feedback semantics** (causal/strict vs lazy/fixpoint) are specified — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)
- **Scope**: kernel + bootstrap + canonical proc surface; everything else — [ADR-0052](adrs/0052-arrow-calculus-kernel.md)

### Generic Pratt Parser — Grammar Data Replaces Per-Construct Branches

- **TokenKindDef.literal: Option<LiteralKind>** replaces the — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **AstShapeRef.child_separators: Vec<ChildSeparator>** replaces the — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **DelimitedShape.open_is_operator_token: bool** — the open delimiter is — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **DelimitedShape.flatten: bool** splices elements into the parent's — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **DelimitedShape.rest_token: Option<String>** replaces the — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **The infix path consults delimited**, mirroring the prefix path — the — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **Parser desugars move to a data-driven post-parse pass.** if becomes a — [ADR-0053](adrs/0053-generic-pratt-parser.md)
- **Contextual forms (match, data, from) are closed by grammar data** — — [ADR-0053](adrs/0053-generic-pratt-parser.md)

### Generic Typecheck Engine (`plugin-engine-typecheck` + `plugin-engine-typecheck-nbe`)

- **Type rules reuse Scheme — lang.types.** A rule synthesizes a node's — [ADR-0054](adrs/0054-generic-typecheck-engine.md)
- **Judgment ops are Arr ops** (unify, quote, normalize, meta-solve, — [ADR-0054](adrs/0054-generic-typecheck-engine.md)
- **plugin-engine-typecheck is the abstraction crate** — marker, — [ADR-0054](adrs/0054-generic-typecheck-engine.md)
- **plugin-engine-typecheck-nbe implements TypecheckEngine.** NBE — — [ADR-0054](adrs/0054-generic-typecheck-engine.md)
- **Typecheck is wired on RuntimeHandle (pure path).** Types are values, so — [ADR-0054](adrs/0054-generic-typecheck-engine.md)
- **plugin-lib-lang-typecheck is the meta leaf** exposing — [ADR-0054](adrs/0054-generic-typecheck-engine.md)
- **Rules are authored data** in crates/handl-kit-data-canonical/data/types.yaml, — [ADR-0054](adrs/0054-generic-typecheck-engine.md)

### Tagged Values

- **Tag registries** define the legal tags per host; a tag is registered once — [ADR-0055](adrs/0055-branded-values.md)
- **tag / match_tag / tag_registry are host capabilities**, not syntax — [ADR-0055](adrs/0055-branded-values.md)
- **Representation**: a tagged value is a serde_json::Value carrying its tag — [ADR-0055](adrs/0055-branded-values.md)
- **Two equality flavors, one primitive**: structural equality and — [ADR-0055](adrs/0055-branded-values.md)
- **Forgery is impossible** — a tag can only be minted through the registered — [ADR-0055](adrs/0055-branded-values.md)
- **Sugar**: type (and future unit) declares tags; the primitive is the — [ADR-0055](adrs/0055-branded-values.md)
- **Untagged values are unchanged**; tagging is additive — [ADR-0055](adrs/0055-branded-values.md)

### Import / Export and Multi-File Programs

- **A module is an environment value** — evaluating a file produces the — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)
- **import resolves source and caches the resulting environment** (resolve → — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)
- **export defines a module's public surface**; it is not a visibility — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)
- **field-access is the module access mechanism** — module.name reads the — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)
- **Import capability is a policy boundary**, not a language concept: the host — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)
- **The stdlib becomes .hnl files** evaluated into environments — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)
- **The runtime exposes environments as values**, so modules are inspectable — [ADR-0056](adrs/0056-import-export-and-multi-file-programs.md)

### Generic Data-Driven Engines

- Looks up a node's eval scheme from Language.eval (already data-driven) — [ADR-0058](adrs/0058-generic-data-driven-engines.md)
- Interprets the scheme's Arr/Seq/First/App combinators (already — [ADR-0058](adrs/0058-generic-data-driven-engines.md)
- Dispatches native ops to **data-specified behavior**, not hardcoded — [ADR-0058](adrs/0058-generic-data-driven-engines.md)
- **Desugar rules** (for syntactic rewrites — [k]: → computed key, — [ADR-0058](adrs/0058-generic-data-driven-engines.md)
- **Scheme descriptions** (for eval-time behavior — new native ops are — [ADR-0058](adrs/0058-generic-data-driven-engines.md)

### Generic Object Key Evaluation

- **var or string** — use the payload string directly (literal key) — [ADR-0059](adrs/0059-generic-object-key-evaluation.md)
- **anything else** — evaluate the expression and use its value's string — [ADR-0059](adrs/0059-generic-object-key-evaluation.md)

### Generic Call Dispatch

- id: eval.call — [ADR-0060](adrs/0060-generic-call-dispatch.md)
- !Arr { op: proj_child, args: [], source: Native, lit: 0 } — [ADR-0060](adrs/0060-generic-call-dispatch.md)
- !Arr { op: proj_child, args: [], source: Native, lit: 1 } — [ADR-0060](adrs/0060-generic-call-dispatch.md)
- **typeof** — a native op under arr that returns a type tag string — [ADR-0060](adrs/0060-generic-call-dispatch.md)
- **route_on_type** — a native op under arr that takes a value and — [ADR-0060](adrs/0060-generic-call-dispatch.md)
- **call_host** — a native op that takes a string callee and an args — [ADR-0060](adrs/0060-generic-call-dispatch.md)
- **ArrSource::DynamicForeign** — a new variant of ArrSource — [ADR-0060](adrs/0060-generic-call-dispatch.md)

### Generic Pattern Match

- **define_binding** — bind a name to a value in the current scope — [ADR-0061](adrs/0061-generic-pattern-match.md)
- **eval_pattern_node** — evaluate a pattern AST node as a runtime — [ADR-0061](adrs/0061-generic-pattern-match.md)
- **Desugar patterns into match-op AST nodes.** Each pattern form — [ADR-0061](adrs/0061-generic-pattern-match.md)
- **Express match/handle as Loop + Choice.** The arm-by-arm — [ADR-0061](adrs/0061-generic-pattern-match.md)
- **The shared try_pattern becomes the try_pattern_arm native op.** — [ADR-0061](adrs/0061-generic-pattern-match.md)

### Scheme-Core Completeness for Self-Hosting

- **Genuine host effect** (I/O, FFI, capability-tracked) — keep — [ADR-0063](adrs/0063-scheme-core-completeness.md)
- **Meta-operation masquerading as host function** (eval, parse, — [ADR-0063](adrs/0063-scheme-core-completeness.md)

### Derived-Artifact Cache and Compression

- Every fragment source (YAML file content hash, or handl program content — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- The kit manifest (ordered fragment-source list + include/exclude filters) — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- The extends chain (transitive: parent kits' manifests, recursively) — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- Engine selections (lex/parse/eval engine names + versions, per ADR-0044 — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- The scheme_version declared by the Language (ADR-0044 §7) — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- NoCache — always misses (default for compile-time-embedded kits) — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- InMemoryCache — HashMap<CacheKey, CachedLanguage>, process-lifetime — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- OnDiskCache — files in a cache directory, persistent across runs — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)
- (future) NetworkCache — shared cache for a fleet of hosts — [ADR-0065](adrs/0065-derived-artifact-cache-and-compression.md)

### Dependency-Graph Chunking

- Its fragment data (serde-serializable, per ADR-0044) — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Its dependencies: references to other chunks (by CacheKey, per — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Its replaces targets (so the override graph can be resolved across — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Decompress the chunk — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Resolve dependencies: load each dependency chunk (recursively, depth-first) — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Merge the loaded chunks' fragments (ADR-0044 §5 merge semantics) — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Typecheck the loaded subset (ADR-0044 §1 AstShapeDef payload schemas, — [ADR-0066](adrs/0066-dependency-graph-chunking.md)
- Eval — [ADR-0066](adrs/0066-dependency-graph-chunking.md)

### Chunk Certificates

- **A certificate is an Ed25519 detached signature** over the canonical — [ADR-0067](adrs/0067-chunk-certificates.md)
- **Verification happens before load**: recompute the hash, verify the — [ADR-0067](adrs/0067-chunk-certificates.md)
- **Dependency certificates are required** and pin each dependency's content — [ADR-0067](adrs/0067-chunk-certificates.md)
- **Author keys are distributed out of band** — key server, pinned host — [ADR-0067](adrs/0067-chunk-certificates.md)
- **Certificates complement capability checks**: ADR-0046 inspects what the — [ADR-0067](adrs/0067-chunk-certificates.md)

### Arrow-Native Modules

- **A file is an arrow.** A module is the arrow its file evaluates to, not a — [ADR-0069](adrs/0069-arrow-native-modules.md)
- **Import normalizes to projection**: importing a name is selecting a field — [ADR-0069](adrs/0069-arrow-native-modules.md)
- **Using a name is App** — a reference is application of a projection, not — [ADR-0069](adrs/0069-arrow-native-modules.md)
- **Two caches: graphs and contracts.** Normalized module graphs and their — [ADR-0069](adrs/0069-arrow-native-modules.md)
- **Normalization is one abstraction with many implementations**; the — [ADR-0069](adrs/0069-arrow-native-modules.md)
- **No export, no visibility, no environments.** Those concepts disappear — [ADR-0069](adrs/0069-arrow-native-modules.md)
- **Multi-file programs are arrow composition** — linking is composition — [ADR-0069](adrs/0069-arrow-native-modules.md)

### Refinement Inference Scope — Deterministic Dataflow

- **List operations**: sort, map, filter, fold, insert, take, drop, zip, — [ADR-0070](adrs/0070-refinement-inference-scope.md)
- **Parsers (recursive descent, no backtracking)**: Pratt parsing, — [ADR-0070](adrs/0070-refinement-inference-scope.md)
- **State machines (data invariants)**: counters, bounded buffers, — [ADR-0070](adrs/0070-refinement-inference-scope.md)
- **Parsers with backtracking**: need failure as a first-class effect — [ADR-0070](adrs/0070-refinement-inference-scope.md)
- **Concurrent systems**: need non-deterministic scheduling and — [ADR-0070](adrs/0070-refinement-inference-scope.md)
- **Non-deterministic computation**: the inference assumes the graph — [ADR-0070](adrs/0070-refinement-inference-scope.md)

### Concurrent Arrow Calculus

- receive requires: channel non-empty (precondition) — [ADR-0072](adrs/0072-concurrent-arrow-calculus.md)
- send guarantees: channel non-empty after (postcondition) — [ADR-0072](adrs/0072-concurrent-arrow-calculus.md)

### Stdlib Migration to Handl — `op` Declarations, Axioms, and the Elimination of Per-Library Rust

- The stdlib is handl source. Every plugin-lib-* library is a .hnl file — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- Every public arrow carries a --- doc comment (the API contract in prose) — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- A pure-handl arrow needs no proof file: the contract cache infers each arrow — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- op declarations are the FFI surface — op "name" : A -> B guarantee: … — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- Every migrated file declares its capabilities and follows the — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- The six-phase port plan, the per-file requirements and the per-library — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- Native bodies are handl over direct C/POSIX FFI (ADR-0083) — no Rust — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)
- Each library's public API is preserved across its migration; a breaking — [ADR-0077](adrs/0077-stdlib-migration-to-handl.md)

### Data-Driven AST Child Roles — Closing the Last Engine Hardcoding

- id: ast.assign — [ADR-0081](adrs/0081-data-driven-ast-child-roles.md)
- role: target — [ADR-0081](adrs/0081-data-driven-ast-child-roles.md)
- role: value — [ADR-0081](adrs/0081-data-driven-ast-child-roles.md)
- id: ast.block — [ADR-0081](adrs/0081-data-driven-ast-child-roles.md)
- role: statements — [ADR-0081](adrs/0081-data-driven-ast-child-roles.md)

### FFI-Native Stdlib — Handl over Direct C/POSIX Bindings

- **ffi_bind is a host-exposed arrow op.** A binding is data — library, — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **Marshalling is a closed ABI vocabulary**: scalars, pointers, structs, — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **Errors are declared per binding.** An optional error clause maps errno — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **A Rust FFI dispatcher does only the mechanics**: load the library and — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **The abstraction layer is handl**: POSIX wrappers, constants and the — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **Platform variants are abstracted from day one** (Linux first) and chosen — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **Rust keeps only the kernel**: dispatcher, module loader, engines, host and — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **What is eliminated**: per-library Rust Function bodies and the — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **The async runtime is handl-authored from day one**: epoll_wait for — [ADR-0083](adrs/0083-ffi-native-stdlib.md)
- **Kernel freedom**: the kernel knows nothing of POSIX, libc or any library — — [ADR-0083](adrs/0083-ffi-native-stdlib.md)

### Bootstrap Tooling Strategy — Rust Tooling on the Generic Substrate

- **The substrate stays and is the implementation.** The plugin system, — [ADR-0085](adrs/0085-bootstrap-tooling-strategy.md)
- **Best practices in Rust, now.** Composability, extensibility, — [ADR-0085](adrs/0085-bootstrap-tooling-strategy.md)
- **Canonical style/rules are hardcoded; the machinery is generic.** What — [ADR-0085](adrs/0085-bootstrap-tooling-strategy.md)
- **Minimal decency, not prototyping quality.** Small module boundaries — [ADR-0085](adrs/0085-bootstrap-tooling-strategy.md)
- **The CLI stays thin.** host-cli gains/keeps subcommands that dispatch — [ADR-0085](adrs/0085-bootstrap-tooling-strategy.md)
- **Port to handl in the future, improve in place until then.** When handl — [ADR-0085](adrs/0085-bootstrap-tooling-strategy.md)

### Bootstrap Formatter (`plugin-ext-fmt`)

- **Input/output:** surface source text → parse (via — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- **Hardcoded style, no configuration.** Canonical preferences: — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- Two-space indentation; continuation lines align with enclosing arrows — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- One statement per line inside graph blocks; a blank line between — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- Single trailing newline; no trailing whitespace; no more than one — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- >>> chains break _before_ the operator, one segment per line, when a — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- No spaces inside brackets; single spaces around binary operators per — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- **Idempotence:** fmt(fmt(src)) == fmt(src) — tested exhaustively — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- **API surface (minimal):** — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- format_source(src: &str) -> Result<String, FormatError> — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- check_source(src: &str) -> Result<bool, FormatError> (true = already — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- **CLI contract:** hnl fmt [--check] [--write] [paths...]. --check — [ADR-0086](adrs/0086-bootstrap-formatter.md)
- **Testing:** 100% coverage. Golden-file tests under tests/golden/ (one — [ADR-0086](adrs/0086-bootstrap-formatter.md)

### Bootstrap Linter (`plugin-ext-lint`)

- **Core types:** re-exported from plugin-ext-diag (ADR-0091) — this ADR — [ADR-0087](adrs/0087-bootstrap-linter.md)
- **Initial rule set** (all v0.1-surface-specific): — [ADR-0087](adrs/0087-bootstrap-linter.md)
- unused-import (Warning, fixable) — import projection never referenced — [ADR-0087](adrs/0087-bootstrap-linter.md)
- unbound-wire (Error) — reference to a name not bound in the enclosing — [ADR-0087](adrs/0087-bootstrap-linter.md)
- shadowed-binding (Warning) — re-binding a name already in scope — [ADR-0087](adrs/0087-bootstrap-linter.md)
- dead-branch (Warning) — choice arm guarded by a constant condition — [ADR-0087](adrs/0087-bootstrap-linter.md)
- naming-convention (Warning, fixable) — bindings are snake_case, — [ADR-0087](adrs/0087-bootstrap-linter.md)
- missing-module-doc (Info) — a module lacks a leading doc comment — [ADR-0087](adrs/0087-bootstrap-linter.md)
- fmt (Warning, fixable) — file is not canonical per ADR-0086; the fix — [ADR-0087](adrs/0087-bootstrap-linter.md)
- **Exit contract:** exit 1 iff any Error diagnostic; --fail-on-warn — [ADR-0087](adrs/0087-bootstrap-linter.md)
- **Rule architecture:** rules are pure functions — [ADR-0087](adrs/0087-bootstrap-linter.md)
- **Testing:** 100% coverage; every rule has positive/negative/fix cases; — [ADR-0087](adrs/0087-bootstrap-linter.md)

### Bootstrap Typechecker (`plugin-engine-typecheck`)

- **Scope (v0.1):** — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- Arrow kind checking of composed graphs (arr/>>>/first/choice — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- Structural type inference for literals, records, positional — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- rec region well-formedness: feedback wires are consumed exactly — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- Module-level checks: every import projection used, every exported — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- **Not in scope (deferred to RFC-0004):** trait/impl checking (ADR-0075), — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- **API surface:** — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- **Architecture:** single pass composition — collect bindings → infer — [ADR-0088](adrs/0088-bootstrap-typechecker.md)
- **Testing:** 100% coverage. Table-driven tests/ per typing rule from — [ADR-0088](adrs/0088-bootstrap-typechecker.md)

### Bootstrap Test Runner (`plugin-engine-test`)

- **A new plugin-engine-test crate owns the runner** — discovery, index, — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **plugin-lib-test becomes the framework library** (assert/property/ — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **Stable pipeline contracts**: Providers → Discovery → Index/Cache → Filter — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **Core types are hard to change and versioned**; internal contracts are — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **Test declaration is a merged language fragment**, not the test_* — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **Providers are extensible from day one**; discovery/index and filtering — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **Planner and scheduler are separate; the scheduler is generic**; the — [ADR-0089](adrs/0089-bootstrap-test-runner.md)
- **Determinism is a hard invariant**; observability emits structured events — [ADR-0089](adrs/0089-bootstrap-test-runner.md)

### Bootstrap CLI (`host-cli`)

- **Command surface (v0.1):** — [ADR-0090](adrs/0090-bootstrap-cli.md)
- **No _new_ plugin system.** No capability registry, no dynamic loading, — [ADR-0090](adrs/0090-bootstrap-cli.md)
- **Coupling policy:** the CLI is allowed to know about every tool crate, — [ADR-0090](adrs/0090-bootstrap-cli.md)
- **Exit codes:** 0 success; 1 check/lint/test failure; 2 usage or — [ADR-0090](adrs/0090-bootstrap-cli.md)
- **Diagnostics:** all subcommands print through the shared renderer in — [ADR-0090](adrs/0090-bootstrap-cli.md)
- **Testing:** 100% coverage of argument parsing, dispatch, exit-code — [ADR-0090](adrs/0090-bootstrap-cli.md)

### Shared Diagnostics Crate (`plugin-ext-diag`)

- **Types:** Diagnostic { rule, severity, span, message, fix? }, — [ADR-0091](adrs/0091-plugin-ext-diag.md)
- **Rendering:** one shared renderer (human `file:line:col: severity — [ADR-0091](adrs/0091-plugin-ext-diag.md)
- **Fix application:** span-ordered, idempotent text-edit application with — [ADR-0091](adrs/0091-plugin-ext-diag.md)
- **Relationship to plugin-diag-expression:** it keeps its — [ADR-0091](adrs/0091-plugin-ext-diag.md)
- **Generic, not surface-coupled:** the crate knows spans and text, not the — [ADR-0091](adrs/0091-plugin-ext-diag.md)

### Test Framework in Handl

- **Target layout**: the framework lives in handl-packages/test/ as handl — [ADR-0092](adrs/0092-test-framework-in-handl.md)
- **Migration per op**: each framework operation moves from Rust to a handl — [ADR-0092](adrs/0092-test-framework-in-handl.md)
- **Crate boundary**: plugin-lib-test is deleted; the runner — [ADR-0092](adrs/0092-test-framework-in-handl.md)
- **Relationship to ADR-0089**: the runner stays Rust; this ADR moves only the — [ADR-0092](adrs/0092-test-framework-in-handl.md)
- **Relationship to ADR-0077**: the framework is part of the stdlib migration; — [ADR-0092](adrs/0092-test-framework-in-handl.md)
- **What stays Rust**: the test runner only — [ADR-0092](adrs/0092-test-framework-in-handl.md)

### String / Char / Unicode Layering

- str-handl calls only char.* semantic atoms. It never indexes bytes, — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- char names are _semantic_: is_whitespace, is_alpha, to_upper, etc — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- Indices in char and str are character (scalar) indices, not byte offsets — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- char.chars and char.from_chars are the bulk conversion atoms. They make — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- unicode-c provides the subset computable from char.* alone exactly, and — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- kit-canonical resolves the module name "str" to str-handl through its — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- Non-canonical kits that do not wire the char resolver still get the FFI — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- "char" resolves to CharModule (plugin-lib-char-native) directly in — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- "unicode_c" resolves to the unicode_c table module (unicode-c.hnl), — [ADR-0093](adrs/0093-string-char-unicode-layering.md)
- The module name is unicode_c, **not** unicode (a deliberate surface — [ADR-0093](adrs/0093-string-char-unicode-layering.md)

### Generic AST Transform Engine (`plugin-engine-transform` + `plugin-engine-transform-rewrite`)

- **Two crates, mirroring the exec split.** plugin-engine-transform is the — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)
- **TransformDef is a first-class Fragment in handl-language.** Its — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)
- **The rewrite vocabulary generalizes AstDesugarChild.** Keep, KeepPath, — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)
- **Traversal is part of the pass definition** — BottomUp, TopDown or — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)
- **RuleSource is the seam.** The default reads lang.transforms; an — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)
- **Desugar migrates onto the engine.** handl-runtime::desugar_pass becomes a — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)
- **Crate layout has no cycles**: contract/vocabulary, rewrite implementation, — [ADR-0094](adrs/0094-generic-ast-transform-engine.md)

### Formatter as a Production Pipeline

- **Crate boundary**: the formatter keeps the abstraction/engine split — — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Pipeline and stage contracts**: parse → layout → emit, with stable — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Rule seam**: FormattingRule is replaceable and composable; formatting — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Config seam**: formatting options are resolved once and passed as a — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Engine seam**: NodeEmitter stays the swap point for a different layout — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Generic capability**: platform/format.rs is either wired to the real — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Language-as-data style**: no hardcoded kind strings; the formatter reads — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)
- **Performance**: parallel --write/--check across files and a — [ADR-0095](adrs/0095-formatter-as-production-pipeline.md)

### Module Resolution — Path-Based Imports + `.handl.toml` Path Dependencies

- **Dependency specifier.** If the first /-separated segment of <spec> — [ADR-0096](adrs/0096-module-resolution.md)
- **Relative path.** Otherwise <spec> resolves against the directory of — [ADR-0096](adrs/0096-module-resolution.md)

### Lint Rules as Language Data

- **LintRuleDef fragment** (handl-language::fragments::lint_rule), — [ADR-0097](adrs/0097-lint-rules-as-data.md)
- **Two check kinds**, mirroring ADR-0094's declarative/native split: — [ADR-0097](adrs/0097-lint-rules-as-data.md)
- check: !Match { node_kind, when? } — node-local matching. when is a — [ADR-0097](adrs/0097-lint-rules-as-data.md)
- check: !Native "name" — the disciplined escape hatch, resolved — [ADR-0097](adrs/0097-lint-rules-as-data.md)
- **Module-level engine API** in plugin-engine-lint: — [ADR-0097](adrs/0097-lint-rules-as-data.md)
- **Canonical natives** ship in plugin-engine-lint — [ADR-0097](adrs/0097-lint-rules-as-data.md)
- **Canonical rules live in handl-kit-data-canonical/data/lints.yaml**, — [ADR-0097](adrs/0097-lint-rules-as-data.md)

### One Language, Multiple Interpreters — A Graded Arrow Calculus

- **Everything is an arrow, and the arrow language is graded.** Grades are — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **The graded-theory map is the grade vocabulary** — each grade names its — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **plugin-engine is the base Term language plus the Model trait** that — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **All primitives and combinators are optional per runtime**; a model — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **The arrow-handler algebra is data-driven** and interprets Term; handlers — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **Scheme/Term is the calculus**, with a fixed shape; interpreters — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **Surface arrow combinators are Term sugar**; bounded repetition is a — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **Sorts are grade records**; the data keyword is removed and inductive — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **Generics are Generic tagged applications over grades**, not a separate — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)
- **Effects stay structural and handlers composable**: an op declares its — [ADR-0098](adrs/0098-one-language-multiple-interpreters.md)

### Principled Boundaries Between `plugin-engine-*` and `plugin-ext-*` Crates

- plugin-engine-<name> crates are **generic, modular, composable engines** — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- They operate on handl_language::Language data and are not tied to the canonical handl v0.1 grammar — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- They contain no canonical rule sets, no embedded handl-kit-data-canonical data, and no hnl subcommand wiring — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- They may expose operation markers, traits, and abstract request/response types (e.g. Typecheck, TypecheckEngine,… — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- A single engine crate may contain multiple internal modules/algorithms. The typechecker engine, for example, is one… — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- plugin-ext-<name> crates are the **canonical handl bootstrap extension** for a tool — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- They depend on the matching plugin-engine-<name> crate and on handl-kit-data-canonical — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- They supply the canonical rules, language assembly, and the hnl <name> capability wiring — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- They are the only crates that may embed canonical handl rule data or register a hnl subcommand — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- Existing violations must be remediated: — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- Create plugin-ext-typecheck and move the canonical check_module entry point out of plugin-engine-typecheck.… — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- Merge plugin-engine-typecheck-nbe into plugin-engine-typecheck as an internal nbe module — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- Convert the arrow-contract inference algorithm from plugin-ext-refinement into a refinement module inside… — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- Add a composite (or pipeline) module in plugin-engine-typecheck that runs nbe then refinement on the appropriate… — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)
- No plugin-ext-* crate may implement a generic algorithm. No plugin-engine-* crate may expose a bin target or a hnl… — [ADR-0099](adrs/0099-plugin-engine-vs-ext-crate-boundaries.md)

### Canonical Term Arrows

- **The canonical internal form is the arrow-calculus Term** — no — [ADR-0100](adrs/0100-canonical-term-arrows.md)
- **The constructor set is fixed**: Arr, Const, Op, Seq, First, — [ADR-0100](adrs/0100-canonical-term-arrows.md)
- **x ~> body lowers point-free** using arr and the product combinators; — [ADR-0100](adrs/0100-canonical-term-arrows.md)
- **Calls lower to App** when the callee is dynamic and to Seq when it is — [ADR-0100](adrs/0100-canonical-term-arrows.md)
- **Records, operators and literals lower to Op/Arr/Const**; there are — [ADR-0100](adrs/0100-canonical-term-arrows.md)
- **The engine interprets Term directly**; eval.yaml describes how each — [ADR-0100](adrs/0100-canonical-term-arrows.md)

### The Directed-HoTT Limit of the Term Calculus

- **Term is a 2-category**: objects are sorts, 1-cells are arrows, 2-cells — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Language is a presented 2-category** — its data (sorts, arrows, rewrites) — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **A Model is a 2-functor** from the presented theory to a semantic — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **The model family** (value, type, exec, meta, transform) are 2-functors — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Syntax for rewrites and paths** is data (law, rewrite, Path), not — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Univalence and transport** are 1-cell operators derived from the — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Grades are propositions** — a grade is a proposition about a cell, and — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Directed paths**: Term is a directed (∞,1)-category; the 2-cells are — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Implementation**: add Term::Rewrite, law/rewrite declarations, — [ADR-0100](adrs/0100-directed-hott-limit.md)
- **Keep the 1-categorical engine**: evaluation never requires 2-cells; a — [ADR-0100](adrs/0100-directed-hott-limit.md)

### Wiring `.hnl` modules into `HandlNativeLint` and `HandlNativeRewrite`

- Extend handl-kit-canonical to track every .hnl-backed HandlModule it registers, alongside the existing ModuleRegistry.… — [ADR-0100](adrs/0100-handl-natives-wire.md)
- Expose Kit::handl_lint_adapter() and Kit::handl_rewrite_adapter() methods that build fully populated HandlNativeLint… — [ADR-0100](adrs/0100-handl-natives-wire.md)
- Wire the lint adapter into plugin-ext-lint::lint_engine and host-tooling::diagnose by adding it as a resolver on the… — [ADR-0100](adrs/0100-handl-natives-wire.md)
- Wire the rewrite adapter into host-cli transform by layering it over the desugar.* native registry — [ADR-0100](adrs/0100-handl-natives-wire.md)
- Add a RewriteEngine::with_desugar_and_handl_natives constructor that installs both the handl-native registry and the… — [ADR-0100](adrs/0100-handl-natives-wire.md)

### The Kernel Is the Evaluator, Not the Calculus

- **The kernel is the evaluator, not the calculus**: a bytecode VM over values, — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **The ISA is small and structural**; anything expressible as data is not a — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **Computation is represented as data** (TermData over serde_json::Value), — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **exec_model.hnl** is the scheme interpreter over VM primitives — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **arrow_model.hnl** implements all Term tags of the arrow calculus — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **type_model.hnl** implements the HoTT type judgments — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **meta_model.hnl** implements module loading and evaluation — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **The categorical picture**: composition and the arrow primitives are the — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)
- **Rust keeps the VM and the model adapters**; the models are handl data — [ADR-0103](adrs/0103-kernel-is-evaluator-not-calculus.md)

### Typechecker in Handl

- **The typechecker's models are handl**; Rust keeps only the engine/driver — [ADR-0105](adrs/0105-typechecker-in-handl.md)
- **What stays in Rust**: the engine, VM primitives and the adapter that loads — [ADR-0105](adrs/0105-typechecker-in-handl.md)
- **What moves to handl**: the type model (judgment ops) plus the scope and — [ADR-0105](adrs/0105-typechecker-in-handl.md)
- **Bootstrap**: Rust embeds the .hnl model with include_str!, loads it into — [ADR-0105](adrs/0105-typechecker-in-handl.md)
- **The adapter is thin** — the model is the implementation, not a wrapper — [ADR-0105](adrs/0105-typechecker-in-handl.md)
- **Parity is proven against the Rust model** by the existing type-model — [ADR-0105](adrs/0105-typechecker-in-handl.md)

### The Object-Arrow Kernel

- **One semantic world.** Values, objects, types and arrows share one value — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **sym() / sym(A) is the nominal identity primitive** — names are values, — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Arrows are the fundamental computational primitive.** There is no binding — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Products and composition are the structural core**; do is sugar over — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Dimensions <> specialize any value or arrow**, and generic objects use — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Graded arrows** carry effects in the grade, not in special forms — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Algebra, capabilities and ordering** use the same objects and arrows — no — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Proofs, refinements and dependent types are ordinary objects/arrows.** — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Polymorphism and Liskov are sym-based**; higher-order arrows and — [ADR-0106](adrs/0106-object-arrow-kernel.md)
- **Expected-type inference and runtime/static convergence** follow from the — [ADR-0106](adrs/0106-object-arrow-kernel.md)

### Handl SDK Generator

- **Port-adapter model**: a generated *port* is the abstract Handl interface; — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **New crates**: plugin-sdkgen (shared IR), a backend per source format, an — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **Multi-backend by three dimensions** — source format, target ABI, emitter — — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **The C backend uses libclang**, not a hand-rolled parser — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **File output mirrors the module tree**; generated packages are ordinary — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **Output form**: abstract export in the port, concrete — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **All FFI arrows carry the FFI grade**, so effects are visible in the — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **Mappings to Handl types/grades are data**, not backend code — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **Doc comments are emitted** from the source into the port — [ADR-0107](adrs/0107-handl-sdk-generator.md)
- **Type/grade composition and the CLI surface** are part of the generator: — [ADR-0107](adrs/0107-handl-sdk-generator.md)

### Handl Compile Plugin Architecture

- **A plugin-compile crate defines Compiler and CompilerInput** (a parsed, — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)
- **handl-emitters keeps its role** — emitting Language data fragments; it — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)
- **hnl compile <target> <file> dispatches by target name**, not file — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)
- **The JS target exposes** compile_to_string, emit_module and — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)
- **Value lowering** maps Handl primitives to JS, Arr/Obj to arrays/objects, — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)
- **Foreign calls lower to js.call(...)** and inline to the native call when — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)
- **v1 emits no source maps**; # sourceURL may be added in esm/inline — [ADR-0114](adrs/0114-handl-compile-plugin-architecture.md)

### Paper/Implementation Alignment

- **Phase 1 — arrows as values**: a function used as a value lowers to — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Phase 1 — no self-application**: desugar.data's recursor becomes — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Phase 2 — composition axes**: Term::Seq gains an axis slot (default — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Phase 3 — dim unification**: ArrSource::Foreign(grade) merges into the — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Phase 3 — effects as cells**: op is a cell and handle is a cell — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Phase 4 — dependent structure**: Family<A>, Π, Σ enter via — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Phase 5 — laws as M[2] terms**: f ~[2]~> g parses to a Term and — [ADR-0120](adrs/0120-paper-implementation-alignment.md)
- **Ordering is load-bearing**: 1 → 2 → 3 → (4, 5 in parallel), with the — [ADR-0120](adrs/0120-paper-implementation-alignment.md)

### AST shape faces — closing the last kind-dispatch sites

- id: ast.assign — [ADR-0123](adrs/0123-ast-shape-faces.md)
- id: ast.bind — [ADR-0123](adrs/0123-ast-shape-faces.md)
- id: ast.function — [ADR-0123](adrs/0123-ast-shape-faces.md)
- id: ast.call — [ADR-0123](adrs/0123-ast-shape-faces.md)
- id: ast.dim_arrow — [ADR-0123](adrs/0123-ast-shape-faces.md)

### Handing a `Language` to capability plugins

- A host serializes its language **once** and reuses it (an Arc<Value> or — [ADR-0124](adrs/0124-capability-language-handoff.md)
- A plugin that needs a language reads payload["language"], deserializes it, — [ADR-0124](adrs/0124-capability-language-handoff.md)
- This path is **portable**: it works for in-process hosts, WASM plugins, and — [ADR-0124](adrs/0124-capability-language-handoff.md)
- an Arc<Language> placed directly on the request (fast path, exact); — [ADR-0124](adrs/0124-capability-language-handoff.md)
- the request's LanguageResolver, asked for payload["kit"] (or the — [ADR-0124](adrs/0124-capability-language-handoff.md)
- otherwise PlatformError::InvalidMetadata naming *both* ways to supply a — [ADR-0124](adrs/0124-capability-language-handoff.md)

### Kernel boundary, and the idea ledger for retired reifications

- **The boundary rule.** Rust holds the **kernel** (the ADR-0040 primitives) — [ADR-0126](adrs/0126-kernel-boundary-and-the-idea-ledger.md)
- **Unwired is a declared state, not an accident.** Every workspace member is — [ADR-0126](adrs/0126-kernel-boundary-and-the-idea-ledger.md)
- **project/parked.yaml is the idea ledger.** Each retired entry carries the — [ADR-0126](adrs/0126-kernel-boundary-and-the-idea-ledger.md)
- **Retirement protocol.** Classify (aspirational → proposed; replaced → — [ADR-0126](adrs/0126-kernel-boundary-and-the-idea-ledger.md)
- **The gate is a test, not an audit.** crates/platform-tests/tests/wiring.rs — [ADR-0126](adrs/0126-kernel-boundary-and-the-idea-ledger.md)

### `TransformRule::Replace` — a rule form for root-eliminating rewrites

- path is interpreted exactly as RewriteStep::KeepPath's — a non-empty — [ADR-0127](adrs/0127-transform-rule-replace.md)
- A path that does not resolve to an existing descendant (out of range, or — [ADR-0127](adrs/0127-transform-rule-replace.md)
- The rule stays data-shaped: dispatch is still `(pass, node_kind, — [ADR-0127](adrs/0127-transform-rule-replace.md)
- It does **not** create, splice, or wrap anything. Replace returns a node — [ADR-0127](adrs/0127-transform-rule-replace.md)
- It does **not** change KeepPath's existing behaviour (a missed path in a — [ADR-0127](adrs/0127-transform-rule-replace.md)
- It is **not** a general subterm-selection language. There is one path, from — [ADR-0127](adrs/0127-transform-rule-replace.md)
- It does **not** make Native redundant. Native remains the escape hatch — [ADR-0127](adrs/0127-transform-rule-replace.md)

### Target Repos — Repo = Target, Module = One Port Implementation

- **Repo = target/ecosystem; module = one implementation of one port.** A — [ADR-0128](adrs/0128-target-repos.md)
- **Module directories carry the specific name** (modules/<port>-<specific>), — [ADR-0128](adrs/0128-target-repos.md)
- **Compiler targets are one repo each** — dot/, term/, sexp/, handl/, — [ADR-0128](adrs/0128-target-repos.md)
- **Platform keeps** ports, core crates, kit machinery, engines, wiring, — [ADR-0128](adrs/0128-target-repos.md)
- **The per-crate extraction map** (26 rows of crate → repo/module) is the — [ADR-0128](adrs/0128-target-repos.md)
- **The platform repo is renamed host/**; handl/ means the language target; — [ADR-0128](adrs/0128-target-repos.md)
- **Hosts sit at the top of the fan**: host-cli and host-lsp declare — [ADR-0128](adrs/0128-target-repos.md)
- **Git deps bypass registries**: consumers set — [ADR-0128](adrs/0128-target-repos.md)
- **The lang/ cycle is broken by injection**: consumers take &Language/a — [ADR-0128](adrs/0128-target-repos.md)

### Project-docs topology for the target-repo split

- **Every repo carries its own docs on a project orphan branch, mounted — [ADR-0129](adrs/0129-project-docs-topology.md)
- **ADRs live with the repo that owns the decision.** — [ADR-0129](adrs/0129-project-docs-topology.md)
- **Ports, kernel, composition, engines, tooling, runtime** are — [ADR-0129](adrs/0129-project-docs-topology.md)
- **Repo-local decisions** (e.g. 0113 WebIDL/JS, 0116 .hfc, 0109 — [ADR-0129](adrs/0129-project-docs-topology.md)
- **Cross-cutting ADRs are split.** The original number stays with the — [ADR-0129](adrs/0129-project-docs-topology.md)
- **project/adrs/INDEX.md is the global ADR registry** — — [ADR-0129](adrs/0129-project-docs-topology.md)
- **principles.md is hosted once, in the docs/ repo**, and linked from — [ADR-0129](adrs/0129-project-docs-topology.md)
- **History is preserved with git filter-repo --paths-from-file**, one — [ADR-0129](adrs/0129-project-docs-topology.md)
- **platform adopts the worktree model too**, and its project branch — [ADR-0129](adrs/0129-project-docs-topology.md)
- **Until docs/ exists as a repo, platform is the hub.** Each repo's — [ADR-0129](adrs/0129-project-docs-topology.md)

### Lower the `while` state fold to `Loop` + explicit protocol ops

- **current** = st == null ? in : st, expressed as — [ADR-0147](adrs/0147-lowering-state-fold-while.md)
- The step arrow applies to current destructured **by param name** — [ADR-0147](adrs/0147-lowering-state-fold-while.md)
- A new loop_outcome op maps the step result: {state: v} → right(v) — [ADR-0147](adrs/0147-lowering-state-fold-while.md)

### Lower `call`/`dimension` application to `App` terms

- **var callee bound in the compile-time env** — substitute the bound — [ADR-0148](adrs/0148-lower-call-application.md)
- **var callee bound as a parameter** — project it out of the input — [ADR-0148](adrs/0148-lower-call-application.md)
- **var callee unknown to the env** — Op{name, args}: a native-op — [ADR-0148](adrs/0148-lower-call-application.md)
- **call-face callee** (a nested call/dimension, per the ADR-0123 — [ADR-0148](adrs/0148-lower-call-application.md)
- **any other callee** — sequential Apps, preserving curried — [ADR-0148](adrs/0148-lower-call-application.md)

### `host-cli` is a dumb host; `ext-*` extensions load dynamically

- **The host knows nothing about tools.** host-cli exposes exactly one — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- **Extensions are dynamic.** An extension is a shared library — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- **The ABI is JSON over a C boundary.** Three exported symbols per — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- handl_ext_manifest() -> char* — {"name","version","commands":[{name,usage}]} — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- handl_ext_dispatch(cmd: *const c_char, input: *const c_char) -> char* — — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- handl_ext_free(char*) — extension-owned strings freed by the — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- **One repo per extension** (ext-fmt, ext-lint, ext-typecheck, — [ADR-0149](adrs/0149-dynamic-extension-host.md)
- **Discovery**: --ext-dir <dir> flag, else $HANDL_EXT_DIR, else — [ADR-0149](adrs/0149-dynamic-extension-host.md)

### Repo visibility is derived from the dep closure, not chosen per-repo

- **Edges point private → public.** An adapter or tool binding lives — [ADR-0150](adrs/0150-visibility-closure-model.md)
- **The closure is checked, not remembered.** — [ADR-0150](adrs/0150-visibility-closure-model.md)
- **handl-kit-prelude-lang** (public, new repo kit-prelude-lang) — — [ADR-0150](adrs/0150-visibility-closure-model.md)
- **handl-kit-data-prelude** (private) — the composition wrapper: — [ADR-0150](adrs/0150-visibility-closure-model.md)

### Exec erases type annotations — signatures are typecheck-owned metadata

- **Signature exports erase to null.** is_signature_rhs(rhs) — [ADR-0151](adrs/0151-exec-erases-type-annotations.md)
- **The mirror stays in lockstep.** The transform is applied to every — [ADR-0151](adrs/0151-exec-erases-type-annotations.md)
- **eval.type-annot evaluates the subject only.** The canonical — [ADR-0151](adrs/0151-exec-erases-type-annotations.md)

### `~[n]~>` is a dimension index — n-cells as one indexed former

- The semiring-grade reading of ~[R]~> (ADR-0106 §10, — [ADR-0153](adrs/0153-grades-as-arrow-indices.md)
- sort/grade/op declaration forms remain, but their graded-arrow — [ADR-0153](adrs/0153-grades-as-arrow-indices.md)

### Typecheck semantics — identity, equality, universes, literals, erasure

- values carry a type link → their type's sym; — [ADR-0154](adrs/0154-typecheck-semantics.md)
- types carry a parent link → their supertype sym; — [ADR-0154](adrs/0154-typecheck-semantics.md)
- x : T reads x.type; T <: U walks parent; instanceof = — [ADR-0154](adrs/0154-typecheck-semantics.md)
- Type is an indexed former: Type(i) : Type(i+1); bare Type = — [ADR-0154](adrs/0154-typecheck-semantics.md)
- Pi's level is max(domain, codomain) — the existing — [ADR-0154](adrs/0154-typecheck-semantics.md)
- Levels infer like other indices — invisible in ordinary code; only — [ADR-0154](adrs/0154-typecheck-semantics.md)
- Elaboration consumes all type-level content — metas solved, indices — [ADR-0154](adrs/0154-typecheck-semantics.md)
- **Invariant**: the final runnable artifact contains only 1-arrows + — [ADR-0154](adrs/0154-typecheck-semantics.md)
- Runtime-minted cells (eval/resolve constructing a law or type at — [ADR-0154](adrs/0154-typecheck-semantics.md)

### Scope-Bound Arrow Environments

- **A dependency binding is a callable value** — one payload kind, **arrows**: — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)
- **Name resolution does not change per engine.** proj_env's scope-first — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)
- **ModuleEnv (ADR-0156) becomes the builder** of the lexical environment — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)
- **Bootstrap is unchanged**: HandlLoader::new builds the loader raw over — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)
- **Caching keys on (module_id, env_id)**: minted arrows capture the env, so — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)
- **Staged plan**: callv node + wrap helpers → engine switch → cache keys, — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)
- **Rejected: TaggedValue::NativeFn(Rc<NativeFnHandle>)** — a second callable — [ADR-0155](adrs/0155-scope-bound-arrow-environments.md)

### Module environments are parameters, not construction state

- ModuleEnv owns `{ dependencies: Vec<Arc<dyn Module>>, — [ADR-0156](adrs/0156-module-env-as-parameter.md)
- instantiate produces a sibling module sharing the prepared source, — [ADR-0156](adrs/0156-module-env-as-parameter.md)
- with_dependency* / with_global_lookup / with_import_resolver — [ADR-0156](adrs/0156-module-env-as-parameter.md)
- mod.fn keeps dispatching through ModuleLookup — dep *functions* — [ADR-0156](adrs/0156-module-env-as-parameter.md)

### Optimizer domains dissolve into `ext` tools, on a trigger

- **Domains stay labs for now.** opt-graph and opt-kernel keep their — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- **Dissolve when all three triggers hold:** — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- a **caller that is not the researcher** (CI job, product, another — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- the domain's public surface has been **unchanged for four weeks**; — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- the **referee is shared** (bench/benchkit usable by any domain), — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- **When triggered, wrap — do not rewrite.** The Python core is exposed — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- **Naming:** an opt-* family (opt-graph, opt-kernel). A third — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)
- **The generic core stays a library.** egraph, discovery, eqsat — [ADR-0159](adrs/0159-opt-domains-dissolve-into-ext-tools.md)

### `lib-*` repos are language packages, not crates

- **Repo shape is fixed** (all 113): — [ADR-0160](adrs/0160-lib-repos-are-language-packages.md)
- **License is MIT only**, one LICENSE file. LICENSE-APACHE/LICENSE-MIT — [ADR-0160](adrs/0160-lib-repos-are-language-packages.md)
- **CI is bupha-only**: root ci.sh (the repo's gate) + a committed — [ADR-0160](adrs/0160-lib-repos-are-language-packages.md)
- **The shell is a bug, not a feature.** It is removed in the same change that — [ADR-0160](adrs/0160-lib-repos-are-language-packages.md)
- **Consumers move to packages**: engine-test (and any other Rust consumer) — [ADR-0160](adrs/0160-lib-repos-are-language-packages.md)

### An ADR is a one-page decision record

- An ADR is **at most ~8 KB and ten numbered rules**. Algorithms, tables, — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- ## Why is mandatory: the failure the decision prevents, in ≤8 lines — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- ## Not doing is mandatory: each rejected alternative on one line with its — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- ## Enforcement is mandatory: the file or test where the decision is true — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- The status vocabulary is closed and lives in the frontmatter (ADR-0163): — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- A dead ADR keeps its **number** as a tombstone row (successor + one-line — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- Authority is single. When a decision replaces another, the replacement — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- project/DECISIONS.md is generated from ## Decision sections — one line — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)
- scripts/check-adrs.py gates 1–7. Report-only for the legacy corpus; — [ADR-0161](adrs/0161-adr-is-a-one-page-decision.md)

### `project/` holds process artifacts; published docs live in the org `docs/`

- **project/ holds process artifacts only.** Explanations, tutorials, — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **The top-level set is closed**, identical in every repo: — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **One artifact answers one question.** A document that answers two is — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **Dated vs living.** notes/, reports/, retros/ are dated artifacts and — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **design/** is either named after the ADR it elaborates (NNNN-slug.md) — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **A rejected design produces an ADR, never a note.** notes/ is a — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **No copies across repos.** Cross-repo references are ids + URLs — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- **One project/ per repo, on its project branch** (ADR-0129 §1), mounted — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- scripts/check-project-layout.py enforces 1, 2, 7 and 8; — [ADR-0162](adrs/0162-project-folder-taxonomy.md)
- project/README.md states this map in one screen and links this ADR — [ADR-0162](adrs/0162-project-folder-taxonomy.md)

### Every `project/` artifact declares its metadata in YAML frontmatter

- Every Markdown artifact under project/ opens with a YAML header. Three — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- type names the ADR-0162 taxonomy entry that owns the file — adr, — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- date is ISO-8601 (YYYY-MM-DD, or YYYY-MM when only the month is — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- ADRs carry status from the closed vocabulary `proposed | accepted | — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- A supersession is structured, never prose: status: superseded plus — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- **Status lives only in the header.** ADRs and RFCs use status (the enum in — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- refs is a list of ADR ids the document elaborates or relies on — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- The body never carries status. Template bullets (- Status:, - Date:, — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- title is the H1 without its identifier prefix (ADR-NNNN: , — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)
- scripts/project-frontmatter.py enforces the schema (--write migrates), — [ADR-0163](adrs/0163-project-artifact-frontmatter.md)

### The kernel IR is a `Term` plus kit-declared ops

- A kernel body is a **Term**: an op name applied to arguments, where — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)
- The **op set is closed at nine** — tile_load, unpack_i4, dequant, — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)
- Every op declares four things: a **signature**, a **grade** — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)
- Two terms are equal iff their **denotations** are equal, checked by — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)
- The IR carries **no kernel combinators**. It enters the platform as a — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)
- Cost is a **hard bound only** — roofline floor and legality pruning — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)
- The denotation is the *meaning*, not the *schedule*. Two equal terms — [ADR-0165](adrs/0165-kernel-ir-is-term-plus-ops.md)

### The IR gains `pipeline(depth)`, and the schedule programme stops there

- pipeline(depth) is the **tenth op**. It denotes the **identity** and — [ADR-0166](adrs/0166-pipeline-depth-and-the-end-of-the-schedule-programme.md)
- It is priced at **zero** (no extra traffic, no launch). The cost model — [ADR-0166](adrs/0166-pipeline-depth-and-the-end-of-the-schedule-programme.md)
- stage_shared is **not** added. num_stages already stages loads — [ADR-0166](adrs/0166-pipeline-depth-and-the-end-of-the-schedule-programme.md)
- **The schedule-vocabulary programme is closed.** Measured: depth — [ADR-0166](adrs/0166-pipeline-depth-and-the-end-of-the-schedule-programme.md)
- The 47 % → 86 % gap to Marlin is therefore **not load latency**. It is — [ADR-0166](adrs/0166-pipeline-depth-and-the-end-of-the-schedule-programme.md)
