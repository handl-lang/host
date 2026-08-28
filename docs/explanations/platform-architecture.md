---
title: Platform architecture
---

# Platform architecture

Handl is built around a single, composable platform layer. Every tool, language stage, and runtime backend is a `Plugin`, and every executable is a `Host` that loads and composes plugins.

## The layers

- **Platform** — the shared contracts in `handl-platform`. It defines `Plugin`, `RuleAcceptor`, `Operation`, `Behavior`, `Contribution`, `ContributionContext`, `Host`, `HostConfig`, `HostApplier`, `Options`, `AstNode`, `DataTypes`, and the loading algorithm.
- **Operation** — a cross-cutting concern. Each `handl-ext-*` crate defines an `Operation` marker and a default `RuleAcceptor` plugin such as `BasicFormatter`, `BasicLinter`, `BasicLexer`, `BasicParser`, `BasicTypechecker`, `BasicCli`, `BasicLsp`, `BasicDoc`, or `BasicHover`.
- **Plugin** — the universal unit. A plugin can be a library, a target, an operation, a rule, or an executable contribution.
- **Rule** — a `Behavior<Op>` contributed to a `RuleAcceptor` for a specific `AstNode.kind`.
- **Kit** — a curated, version-pinned manifest of plugins and default option values. A kit is not executable.
- **Host** — a composer that loads kits, resolves dependencies, runs `contribute` to a fixed point, and executes capabilities. `handl-host-cli` and `handl-host-lsp` are executable hosts.
- **Target** — a runtime backend. `handl-target-*` crates register target-specific operations and options.

## Plugins are the only unit

`handl-platform` does not distinguish between a component, an extension, a library, a target, and a capability. The only real distinction is whether a plugin implements `RuleAcceptor` and, if so, what rule it accepts. `handl-plugin-*` crates contribute rules; `handl-kit-*` crates curate sets of plugins; `handl-host-*` crates run them.

## How a new plugin becomes a command

A rule plugin can contribute a `CliCommand` to the `Cli` `RuleAcceptor`. The `handl-ext-cli` crate defines the `Cli` operation and the `BasicCli` plugin that accepts those commands. The `handl-host-cli` host loads the kit, runs `contribute`, and then executes the `cli` capability.

```rust
// handl-plugin-lint
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

`handl-host-cli` does not know `lint` exists. The `LintPlugin` contributed the command and the rule, and the host ran them.

## Why this matters

- The platform layer is the only shared contract, so dependency direction is always clear.
- New operations and commands can be added in new `handl-ext-*` or `handl-plugin-*` crates without changing `handl-platform`.
- Hosts can be tiny: the CLI is a thin executable host that loads a kit and runs `Cli`.
- Tools share the same composition model, from the CLI to the LSP server to the test runner.
