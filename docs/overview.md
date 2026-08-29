# Platform overview

handl is a **composable language platform**. A single `Plugin` trait unifies libraries, targets, operations, and rules. A `Host` loads plugins from one or more kits, resolves their dependencies, runs their `contribute` methods to a fixed point, and then executes capabilities on demand.

## Core concepts

```
Host                       # empty composer
  └── PluginRegistry       # every loaded thing is a Plugin
        ├── Library        # registers more plugins
        ├── Target         # contributes backends and target options
        ├── Operation      # accepts rules for a cross-cutting concern
        ├── Rule           # contributes Behavior<Op> to an Operation
        └── Options        # resolves option values from schemas
```

| Concept              | What it is                                                                                   |
| -------------------- | -------------------------------------------------------------------------------------------- |
| `Plugin`             | The universal, object-safe trait. Every loaded component implements it.                      |
| `RuleAcceptor<Rule>` | A sub-trait for plugins that accept contributions.                                           |
| `Operation`          | A marker type for a cross-cutting concern such as `Format`, `Lint`, `Eval`, `Cli`, or `Lsp`. |
| `Behavior<Op>`       | A rule that a plugin contributes to a specific `Operation` for a specific `AstNode` kind.    |
| `Kit`                | A curated manifest of plugins and default option values.                                     |
| `HostConfig`         | In-memory description of kits, direct plugins, excludes, and option overrides.               |
| `HostApplier`        | Reads `.handl.toml`, CLI flags, and env and applies them to a `Host`.                        |

## What a plugin can be

- **Library plugin** — registers more plugins, such as `StringTypePlugin` or `ConcatPlugin`.
- **Target plugin** — adds a runtime backend, such as `native`, `wasm`, or `js-native`.
- **Operation plugin** — accepts rules for `Format`, `Lint`, `Parser`, `Eval`, `Cli`, `Lsp`, and so on.
- **Rule plugin** — contributes `Behavior` rules to existing operations, such as a formatter rule for a new AST node kind.
- **Options plugin** — resolves option values from contributed schemas.

## Host loading flow

1. Create a `Host`.
2. Use a `HostApplier` to read `.handl.toml` and apply a `HostConfig`.
3. Call `Host::load` to resolve dependencies and run `contribute` to a fixed point.
4. During `contribute`, plugins register more plugins, contribute rules, and define options.
5. Call `Host::execute` to run a capability.

For the full design, read [ADR-0002: Single Plugin Model](../project/adrs/0002-handl-platform-composable-host). For hands-on instructions, see the [Build a host tutorial](../tutorials/build-a-host).
