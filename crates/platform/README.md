# handl-platform

Core platform abstractions for the Handl language: capabilities, plugins, kits, hosts, configuration, and boundaries.

Capabilities are identified by `CapabilityId` and carry generic JSON-serializable `CapabilityMetadata` (`serde_json::Value`). No CLI-specific fields are defined here; hosts such as the CLI, LSP, or web UI may interpret the metadata independently.

This crate intentionally does not include concrete built-in capabilities (e.g. `fmt`, `lint`), middleware, RPC, or kit-as-product features yet. Those are defined in separate `handl-plugin-ext-*` / `handl-plugin-lib-*` crates or deferred to later phases.

## Modules

- `capability` — `CapabilityId`, `CapabilityMetadata`, and the `Capability` trait.
- `plugin` — `Plugin`, `PluginRegistry`, `InProcessPlugin`, `ProcessPlugin`, `CapabilityRequest`, `CapabilityResponse`, `PluginContext`.
- `plugin_model` — `Contribution`, `RuleAcceptor`, `Operation`, `Behavior`, `ContributionContext` — the composable contribution model every plugin kind is built on.
- `host` — `DefaultHost` (the concrete composer) and `CallableLookup`.
- `kit` — `Kit`, `KitManifest`, and `KitRegistry`.
- `manifest` — `PluginManifest` and the dependency declarations used by composition resolution.
- `resolve` — manifest-level composition resolution (`resolve`, `ResolvedComposition`).
- `config` — `HostConfig` (`.handl.toml` parsing via `HostConfig::from_toml`), `KitReference` / `PluginReference` / `DependencySpec`, and dependency resolution (`resolve_dependencies`, `[patch]` overrides, git fetch-on-miss).
- `options` — `OptionSchema`, `OptionScope`, `ResolvedOptions`, and `OptionsPlugin` (the `option/set` capability).
- `format`, `lint`, `build`, `compile`, `eval` — the built-in operation markers (`Format`, `Lint`, `Build`, `Compile`, `Eval`), their `*Request` / `*Behavior` traits, and the `Basic*` `RuleAcceptor` plugins that collect contributed rules.
- `ast_node` — `AstNode` / `Span`, the generic AST value type that `Behavior`s dispatch on.
- `boundary` — `Boundary` / `BoundaryKind` for runtime isolation boundaries.
- `vm` — `Vm`, a composable wrapper pairing a `CallableLookup` host with an environment.
- `error` — `PlatformError` with `thiserror`.
