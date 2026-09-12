# ADR-0100: LSP as Language Data

## Context

ADR-0044 ("language as data") established that engines and tooling are generic
and data-driven: a new language is a YAML fragment merged into a `Language`,
never an engine recompile. The formatter, linter, and typechecker already
follow this. The LSP support did not.

Currently the canonical LSP surface is spread across four small crates
(`plugin-lsp-completion`, `plugin-lsp-goto`, `plugin-lsp-hover`,
`plugin-lsp-symbol`) and each one hard-codes handl constructs: `prev.text ==
"from"`, `prev.text == "import"`, scanning for `axiom` / `law` / `export`,
`.hnl` extension handling, `---` and `--` doc comments, and the `time/time.hnl`
module-directory layout. Renaming a keyword or adding a new export form requires
editing Rust in 3–4 places. This is the exact pattern ADR-0044 bans.

## Decision

LSP behavior is first-class `Language` data, with a single generic engine crate
and a thin handl adapter.

1. **`LspDef` fragment** (`handl-language::fragments::lsp`), merged into a new
   `Language::lsp: FragmentSet<LspDef>` (added with `serde(default)` so
   pre-ADR-0100 serialized languages still load). The canonical data lives in
   `crates/handl-kit-data-canonical/data/lsp.yaml`.

   `lsp.yaml` declares:

   - `import` — tokens that introduce an import path or name, and the
     filesystem layouts used to resolve a module spec.
   - `export` — `!Match { node_kind }` rules for declarations that can be
     completed, hovered, jumped to, or outlined; each rule names the `role`
     that yields the exported name, an optional `signature` role, and which
     trivia tokens provide documentation.
   - `hover`, `goto`, `completion` — feature sections that *reference* the
     shared `export` rules by id instead of duplicating them.
   - `document_symbol` — `!Match` rules mapping AST nodes to LSP `SymbolKind`
     names and a name role.

2. **Two new crates** replace the four `plugin-lsp-*` crates:

   - `crates/plugin-lsp` — the generic LSP engine. It implements
     `textDocument/completion`, `textDocument/hover`,
     `textDocument/definition`, and `textDocument/documentSymbol` by reading
     `Language::lsp`. It contains modules `completion.rs`, `hover.rs`,
     `goto.rs`, `document_symbol.rs`, and shared helpers (`import.rs`,
     `export.rs`, `docs.rs`). It is not handl-specific.
   - `crates/plugin-lsp-std` — the canonical handl adapter. It registers the
     `handl-kit-data-canonical` `Language` (which now includes `lsp.yaml`) with
     `plugin-lsp` and provides the `LspPlugin` wiring that `host-lsp` loads.

   The old `plugin-lsp-completion`, `plugin-lsp-goto`, `plugin-lsp-hover`, and
   `plugin-lsp-symbol` crates are deleted and removed from the workspace.

3. **Validation and resilience.** `merge_fragments` and
   `handl_plugin_engine_transform::validate` already check token kinds, AST
   shapes, roles, and `TransformCondition` references at `Language` build time.
   `lsp.yaml` reuses the same `!Match` vocabulary, so the same validation
   applies. Missing or unknown roles, tokens, or node kinds are caught when the
   `Language` is assembled, not at LSP request time.

4. **Fallbacks and robustness.** Each export rule declares `fallback: source`
   for signatures (use the node's own source text when a dedicated signature
   role is absent) and `dotted_last_segment: true`/`false` for names. The
   engine silently skips a rule if the matched node does not provide the
   required role, so one malformed construct does not break the whole outline
   or completion list.

5. **Caching is the host's responsibility.** `plugin-lsp` is stateless and
   operates on a `(source, Language)` snapshot. `host-lsp` already re-parses on
   every change; `plugin-lsp` may cache per-document exported-name tables if
   profiling shows it is necessary, but the initial implementation does not.

## Consequences

- Adding a new handl export form (e.g. `op` or `handler`) is an `lsp.yaml` edit
  plus the corresponding `ast.yaml` shape — no `plugin-lsp` recompile.
- Changing the `from`/`import` surface is a token or grammar data change;
  `plugin-lsp` does not hard-code any token text.
- A future `plugin-lsp-python` or `plugin-lsp-foo` can reuse the same engine
  with a different `Language` and `lsp.yaml`; the engine is generic over the
  `handl-language` model.
- `!Match` is shared vocabulary with `lints.yaml` and `transforms.yaml`, so
  validation, condition evaluation, and third-party fragment authoring are
  consistent.
- The generic engine is larger and more abstract than the current hand-coded
  plugins. It must be tested with a minimal `Language` and `lsp.yaml` in
  `plugin-lsp/tests`.
- `plugin-lsp-std` should eventually contain no logic: it is only a
   registration crate. Any handl-specific code that cannot be expressed in
   `lsp.yaml` is a defect in the data model, not a reason to add Rust to
   `plugin-lsp-std`.

## Status

accepted
