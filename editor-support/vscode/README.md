# handl

Language support for [handl](https://handl-lang.org) — a language for authoring computation as arrows with refinement types.

## Features

- **Syntax highlighting** — provided entirely by the LSP server via semantic tokens for `.hnl` files (no TextMate grammar)
- **Refinement inference** — hover over any arrow file to see its inferred input/output types and guarantee predicate
- **Per-property hover** — hover over `state.acc` or `state.remaining` to see the inferred type for that specific field
- **Cross-arrow hover** — hover over an arrow reference (e.g. `insert` in `sort.hnl`) to see that arrow's doc comments and inferred contract
- **Doc comment hover** — `///` doc comments at the top of a file are shown when hovering over `loop` or `~>`
- **Signature help** — parameter hints for arrow combinators (`arr`, `comp`, `first`, `choice`, etc.)
- **Go to definition** — jump to arrow files and axiom definitions across the workspace
- **Document symbols** — outline view of arrows, axioms, and bindings in a file
- **Workspace symbols** — `Ctrl+T` to search for arrows and axioms across all files
- **Formatting** — auto-indent, colon alignment, and trailing whitespace cleanup on save
- **Inlay hints** — inferred refinement types shown inline next to loop state fields
- **Rename** — rename arrows across all files, including renaming the file itself
- **Folding ranges** — collapse `loop`, `match`, and object blocks
- **Document highlight** — highlight all occurrences of a symbol under the cursor
- **Selection ranges** — `Shift+Alt+→` smart selection expansion (bracket-aware)
- **Code actions** — quick fixes: add missing `returnA`, wrap in `do`, convert `//` to `///`
- **Context-aware completion** — keyword snippets, field names, combinators, and match patterns
- **Snippets** — templates for `loop`, `match`, `do`, `returnA`, `arr`, `seq`, `choice`, `axiom`, and more

## Installation

```bash
cd editor-support/vscode
npm install
npm run package
code --install-extension handl-*.vsix --force
```

The extension looks for the LSP server binary at `<workspace>/target/debug/handl-host-lsp`. Build it with:

```bash
cargo build -p handl-host-lsp
```

You can also set `handl.server.path` in VS Code settings to point to a custom binary location.

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `handl.server.path` | `""` | Path to the `handl-host-lsp` binary |
| `handl.enableInlayHints` | `true` | Show inferred refinement types as inlay hints |
| `handl.enableCodeLens` | `true` | Show inferred contract summaries as code lenses |
| `handl.enableRefinementDiagnostics` | `true` | Show refinement inference errors as diagnostics |
| `handl.enableSemanticHighlighting` | `true` | Enable semantic token-based syntax highlighting |

## Development

```bash
npm run dev    # build, package, and install
```

After making changes, reload the VS Code window (`Ctrl+Shift+P` → "Developer: Reload Window").
