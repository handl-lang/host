# ADR: LSP Server as a Tooling Plugin

Status: Proposed

## Context

The `handl` architecture is organized as a stack of replaceable, independently-installable layers:

```text
                ┌─────────────────┐
                │   language-kit  │
                │   LEGO pieces   │
                └────────┬────────┘
                         │
                 stable protocols
                         │
                ┌────────▼────────┐
                │      lang       │
                │ CLI / registry  │
                │ orchestration   │
                └────────┬────────┘
                         │
              ┌──────────┼──────────┐
              ↓          ↓          ↓
           compiler    tooling    runtime
           plugins     plugins    plugins
              │          │          │
         native/JS/   fmt/lint/   VM/GC/
         Wasm/etc.    check/LSP   debugger
```

The cardinal constraint we are preserving is:

> **No layer should require the layer above it.**

This means `language-kit` works without `lang`, compiler plugins work without the CLI, and tooling plugins must work without knowing the compiler's concrete implementation or the CLI entry point.

We want an LSP server so that editors (VS Code, Helix, Zed, Emacs, etc.) can get completions, diagnostics, hover, go-to-definition, formatting, and rename support for `handl` source files.

## Decision

1. The LSP server is a **tooling plugin**, not a top-level crate embedded in `lang`.
2. It lives in a new crate/package named `tool-lsp`.
3. `tool-lsp` depends only on:
   - `language-kit` stable protocols (document model, diagnostics, symbol index, formatting requests);
   - the abstract `dialect` and `lexer` traits for parsing and tokenization;
   - the abstract `check` / `lint` tooling traits for diagnostics;
   - the abstract `format` tooling trait for document formatting.
4. `tool-lsp` does **not** depend on `lang`, the CLI, or any concrete compiler implementation.
5. `lang` may discover and launch `tool-lsp` as a sub-process (stdio or TCP), the same way it discovers any other tool plugin.
6. The VS Code extension is a **client only**. It starts or connects to `tool-lsp`; it never reimplements language logic.

## Consequences

- **Editors are decoupled from the compiler.** VS Code, Helix, Zed, Emacs, etc. all talk to the same protocol through `tool-lsp`.
- **The LSP can run standalone.** It does not need the `lang` CLI to function, which makes unit testing and integration into other build systems easier.
- **Compiler and tooling can evolve independently.** As long as the `language-kit` protocols are honored, both sides can change without lock-step releases.
- **One extra crate to maintain.** `tool-lsp` needs its own release cycle, versioning, and protocol compatibility tests.
- **No custom editor code for language features.** All smarts live in the LSP server; clients only handle UI.

## Language plugin contract

For the LSP to support a given module syntax, the concrete language plugin for that syntax must expose the relevant tooling interfaces through `language-kit`. The `tool-lsp` server will ask `language-kit` which plugin is responsible for a given module and then delegate to it:

| LSP feature | Required plugin interface |
| ----------- | ------------------------- |
| Syntax highlighting / tokenization | `Lexer` |
| Diagnostics, type errors | `Check` / `Typecheck` |
| Hover, completion | `Parser` + `Check` |
| Go to definition, references | `SymbolIndex` |
| Formatting | `Format` |
| Rename | `SymbolIndex` + `Refactor` |

If a module uses a syntax whose plugin does not implement one of these interfaces, `tool-lsp` simply returns no results for that feature. The LSP itself stays editor-agnostic and does not need to know the concrete syntax.

## Implementation Notes

- `tool-lsp` should expose a binary (e.g. `handl-lsp`) that implements the LSP spec over stdio by default.
- It should consume a `Workspace` protocol from `language-kit` that provides document text, module resolution, and change notifications.
- It should delegate requests to the appropriate tooling plugins through the `language-kit` traits, never calling concrete compiler plugins directly.
