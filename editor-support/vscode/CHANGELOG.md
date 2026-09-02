# Changelog

All notable changes to the **handl** VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-02

Initial release of the handl extension for VS Code.

### Added

- **Syntax highlighting** — TextMate grammar and semantic tokens for `.hnl` files, covering keywords, operators, strings, numbers, comments, type names, object keys, and property access
- **Refinement hover** — hover over any arrow file to see its inferred input/output types and guarantee predicate
- **Per-property hover** — hover over `state.acc` or `state.remaining` to see the inferred refinement type for that specific loop state field
- **Cross-arrow hover** — hover over an arrow reference (e.g. `insert` in `sort.hnl`) to see that arrow's doc comments and inferred contract
- **Doc comment hover** — `///` doc comments at the top of a file are shown when hovering over `loop` or `proc` keywords
- **Signature help** — parameter hints for arrow combinators (`arr`, `comp`, `first`, `choice`, `app`, `loop`, `id`, `insert`)
- **Go to definition** — jump to arrow files and axiom definitions across the workspace (`Ctrl+Click` or `F12`)
- **Document symbols** — outline view of arrows, axioms, and bindings in a file (`Ctrl+Shift+O`)
- **Workspace symbols** — search for arrows and axioms across all `.hnl` files (`Ctrl+T`)
- **Formatting** — auto-indent with 2-space tabs, colon alignment, trailing whitespace cleanup, and final newline enforcement (format on save enabled by default)
- **Inlay hints** — inferred refinement types shown inline next to loop state fields and overall `In ~> Out` signatures
- **Rename** — rename arrows across all files in the workspace, including renaming the file itself (`F2`)
- **Folding ranges** — collapse `loop`, `proc`, `match`, object blocks, and doc comment blocks
- **Document highlight** — highlight all occurrences of a symbol under the cursor (declarations as Write, usages as Read)
- **Selection ranges** — `Shift+Alt+→` smart selection expansion (bracket-aware: token → bracket content → bracket pair → line → document)
- **Code actions** — quick fixes: "Add missing `returnA`", "Wrap in `do` block", "Convert `//` to `///`", "Extract to named arrow"
- **Context-aware completion** — keyword snippets after `loop`/`proc`/`match`/`do`/`returnA`, field name completion after `state.`, combinator suggestions at expression positions, match pattern completion after `]:`
- **Snippets** — 18 templates including `loop`, `loop-fields`, `proc`, `match`, `do`, `returnA`, `arr`, `seq`, `choice`, `comp`, `first`, `named`, `axiom`, `doc`, `if`, `where`, `bind`
- **Parse diagnostics** — real syntax errors shown as you type (severity: Error)
- **Refinement diagnostics** — refinement inference errors shown as warnings
- **Settings** — `handl.server.path`, `handl.enableInlayHints`, `handl.enableCodeLens`, `handl.enableRefinementDiagnostics`, `handl.enableSemanticHighlighting`
- **Logo** — handl brand icon displayed in the extensions panel
- **README** — full feature documentation, installation guide, and settings reference
