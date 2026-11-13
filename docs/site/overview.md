# Platform overview

handl is a **composable language platform**. A single `Plugin` trait unifies libraries, targets, operations, and rules. A `DefaultHost` registers plugins from one or more kits, runs `load` — a single registration-order `contribute` pass with deferred-rule retries — and then executes capabilities on demand through `run`.

This page covers the published platform layers. Parts of the language surface are public (the `engine-fmt` / `engine-lint` / `engine-typecheck` engines and a few `lib-*` packages), but the `hnl` toolchain and most of the standard library are not yet released.

## Core concepts

```
DefaultHost                 # empty composer
  ├── PluginRegistry        # every loaded thing is a Plugin
  │     ├── Library         # registers more plugins
  │     ├── Target          # contributes backends and target options
  │     ├── Operation       # accepts rules for a cross-cutting concern
  │     ├── Rule            # contributes Behavior<Op> to an Operation
  │     └── Options         # resolves option values from schemas
  └── KitRegistry           # loaded Kit manifests
```

| Concept              | What it is                                                                                   |
| -------------------- | -------------------------------------------------------------------------------------------- |
| `Plugin`             | The universal, object-safe trait. Every loaded component implements it.                      |
| `RuleAcceptor<Rule>` | A sub-trait for plugins that accept contributions.                                           |
| `Operation`          | A marker type (with a `Request` type) for a cross-cutting concern such as `Format`, `Lint`, `Build`, `Compile`, or `Eval`. |
| `Behavior<Op>`       | A rule that a plugin contributes to a specific `Operation` for a specific `AstNode` kind.    |
| `Kit` / `KitManifest`| A curated manifest of plugins (and optional target / capability selectors).                  |
| `HostConfig`         | In-memory description of kits, direct plugins, excludes, option overrides, and dependencies; parsed from `.handl.toml` by `HostConfig::from_toml`. |
| `ApplierFactory` / `DefaultApplierPlugin` | The `plugin-applier-*` seam that reads `.handl.toml` and applies a `HostConfig` through capabilities. |

## What a plugin can be

- **Library plugin** — registers more plugins, such as `StringTypePlugin` or `ConcatPlugin`.
- **Target plugin** — adds a runtime backend, such as `native`, `wasm`, or `js-native`.
- **Operation plugin** — accepts rules for `Format`, `Lint`, `Build`, `Compile`, `Eval`, and so on.
- **Rule plugin** — contributes `Behavior` rules to existing operations, such as a formatter rule for a new AST node kind.
- **Options plugin** — `OptionsPlugin` resolves option values from contributed `OptionSchema`s.

## Host loading flow

1. Create a `DefaultHost`.
2. Register plugins and kits — directly, or via an applier plugin that reads `.handl.toml` into a `HostConfig` (`plugin-applier-*`).
3. Call `load()` to run the single registration-order `contribute` pass; deferred rule contributions are retried as their acceptors appear.
4. During `contribute`, plugins register more plugins, contribute rules, and define options.
5. Call `run()` with a `CapabilityRequest` to execute a capability.

## The object/arrow kernel

The language core is a single semantic world: `Obj : sym()`, `Type : Obj := sym()`, and the arrow `A ~> B` as the only computational primitive. The VM primitives `sym()` / `sym(A)` create fresh nominal identity (with an optional parent), and the same `sym` parent graph powers runtime `instanceof`, static subtyping `<:`, and polymorphism — no separate subtype, interface, trait, or impl systems. Types, grades, proofs, algebras, and capabilities are ordinary objects; generics, dependent types, and refinements ride on one dimension form, `f<...>`; composition is `>>>` (do-blocks lower to it); state is explicit data (`State ~> (State, Result)`), so programs are serializable arrow graphs with no closures or mutable bindings.

For the full kernel design, read [ADR-0106: The object/arrow kernel](https://github.com/handl-lang/host/blob/project/adrs/0106-object-arrow-kernel.md).

For the full platform design, read [ADR-0002: Single Plugin Model](https://github.com/handl-lang/host/blob/project/adrs/archived/0002-handl-platform-composable-host.md). For hands-on instructions, see the [Build a host tutorial](./tutorials/build-a-host).
