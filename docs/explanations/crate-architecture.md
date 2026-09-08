# Crate architecture

The workspace is organised around one idea: the platform defines the composition contracts, and everything else is a plugin that fits those contracts.

## Crate layers

```text
handl-host-cli, handl-host-lsp     # executable hosts
handl-ext-fmt, handl-ext-lint,    # concrete operations
  handl-ext-lexer, handl-ext-parser,
  handl-plugin-engine-typecheck, handl-ext-cli,
  handl-ext-lsp, handl-ext-doc,
  handl-ext-eval
handl-plugin-*                     # rule and library plugins
handl-kit-*                        # curated kit manifests
handl-target-*                     # runtime targets
handl-platform                     # Plugin, Host, and composition primitives
```

| Crate/module                    | Responsibility                                                                                                                                              |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `handl-platform`                | `Host`, `Plugin`, `RuleAcceptor`, `Operation`, `Behavior`, `ContributionContext`, `Options`, `AstNode`, `DataTypes`, resolution, manifests.                 |
| `handl-ext-fmt`                 | `Format` operation, `FormatBehavior`, `FormatContext`, `BasicFormatter` `RuleAcceptor` plugin.                                                              |
| `handl-ext-lint`                | `Lint` operation, `LintBehavior`, `LintContext`, `BasicLinter` `RuleAcceptor` plugin.                                                                       |
| `handl-ext-lexer`               | `Lexer` operation, `LexerBehavior`, `BasicLexer` `RuleAcceptor` plugin.                                                                                     |
| `handl-ext-parser`              | `Parser` operation, `ParserBehavior`, `BasicParser` `RuleAcceptor` plugin.                                                                                  |
| `handl-plugin-engine-typecheck` | `Typecheck` operation, `TypecheckEngine` trait, `check_module`; concrete NBE engine in `handl-plugin-engine-typecheck-nbe`.                                  |
| `handl-ext-eval`                | `Eval` operation, `EvalBehavior`, `EvalContext`.                                                                                                            |
| `handl-target-rust-interpreter` | Default `Eval` interpreter `RuleAcceptor` target plugin.                                                                                                    |
| `handl-ext-doc`                 | `Doc` operation, `DocBehavior`, `BasicDoc` `RuleAcceptor` plugin.                                                                                           |
| `handl-ext-cli`                 | `Cli` operation, `CliCommand`, `BasicCli` `RuleAcceptor` plugin.                                                                                            |
| `handl-ext-lsp`                 | `Lsp` operation, `LspHandler`, `BasicLsp` `RuleAcceptor` plugin.                                                                                            |
| `handl-plugin-lib-*`            | `Library` plugins that register the standard library pieces.                                                                                                |
| `handl-target-*`                | `Target` plugins that register runtime backends (e.g. `handl-target-wasm`, `handl-target-llvm`, `handl-target-js-native`, `handl-target-rust-interpreter`). |
| `handl-plugin-*`                | Rule plugins that contribute `Behavior`s and `OptionSchema`s to operations.                                                                                 |
| `handl-kit-*`                   | Curated manifest of plugins and default option overrides (not an executable) (e.g. `handl-kit-canonical`, `handl-kit-server`, `handl-kit-sandbox`).         |
| `handl-host-cli`                | Executable `Host` that loads multiple kits, applies excludes, and runs `Cli`.                                                                               |
| `handl-host-lsp`                | Executable `Host` that loads multiple kits, applies excludes, and runs `Lsp`.                                                                               |

## Why this split

The platform crate stays small and stable. It defines the composition primitives and nothing else. Concrete operations, library rules, and targets live in separate crates because they can be added, swapped, or excluded without changing the core. This lets a host be as small as a formatter or as large as a language server.

A new crate is added only when it represents a distinct dependency or consumer boundary. If two crates are always consumed together, they belong in one crate. If one can be used without the other, they stay split. Every crate boundary should correspond to a real dependency, target, or consumer boundary.
