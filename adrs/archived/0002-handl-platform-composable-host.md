---
title: Single `Plugin` Model for Host, Operations, Libraries, Targets, and Options
type: adr
date: 2026-08-29
status: accepted
status_note: Accepted
---

# ADR-0002: Single `Plugin` Model for Host, Operations, Libraries, Targets, and Options

## Context

`handl-platform` is intended to be the shared, runtime-agnostic kernel for the `handl` language. It currently has several overlapping concepts: `Component`, `Plugin`, `Extension`, `Target`, `Library`, `Kit`, and `Host`. This creates a lot of plumbing and artificial distinctions.

The core insight is that a **library**, a **target**, and an **operation** are all just things the host loads and lets other things plug into. The only real distinction is whether a thing accepts rules from other things.

Limitations of the current design:

1. **Too many concepts.** `Plugin`, `Extension`, `Component`, `Target`, `Library` all overlap.
2. **Host is not a composer.** `DefaultHost` and `LanguageHost` are concrete types with hard-coded fields.
3. **No dependency model for plugins.** A plugin cannot declare that it requires `Lexer` and optionally uses `Formatter`.
4. **Language stages are hard-coded.** `LexerPlugin`, `ParserPlugin`, `FormatterPlugin`, and `DiagnosticsPlugin` are special-cased in `LanguagePipeline`.
5. **AST is not composable.** `SyntaxNode::Custom` exists, but there is no mechanism for a plugin to introduce a new AST node kind.
6. **No data type model.** A plugin cannot register a runtime value type for other plugins to query.
7. **Options are not pluggable.** Formatter/linter options are not introduced by the plugins that own them.

We need an architecture with a single base concept: **Plugin**.

## Decision

Everything the host loads is a `Plugin`. A `Plugin` can:

- be a **library** that registers more plugins
- be a **target** that contributes target-specific operations/options
- be an **operation** that accepts rules from other plugins
- be a **rule** that contributes to an operation
- be **executed** as a capability
- declare required and optional dependencies on other plugins

The `Host` becomes a pure composer with one primary registry:

- `PluginRegistry` — all plugins

It also keeps an `OptionsResolver` (inside the `Options` plugin) for resolved option values.

### Concepts

1. **Plugin**: the universal component. Every loaded thing is a plugin.
2. **RuleAcceptor<Rule>**: a sub-trait for plugins that accept contributions.
3. **Operation**: a marker type for a cross-cutting concern (`Format`, `Lint`, `Lexer`, `Parser`, `Typecheck`, `Doc`, `Cli`, `Lsp`).
4. **Behavior<Op>**: a single rule that a plugin contributes to an operation for a specific `AstNode.kind`.
5. **ContributionContext**: a type-safe API for a plugin to register other plugins and to hand rules to `RuleAcceptor`s.
6. **Kit**: a manifest that lists plugins to load. A host can load zero or more kits.
7. **HostConfig**: the in-memory description of which plugins to load, what to exclude, and which option overrides to apply.
8. **HostApplier**: a trait for reading configuration and applying it to a `Host`. `ConfigHostApplier` is the `.handl.toml` / CLI flags / env implementation.
9. **Library plugin**: a plugin that, in `contribute`, registers more plugins.
10. **Target plugin**: a plugin that, in `contribute`, registers target-specific operations and options.
11. **Options plugin**: a `RuleAcceptor<OptionSchema>` that resolves values from kit/host/user config.

### Common API

```rust
pub trait Contribution: Send + Sync + 'static {
    fn id(&self) -> &str;
}

pub trait Plugin: Send + Sync + 'static + std::any::Any {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn metadata(&self) -> serde_json::Value;
    fn requires(&self) -> &[CapabilityId] { &[] }
    fn optional(&self) -> &[CapabilityId] { &[] }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> { Ok(()) }
    fn run(&self, request: &CapabilityRequest, ctx: &mut PluginContext)
        -> Result<CapabilityResponse, PlatformError>
    { Err(PlatformError::NotImplemented) }
}

pub trait RuleAcceptor: Plugin {
    type Rule: Contribution;
    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError>;
}

pub trait Operation: Send + Sync + 'static {}

pub trait Behavior<Op: Operation>: Send + Sync + 'static {
    fn kind(&self) -> &str;  // which AstNode.kind this behavior handles
    fn apply(&self, op: &mut Op::Request) -> Result<(), PlatformError>;
}

pub struct ContributionContext<'a> {
    host: &'a mut Host,
}

impl<'a> ContributionContext<'a> {
    pub fn register_plugin(&mut self, plugin: impl Plugin + 'static) -> Result<(), PlatformError> { ... }

    pub fn contribute<P: RuleAcceptor>(&mut self, rule: P::Rule) -> Result<(), PlatformError> { ... }
}
```

The `Plugin` trait is object-safe, so the `Host` can store `Box<dyn Plugin>`. `RuleAcceptor` is used only for type-safe contributions and is downcast at contribution time.

### The AST is a type, not a plugin

- `AstNode` is a single, generic, composable value type:

```rust
#[derive(Debug, Clone)]
pub struct AstNode {
    pub kind: String,
    pub span: Option<Span>,
    pub payload: serde_json::Value,
}
```

- There is no `Ast` plugin. `Formatter`, `Linter`, `Typecheck`, `Parser`, `Lexer`, and `Diagnostics` are all `RuleAcceptor` plugins that accept `Behavior<Op>` rules keyed by `AstNode.kind`.
- A plugin introduces a new AST node type by contributing `Behavior<Format>`, `Behavior<Lint>`, `Behavior<Typecheck>`, `Behavior<Parser>`, `Behavior<Lexer>`, and `Behavior<Diagnostics>` all for the same `kind`.

### Data types are a `RuleAcceptor` plugin

- `DataTypes` is a `Plugin + RuleAcceptor<Rule = DataType>`.
- Other plugins, notably `Typecheck`, query `DataTypes` by `id`.

### Targets and libraries are plugins

A `Library` plugin registers more plugins during `contribute`:

```rust
pub struct LibUtilsPlugin;

impl Plugin for LibUtilsPlugin {
    fn id(&self) -> &str { "lib-utils" }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.register_plugin(StringTypePlugin);
        ctx.register_plugin(ListTypePlugin);
        ctx.register_plugin(ConcatPlugin);
        Ok(())
    }
}
```

A `Target` plugin contributes target-specific operations and options:

```rust
pub struct NativeTarget;

impl Plugin for NativeTarget {
    fn id(&self) -> &str { "native" }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<Options>(OptionSchema {
            id: "target".into(),
            default: serde_json::Value::String("native".into()),
            possible_values: vec![
                Value::String("native".into()),
                Value::String("wasm".into()),
            ],
            ...
        })?;
        ctx.register_plugin(NativeCodeGen);
        Ok(())
    }
}
```

### Operations live outside the platform

- `handl-platform` defines `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior`.
- Concrete operations (`Format`, `Lint`, `Lexer`, `Parser`, `Typecheck`, `Diagnostics`, `Doc`, `Cli`, `Lsp`) live in `handl-ext-*` crates.
- Each `handl-ext-*` crate exports an `Operation` marker and a default runtime `Plugin` (`BasicFormatter`, `BasicLinter`, etc.).
- A host can swap the runtime `RuleAcceptor` for the same marker without changing any rule plugin.

### Options are contributed by plugins

- `Options` is a `Plugin + RuleAcceptor<Rule = OptionSchema>`.
- `OptionSchema` declares the option id, scope, default value, allowed values, and the node kind or rule it applies to.
- `ResolvedOptions` is built by merging defaults with kit/host/user config and validating against `possible_values`.
- Every operation `Request` (`FormatRequest`, `LintRequest`, etc.) carries a `&ResolvedOptions`.
- A plugin both defines an option and implements the `Behavior` that reads it. The formatter and linter never know the option exists.

```rust
pub struct OptionSchema {
    pub id: String,
    pub scope: OptionScope,
    pub applies_to: Option<String>,
    pub default: serde_json::Value,
    pub possible_values: Vec<serde_json::Value>,
    pub description: String,
}

pub struct ResolvedOptions {
    values: HashMap<String, serde_json::Value>,
}

impl ResolvedOptions {
    pub fn get(&self, id: &str) -> Option<&Value>;
    pub fn choice(&self, id: &str) -> Option<&str>;
    pub fn is_enabled(&self, rule_id: &str) -> bool;
}
```

### Language pipeline is a host capability, not a hard-coded struct

- `LanguageHost` (or a `Host` with language `RuleAcceptor`s registered) runs `lex`, `parse`, `typecheck`, `lint`, `format`, `diagnose`, and `diagnostics` by executing the corresponding operation plugins.
- Each stage is a `RuleAcceptor` plugin that receives `Behavior` and `OptionSchema` contributions and produces or transforms `AstNode`s.

### Host loading is a fixed point

```rust
impl Host {
    pub fn load(&mut self) -> Result<(), PlatformError> {
        // 1. resolve dependencies among already registered plugins
        // 2. call contribute in dependency order
        // 3. if a plugin registered more plugins, go back to 1
        // 4. once no new plugins appear, resolve options and finalize
        ...
    }
}
```

### CLI and LSP are executable hosts, not kits

- `handl-host-cli` is an executable `Host`.
- A `HostApplier` reads `.handl.toml` and CLI flags and applies the resulting `HostConfig` to the `Host`.
- `HostConfig` can include multiple `Kit`s, list direct plugins, exclude specific plugins, and set option overrides.
- The `Host` merges all included kits, applies excludes, loads direct plugins, resolves options, and runs `contribute` to a fixed point.
- It registers the `Cli` `RuleAcceptor` and any command-line plugins.
- Plugins contribute `CliCommand`s to the `Cli` plugin.
- The `handl-host-cli` `main` builds the host, loads it, and executes `Cli`.

```rust
// handl-plugin-lint
pub struct LintPlugin;

impl Plugin for LintPlugin {
    fn id(&self) -> &str { "lint" }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<Cli>(CliCommand {
            name: "lint",
            about: "Run the linter",
            run: Box::new(|host, matches| {
                host.execute(&"lint".into(), json!({
                    "path": matches.get_one::<String>("path"),
                }))
            }),
        })?;

        ctx.contribute::<Linter>(Box::new(NoUnusedImports))?;
        Ok(())
    }
}
```

```toml
# .handl.toml
[[kit]]
include = "handl-kit-canonical"

[[kit]]
include = "handl-kit-experimental"
exclude = ["handl-plugin-deprecated-grammar"]

[[plugin]]
register = "handl-plugin-custom-linter"

[options]
"comment.style" = "above"
"no-unused-imports.level" = "error"
"experimental.lifetime-check" = false
```

```rust
// handl-host-cli
fn main() -> Result<(), PlatformError> {
    let mut host = Host::new();

    // HostApplier knows about files and CLI flags; Host does not.
    let applier = ConfigHostApplier::new(std::env::args())?;
    applier.apply(&mut host)?;

    host.register_plugin(ClapCli::new());

    host.load()?;

    host.execute(&"cli".into(), json!({"args": std::env::args().collect::<Vec<_>>()}))
}
```

- `handl-host-lsp` is the same pattern: an executable `Host` that loads multiple kits/direct plugins, registers the `Lsp` `RuleAcceptor`, and dispatches LSP requests to language operations.
- This closes the loop: the same composition model produces CLI, LSP, language servers, tests, and REPLs.

## Consequences

### Crate/module architecture

| crate/module                    | responsibility                                                                                                                                             |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `handl-platform`                | `Host`, `Plugin`, `RuleAcceptor`, `Operation`, `Behavior`, `ContributionContext`, `Options`, `AstNode`, `DataTypes`, resolution, manifests                 |
| `handl-ext-fmt`                 | `Format` operation, `FormatBehavior`, `FormatContext`, `BasicFormatter` `RuleAcceptor` plugin                                                              |
| `handl-ext-lint`                | `Lint` operation, `LintBehavior`, `LintContext`, `BasicLinter` `RuleAcceptor` plugin                                                                       |
| `handl-ext-lexer`               | `Lexer` operation, `LexerBehavior`, `BasicLexer` `RuleAcceptor` plugin                                                                                     |
| `handl-ext-parser`              | `Parser` operation, `ParserBehavior`, `BasicParser` `RuleAcceptor` plugin                                                                                  |
| `handl-ext-type`                | `Typecheck` operation, `TypeBehavior`, `BasicTypechecker` `RuleAcceptor` plugin                                                                            |
| `handl-ext-eval`                | `Eval` operation, `EvalBehavior`, `EvalContext`                                                                                                            |
| `handl-target-rust-interpreter` | Default `Eval` interpreter `RuleAcceptor` target plugin                                                                                                    |
| `handl-ext-doc`                 | `Doc` operation, `DocBehavior`, `BasicDoc` `RuleAcceptor` plugin                                                                                           |
| `handl-ext-cli`                 | `Cli` operation, `CliCommand`, `BasicCli` `RuleAcceptor` plugin                                                                                            |
| `handl-ext-lsp`                 | `Lsp` operation, `LspHandler`, `BasicLsp` `RuleAcceptor` plugin                                                                                            |
| `handl-plugin-lib-*`            | `Library` plugins that register the standard library pieces                                                                                                |
| `handl-target-*`                | `Target` plugins that register runtime backends (e.g. `handl-target-wasm`, `handl-target-llvm`, `handl-target-js-native`, `handl-target-rust-interpreter`) |
| `handl-plugin-*`                | rule plugins that contribute `Behavior`s and `OptionSchema`s to operations                                                                                 |
| `handl-kit-*`                   | curated manifest of plugins and default option overrides (not an executable) (e.g. `handl-kit-canonical`, `handl-kit-server`, `handl-kit-sandbox`)         |
| `handl-host-cli`                | executable `Host` that loads multiple kits, applies excludes, and runs `Cli`                                                                               |
| `handl-host-lsp`                | executable `Host` that loads multiple kits, applies excludes, and runs `Lsp`                                                                               |

### Plugins are the only unit

A plugin can be anything:

- `BasicFormatter` — accepts `Behavior<Format>`
- `NoUnusedImports` — contributes `Behavior<Lint>` to `Linter`
- `LibUtilsPlugin` — registers `StringTypePlugin`, `ListTypePlugin`, etc.
- `NativeTarget` — registers `NativeCodeGen` and sets the `target` option
- `Cli` — accepts `CliCommand`
- `Options` — accepts `OptionSchema`
- `DataTypes` — accepts `DataType`

The only distinction is whether it implements `RuleAcceptor` and, if so, what `Rule` it accepts.

### Replaceable and extensible subsystems

- Swapping the formatter implementation requires no rule plugin changes.
- Adding a `Linter`, `Diagnostics`, or `Doc` stage is adding a new `handl-ext-*` crate.
- Adding a new AST node kind is contributing `Behavior`s to existing operation plugins.
- Adding a new operation (e.g. `Hover`, `Rename`) does not change `handl-platform`; it is a new marker + `Behavior<Op>` trait.
- Adding a library is just loading a `Library` plugin.
- Adding a target is just loading a `Target` plugin.

### Plugin-provided options

- The `Formatter` does not know `comment.style` exists.
- The `Linter` does not know `comment.max-length` exists.
- A comment-formatting plugin can introduce both options and the `Behavior`s that use them.
- `Options` plugin validates, resolves, and passes the values; operations only read `ResolvedOptions`.

### Type safety

- `contribute::<Formatter>(Box::new(rule))` is checked at compile time; the rule must match `Formatter::Rule`.
- `contribute::<Options>(OptionSchema { ... })` ensures the schema matches the `Options::Rule` type.
- `RuleAcceptor::Rule` is an associated type, so each `RuleAcceptor` accepts exactly one contribution type.

## Performance

The design is intended to be production-grade for language tooling (CLI, LSP, compiler, formatter). The following techniques keep it fast:

### Startup-time performance

- **Fixed-point `contribute` is startup-only.** `Host::load` resolves dependencies and runs `contribute` once. After load, no new plugins are discovered under normal operation.
- **Topological sorting is cached.** Dependency resolution produces a stable order. Re-loading only re-resolves when `HostConfig` changes.
- **One-time `RuleAcceptor` downcast.** `PluginRegistry` stores `Box<dyn Any>` for each registered plugin. During the first `contribute::<P>(rule)` for a given `P`, the registry downcasts to the concrete `P` and caches the resulting `&mut dyn RuleAcceptor<Rule = P::Rule>` (or `&mut P`) for the lifetime of `Host::load`. Subsequent contributions are `O(1)`.
- **Kit manifests are pre-baked.** `handl-kit-*` crates compile with a manifest. The `Host` does not discover plugins on disk at startup; it registers them from the loaded kits and config.

### Operation-time performance

- **Pre-group `Behavior`s by `AstNode.kind`.** Every `RuleAcceptor` that accepts `Behavior<Op>` builds, during `load`, a `HashMap<String, Vec<Box<dyn Behavior<Op>>>>` from the contributed rules. At runtime, each `AstNode` does one hash lookup for its `kind` and then iterates only the matching `Behavior`s.
- **Precompute dispatch tables.** Operation plugins may implement a `precompute` hook (called after all `contribute`s) to build a `match`-style dispatch table, a trie, or a state machine. For example, `BasicLexer` can build a trie of token patterns; `BasicFormatter` can build a `kind -> FormatterFn` table.
- **Arena allocation for `AstNode`s and rules.** The `Host` and operation plugins can use an arena (e.g. `bumpalo`) for `AstNode`s, `Behavior`s, and intermediate `Request` objects. This avoids per-node allocation during `execute`.
- **Cache `ResolvedOptions`.** `Options::run` (or `Options::resolve`) builds the `ResolvedOptions` map once during `load`. Operation `Request` objects receive a reference to the same `ResolvedOptions` for the entire `execute`.
- **Intern option keys.** For hot `ResolvedOptions::get` calls, the platform can provide an `OptionId` newtype backed by a `usize` index. Rules can look up by index instead of string, eliminating string hashing in the hot path.
- **Parallel `Behavior` execution where safe.** `Lint` and `Typecheck` can run matching `Behavior`s for independent `AstNode`s in parallel because `Behavior` rules are `Send + Sync`. `Formatter` and `Parser` generally remain sequential.
- **Avoid `Box` in the hot loop if desired.** An operation plugin is free to store rules as `Vec<ConcreteRule>` and downcast `Box<dyn Behavior<Op>>` to `ConcreteRule` at `load` time if the plugin wants a fast, static dispatch table.

### LSP / long-running process performance

- **Incremental updates.** The `Host` can keep `load` state and only re-resolve changed plugins. `ResolvedOptions` and `Behavior` dispatch tables are invalidated only when `HostConfig` changes.
- **Operation result caching.** LSP operations like `Hover` or `Rename` can cache results per `AstNode` version. When the AST changes, only affected nodes are re-processed.
- **Lazy operation loading.** The `Host` does not need to instantiate every operation on startup. It can lazily initialize an operation plugin the first time `host.execute` targets it.

## Security

### Threat model

- The `Host` and all loaded plugins run in the **same process**.
- A plugin is a Rust crate or a dynamically loaded module. A malicious or buggy plugin can compromise the entire process if it runs native code.
- The platform does **not** provide process-level sandboxing by default.

### What the design gives us

- **Type-safe rule boundaries.** `contribute::<P>(rule)` ensures the rule matches `P::Rule` at compile time. A `FormatRule` cannot accidentally be contributed to the `Linter`.
- **Explicit dependency declaration.** `requires` and `optional` make the plugin graph visible and auditable. A `Kit` is a closed manifest; the host cannot be surprised by a hidden plugin.
- **Option validation.** `OptionSchema` declares `possible_values`. The `Options` plugin rejects values outside the declared set, preventing silent misconfiguration.
- **Exclusion in `HostConfig`.** A `.handl.toml` can `exclude` specific plugins from a kit. This lets users remove untrusted or experimental plugins without forking the kit.

### What the design does not give us

- **No process isolation.** A plugin that uses `unsafe` or calls arbitrary syscalls can crash or compromise the `Host`. Rust's type system does not prevent this.
- **No automatic capability restrictions.** By default, a `Plugin` can `contribute` to any `RuleAcceptor` and a `Library`/`Target` plugin can `register_plugin`. The `Host` does not know which actions are safe.
- **No plugin signing.** Kits and libraries are trusted by inclusion, not by cryptographic signature.

### Recommended security measures

1. **Capability-limited `ContributionContext`.** The `Host` can hand different `ContributionContext` flavors to different plugins:
   - `Library` and `Target` plugins get `register_plugin`.
   - Rule plugins get `contribute` to public `RuleAcceptor`s only.
   - Untrusted plugins get a read-only or `contribute`-only context.

2. **Trusted kit curation.** `handl-kit-*` crates are reviewed packages. A user `.handl.toml` can only pull from known kit crates or explicitly list direct plugins.

3. **No `unsafe` in canonical plugins.** The `handl-platform` and `handl-ext-*` crates should be `#![forbid(unsafe_code)]` where possible. User plugins are not subject to this, but the platform itself remains safe.

4. **Reproducible, version-pinned kits.** `Kit` manifests pin plugin versions. `handl-host-cli` can refuse to load a kit whose dependency tree does not match the pinned hashes.

> **Out of scope for this plan:** sandboxed `Target`s, signed plugins, and permission manifests are not required for the initial design. They are tracked in `project/BACKLOG.md`.

### Bottom line

The design is **safe for trusted, curated plugin graphs** (the normal Rust package model). It is **not safe for arbitrary untrusted native plugins**. If untrusted third-party plugins are needed, the isolation must come from a sandboxed `Target` or an external sandbox, not from the `Host`.

## Migration plan

1. **Define `Plugin`** as the universal, object-safe base trait.
2. **Define `RuleAcceptor`** as the sub-trait for accepting contributions.
3. **Define `Operation` and `Behavior<Op>`** in `handl-platform`.
4. **Define `AstNode`** as the generic AST value type and remove the monolithic `SyntaxNode` enum from the platform.
5. **Add `Options` plugin** with `RuleAcceptor<Rule = OptionSchema>` and `ResolvedOptions`.
6. **Replace `LanguagePipeline` stage plugins with `RuleAcceptor` plugins**: `Lexer`, `Parser`, `Formatter`, `Typecheck`, `Linter`, `Diagnostics`.
7. **Create `ContributionContext`** with `register_plugin` and `contribute::<P: RuleAcceptor>(rule)`.
8. **Create `Host` and `PluginRegistry`**. The `Host` is an empty composer; `load` is a fixed-point loop of dependency resolution and `contribute`.
9. **Create `HostConfig` data type** and `Host::apply_config` to merge kits, direct plugins, excludes, and options before `load`.
10. **Create `HostApplier` trait** and `ConfigHostApplier` implementation. The `Host` itself never touches the filesystem; `ConfigHostApplier` reads `.handl.toml`, CLI flags, and env and applies them to a `Host`.
11. **Extract concrete operations** into `handl-ext-fmt`, `handl-ext-lint`, `handl-ext-lexer`, `handl-ext-parser`, `handl-ext-type`, `handl-ext-cli`, `handl-ext-lsp`, etc.
12. **Create library plugins** (`handl-plugin-lib-*`) that register standard library pieces.
13. **Create target plugins** (`handl-target-*`) that register runtime backends.
14. **Move canonical language pieces** into `handl-plugin-*` crates and `handl-kit-canonical`, using the new `Plugin` model.
15. **Create `handl-host-cli` and `handl-host-lsp`** as executable `Host`s that load kits and execute `Cli`/`Lsp`.
16. **Delete `LanguagePipeline` concrete stage traits** once all consumers use the new operations.

## Alternatives considered

- **Keep `Component`, `Target`, `Library`, `Plugin`, and `Extension` separate**: rejected because the only real distinction is whether a thing accepts rules. `RuleAcceptor` captures that without five concepts.
- **Keep `LanguagePipeline` with hard-coded stages**: rejected because it cannot be extended or subset by other kits.
- **Make `Ast` a plugin registry**: rejected because the AST should be a single, open value type that multiple subsystems share, not a monolithic plugin.
- **Use per-operation rule traits (`FormatRule`, `LintRule`) instead of generic `Behavior<Op>`**: rejected because it makes adding new operations require platform changes. `Behavior<Op>` lets new operations live in new `handl-ext-*` crates.
- **Put target and library selection in constructor arguments**: rejected because it breaks composability and makes testing harder.
- **Centralize options in `Host` or `KitManifest`**: rejected because plugins would need host changes to add new tunables. Plugin-contributed `OptionSchema`s make options self-contained.

## Implementation status

- Core `Plugin`/`RuleAcceptor`/`Operation`/`Behavior` types landed in `handl-platform`.
- `AstNode`, `OptionsPlugin`, `HostConfig`, and `Host::load` are implemented.
- `LanguagePipeline`, `SyntaxNode`, `LanguageHost`, `Extension`, `CommandProvider`, and `Library` have been removed.
- Legacy `plugin-comments` and `plugin-doc-comments` were removed; comment handling is tokenized by `plugin-token-comment` and formatted by `plugin-fmt-trivia`.

## Related work

- ADR-0001: Generic Node, Trait-Based Capabilities, and Plugin-Driven Canonical Kit
- `crates/platform` current implementation
- `docs/explanations/design-principles.md`
