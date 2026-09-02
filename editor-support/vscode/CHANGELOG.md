# Changelog

All notable changes to the **handl** VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Configuration moved to `.handl.toml`** (ADR 0004) — `handl.server.path`, `handl.enableInlayHints`, `handl.enableCodeLens`, `handl.enableRefinementDiagnostics`, and `handl.enableSemanticHighlighting` VS Code settings have been removed. Editor configuration now lives in a `.handl.toml` manifest at the workspace root and is passed to the server via `initializationOptions`. The `contributes.configuration` section has been removed from `package.json`.

### Added

- **`.handl.toml` manifest reader** — minimal TOML parser (no dependencies) supporting `[language]` and `[editor]` sections with string, boolean, and array values.
- **File extension mapping via `files.associations`** — additional extensions declared in `.handl.toml` (`[language].extensions`) are mapped to the `handl` language id at runtime, so the extension never needs repackaging for a new file extension.

### Removed

- **VS Code settings** — `handl.server.path`, `handl.enableInlayHints`, `handl.enableCodeLens`, `handl.enableRefinementDiagnostics`, `handl.enableSemanticHighlighting` (replaced by `.handl.toml`).

## [0.1.0] - 2026-09-02

Initial release of the handl extension for VS Code.

### Added

- **Syntax highlighting** — 100% semantic tokens from the LSP server for `.hnl` files, covering keywords, operators, strings, numbers, comments, type names, object keys, and property access (no TextMate grammar)
- **Refinement hover** — hover over any arrow file to see its inferred input/output types and guarantee predicate
- **Per-property hover** — hover over `state.acc` or `state.remaining` to see the inferred refinement type for that specific loop state field
- **Cross-arrow hover** — hover over an arrow reference (e.g. `insert` in `sort.hnl`) to see that arrow's doc comments and inferred contract
- **Doc comment hover** — `---` doc comments at the top of a file are shown when hovering over `loop` or arrow keywords
- **Signature help** — parameter hints for arrow combinators (`arr`, `comp`, `first`, `choice`, `app`, `loop`, `id`, `insert`)
- **Go to definition** — jump to arrow files and axiom definitions across the workspace (`Ctrl+Click` or `F12`)
- **Type definition** — jump to the inferred refinement type of a symbol (`Go to Type Definition`)
- **Implementation** — jump from an axiom/trait to its arrow implementations
- **Document symbols** — outline view of arrows, axioms, and bindings in a file (`Ctrl+Shift+O`)
- **Workspace symbols** — search for arrows and axioms across all `.hnl` files (`Ctrl+T`)
- **References** — find all references to a symbol across the workspace (`Shift+F12`)
- **Document highlight** — highlight all occurrences of a symbol under the cursor (declarations as Write, usages as Read)
- **Formatting** — auto-indent with 2-space tabs, colon alignment, trailing whitespace cleanup, and final newline enforcement (format on save enabled by default)
- **Range formatting** — format just a selection
- **On-type formatting** — auto-indent on `Enter`, `}`, `)`, `]`
- **Inlay hints** — inferred refinement types shown inline next to loop state fields and overall `In ~> Out` signatures
- **Inlay hint resolve** — lazy enrichment of inlay hints with full type tooltips
- **Rename** — rename arrows across all files in the workspace, including renaming the file itself (`F2`)
- **Folding ranges** — collapse `loop`, `match`, object blocks, and doc comment blocks
- **Selection ranges** — `Shift+Alt+→` smart selection expansion (bracket-aware: token → bracket content → bracket pair → line → document)
- **Linked editing range** — sync-edit matching bracket pairs
- **Code actions** — quick fixes: "Add missing `return`", "Wrap in `do` block", "Convert `--` to `---`", "Extract to named arrow"
- **Code action resolve** — lazy resolution of code action edits
- **Code lens** — inferred contract summaries at the top of arrow files
- **Code lens resolve** — lazy resolution of code lens content
- **Call hierarchy** — view incoming and outgoing arrow calls (`Right-click` → "Show Call Hierarchy")
- **Document link** — clickable links in comments for file references, arrow references, and URLs
- **Context-aware completion** — keyword snippets after `loop`/`match`/`do`/`return`, field name completion after `state.`, combinator suggestions at expression positions, match pattern completion after `]:`
- **Completion resolve** — lazy enrichment of completion items with documentation and type signatures
- **Moniker** — stable cross-workspace symbol identifiers
- **Snippets** — templates including `match`, `rec`, `do`, `if`, `seq`, `choice`, `record`, `doc`, and more, provided by the LSP server's completion provider (data-driven, no static snippets file)
- **Parse diagnostics** — real syntax errors shown as you type (severity: Error)
- **Refinement diagnostics** — refinement inference errors shown as warnings
- **File watcher** — re-diagnostic affected files when `.hnl` files change externally
- **Execute command** — `handl.showRefinement`, `handl.runArrow`, `handl.formatFile`, `handl.showContract` commands
- **Settings** — `handl.server.path`, `handl.enableInlayHints`, `handl.enableCodeLens`, `handl.enableRefinementDiagnostics`, `handl.enableSemanticHighlighting`
- **Logo** — handl brand icon displayed in the extensions panel
- **README** — full feature documentation, installation guide, and settings reference

### Changed

- Arrow syntax migrated from `proc (x) -> body` to `x ~> body` (and `[x, y] ~> body` for multi-arg)
- Comment syntax migrated from `//` / `///` to `--` / `---` to avoid URL ambiguity
- Extension renamed from `handl-lsp` to `handl`
