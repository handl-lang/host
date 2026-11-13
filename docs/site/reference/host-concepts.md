# Host concepts

This reference describes the core types, traits, and components that make up a composable host. It is intended to be read while working with the `handl-platform` API — see the rustdoc for exact signatures.

## Traits

| Trait | Role |
| --- | --- |
| `Plugin` | The universal, object-safe base trait. Every loaded component implements this (`id`/`name`/`version`/`capabilities`/`requires`/`optional`/`metadata`/`contribute`/`run`). |
| `RuleAcceptor` | A sub-trait of `Plugin` for components that accept contributions. `RuleAcceptor::Rule` is the associated contribution type. |
| `Contribution` | Base trait for contributed rules — `OptionSchema`, `Box<dyn FormatBehavior>`, `Arc<dyn EvalBehavior>`, etc. |
| `Operation` | A marker trait (with an associated `Request` type) for a cross-cutting concern such as `Format`, `Lint`, `Build`, `Compile`, or `Eval`. |
| `Behavior<Op>` | A rule that contributes to a specific `Operation` for a specific `AstNode.kind`. |
| `CallableLookup` | Read-only view of the plugin and kit registries, used by `Vm`. |

## Core types

| Type | Description |
| --- | --- |
| `DefaultHost` | The concrete composer: owns a `PluginRegistry` and `KitRegistry`; `load()` runs the contribution pass, `run()` dispatches a `CapabilityRequest`, `resolve_composition()` validates against the manifest resolver. |
| `PluginRegistry` | Stores registered plugins and resolves capabilities to their provider. |
| `KitRegistry` | Stores loaded `Kit`s by name. |
| `ContributionContext` | Passed to `Plugin::contribute`. Provides `register_plugin`, `contribute::<P: RuleAcceptor>(rule)` (deferred if the acceptor is not yet registered), `contribute_exec_control`, `run`, `exclude_plugin`, and registry lookups. |
| `PluginContext` | Passed to `Plugin::run`. Gives the plugin mutable access to the plugin and kit registries while serving a capability. |
| `CapabilityRequest` | A request to execute a capability: a `CapabilityId` plus a `serde_json::Value` payload. |
| `CapabilityResponse` | The result of executing a capability (a `serde_json::Value` wrapper). |
| `CapabilityId` | The string identifier for a capability. |
| `AstNode` / `Span` | The generic AST node (`kind`, `span`, `payload`) that `Behavior`s dispatch on. |
| `Kit` / `KitManifest` | A curated composition: manifest (`id`, `name`, `plugins`, `target`, `selectors`) plus the plugin manifests available for resolution. |
| `PluginManifest` | The manifest describing a plugin's provides/requires/extends/conflicts/replaces/targets/exclusive declarations. |
| `HostConfig` | In-memory description of kits to load, plugins to register, excludes, option overrides, and `[dependencies]`/`[patch]` tables; parsed from `.handl.toml` by `HostConfig::from_toml`. |
| `KitReference` / `PluginReference` / `DependencySpec` | The per-entry types inside `HostConfig`. |
| `OptionSchema` / `OptionScope` | A plugin-contributed option definition with id, scope, default, and allowed values. |
| `ResolvedOptions` | The merged option values (`OptionsPlugin::resolve`) queryable via `get`/`choice`/`is_enabled`. |
| `OptionsPlugin` | The `RuleAcceptor` for `OptionSchema` rules; serves the `option/set` capability. |
| `InProcessPlugin` / `ProcessPlugin` | Built-in `Plugin` impls: closure-backed and out-of-process placeholders. |
| `Vm` | A composable wrapper pairing a `CallableLookup` host with an environment. |

## Plugin lifecycle

1. **Register** — plugins are added to `DefaultHost` (`register_plugin`,
   `register_kit`, `register_process_plugin`) directly, from kits, or
   from another plugin's `contribute` via `ContributionContext`.
2. **Contribute** — `DefaultHost::load` makes a single pass over the
   plugins in registration order (plugins registered during the pass are
   visited later in the same pass) and calls each plugin's `contribute`
   exactly once. Rule contributions whose `RuleAcceptor` is not yet
   registered are **deferred** and retried as loading progresses; a rule
   whose acceptor never appears fails `load` with `MissingRuleAcceptor`.
3. **Excludes** — `ContributionContext::exclude_plugin` schedules plugin
   ids for removal after the pass.
4. **Run** — `DefaultHost::run` dispatches a `CapabilityRequest` to the
   plugin that provides the requested capability.

Configuration (`.handl.toml`) is applied by the applier plugins in
`plugin-applier-*`, not by `DefaultHost`: `DefaultApplierPlugin`'s
`contribute` loads configured kits through `kit/load`, plugin manifests
through `load`, `[options]` through `option/set`, and excludes through
`exclude_plugin`.

## Operation categories

`handl-platform` ships the built-in operation markers and their `Basic*`
acceptors; each `handl-plugin-ext-*` crate is the published capability
surface that wires a contract into the capability system. The engines
behind the contracts live in the `engine-*` repos.

| Operation | Marker | `RuleAcceptor` | Notes |
| --- | --- | --- | --- |
| `Format` | `handl_platform::Format` | `BasicFormatter` accepts `Box<dyn FormatBehavior>` | canonical formatter is `handl-plugin-ext-fmt` / `hnl-engine-fmt` |
| `Lint` | `handl_platform::Lint` | `BasicLinter` accepts `Box<dyn LintBehavior>` | canonical linter is `handl-plugin-ext-lint` / `hnl-engine-lint` |
| `Build` | `handl_platform::Build` | `BasicBuilder` accepts `Box<dyn BuildBehavior>` | |
| `Compile` | `handl_platform::Compile` | `BasicCompiler` accepts `Box<dyn CompileBehavior>` | targets live in the `compile-*` repos |
| `Eval` | `handl_platform::Eval` | `BasicEvaluator` accepts `Arc<dyn EvalBehavior>` (+ `contribute_exec_control`) | engines in `engine-exec*` |
| `Load` | `plugin-load` | — | plugin-manifest loading capability |
| `Refine` | `plugin-ext-refinement` | — | refinement/type-inference surface |

Toolchain entry points such as `fmt`, `lint`, `typecheck`, `test`, and
`compile` are **capabilities** served by `handl-plugin-ext-*` plugins
(`FmtPlugin`, `LintPlugin`, `TypecheckPlugin`, `TestPlugin`, …) — not
`Operation` markers. The lex/parse/transform engine abstractions are
`hnl-engine-lex`, `hnl-engine-parse`, and `hnl-engine-transform`.

## Common plugin kinds

| Kind | What it does in `contribute` |
| --- | --- |
| Library plugin | Calls `ContributionContext::register_plugin` to load more plugins. |
| Target plugin | Registers target-specific plugins and contributes `OptionSchema`s. |
| Rule plugin | Calls `ContributionContext::contribute::<P>(rule)` to add a rule to an existing `RuleAcceptor`. |
| Operation plugin | Accepts rules and executes them when a capability is requested. |
| Executable host | Registers plugins/kits, applies config (usually via an applier plugin), calls `load`, then `run` for each capability request. |
