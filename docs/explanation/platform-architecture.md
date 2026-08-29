# Platform architecture

The handl platform is a workspace of Rust crates organised around one goal: every concept in the language is a `Plugin`, and every tool is a `Host` that composes plugins.

## Crate layers

```text
handl-host-cli, handl-host-lsp     # executable hosts
handl-ext-fmt, handl-ext-lint,    # concrete operations
  handl-ext-lexer, handl-ext-parser,
  handl-ext-type, handl-ext-cli,
  handl-ext-lsp, handl-ext-doc,
  handl-ext-eval
handl-plugin-*                    # rule and library plugins
handl-kit-*                       # curated kit manifests
handl-target-*                    # runtime targets
handl-platform                    # Plugin, Host, and composition primitives
```

| Layer            | Responsibility                                                                                                          |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `handl-platform` | `Host`, `Plugin`, `RuleAcceptor`, `Operation`, `Behavior`, `ContributionContext`, `Options`, `AstNode`, and resolution. |
| `handl-ext-*`    | Concrete operations such as `Format`, `Lint`, `Parser`, `Lexer`, `Typecheck`, `Eval`, `Cli`, `Lsp`, and `Doc`.          |
| `handl-plugin-*` | Rule plugins that contribute `Behavior`s and `OptionSchema`s, and library plugins that register more plugins.           |
| `handl-kit-*`    | Curated manifests of plugins and default option values. A kit is not executable.                                        |
| `handl-target-*` | Target plugins that register runtime backends.                                                                          |
| `handl-host-*`   | Executable hosts that load kits and execute `Cli` or `Lsp` capabilities.                                                |

## Why this split

The platform crate intentionally stays small. It defines the composition primitives and nothing else. Concrete operations, library rules, and targets live in separate crates because they can be added, swapped, or excluded without changing the core. This lets a host be as small as a formatter or as large as a language server.

## The host as composer

A `Host` starts empty and ends as a fully composed tool. It does this by:

1. Loading `HostConfig` from a `HostApplier`.
2. Merging kit manifests into `PluginRegistry`.
3. Resolving `requires` and `optional` dependencies.
4. Running `Plugin::contribute` in topological order.
5. Repeating steps 3–4 until no new plugins appear.
6. Resolving `Options` from merged schemas and overrides.

After this, `Host::execute` dispatches a `CapabilityRequest` to the plugin that provides the requested capability.

## Adding a new operation

To add a new operation such as `Hover`:

1. Create `handl-ext-hover`.
2. Define a `Hover` marker type implementing `Operation`.
3. Define `HoverBehavior` implementing `Behavior<Hover>`.
4. Provide `BasicHover` as a `RuleAcceptor<Rule = HoverBehavior>`.
5. Register `BasicHover` in the host.

No `handl-platform` change is required.
