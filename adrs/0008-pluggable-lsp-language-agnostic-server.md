# Pluggable LSP — Language-Agnostic Server

## Context

The `host-lsp` crate is the LSP server for handl. It implements every LSP feature: completion, hover, diagnostics, document symbols, semantic tokens, call hierarchy, code actions, formatting, folding ranges, inlay hints, signature help, rename, and more. Each feature handler is a Rust module under `crates/host-lsp/src/`.

The problem: **every handler hardcodes language knowledge.** Concrete examples from the current code:

- `completion.rs` has a `const KEYWORDS: &[KeywordEntry]` array with 20+ hardcoded entries (`match`, `if`, `rec`, `while`, `import`, `trait`, `law`, ...). Adding a keyword means editing this array.
- `completion.rs` has `detect_context()`, `classify_identifier()`, `inside_do_block()`, `is_match_pattern_colon()`, `rec_state_fields()` — all hardcoded to the current handl surface syntax.
- `diagnostics.rs` has `collect_diagnostics()` and `parse_diagnostics()` that call directly into the handl parser.
- `hover.rs`, `document_symbols.rs`, `signature_help.rs`, `folding_ranges.rs`, `code_actions.rs`, and ~15 other modules each `use handl_kit_canonical::{SpannedToken, is_comment_kind, is_identifier_kind, ...}` and pattern-match on token kinds specific to the current language.
- `refinement.rs` imports `handl_language::Scheme` directly.

The coupling runs in both directions:

1. **LSP → language:** The server imports `handl_kit_canonical`, `handl_language`, and `plugin_ext_refinement`. Every handler depends on concrete language types.
2. **Language → LSP:** When the language changes (new keywords, new syntax, new stdlib functions, new grammar rules), the LSP server must be edited and rebuilt. ADR-0002 made the *extension* data-driven (no static TextMate/snippets), but the *server* is still hand-written per language.

ADR-0003 added per-workspace language variants via `.handl.toml` — different workspaces can use different data files. But the server code that *consumes* those data files is still hardcoded to one language shape. Per-workspace variants work only for data-level differences (token names, grammar rules), not for structural differences in how LSP features are computed.

### What we want

Changing the language — lex, parse, eval, stdlib, type system, refinement — should not require touching `host-lsp`. The LSP server should be a **generic protocol adapter**: it receives LSP requests (positions, document events, workspace folders), delegates language-specific work to pluggable providers, and returns LSP-shaped responses.

This is the same principle as ADR-0002 (data-driven editor support) and ADR-0003 (per-workspace variants), extended from the extension to the server. The extension is already generic; the server should be too.

## Decision

Define a **provider trait surface** that abstracts every language-specific operation the LSP server performs. The server depends only on these traits, never on concrete language types. Language crates implement the traits; the server is wired to the implementations at startup (resolved per workspace via `.handl.toml`).

### The provider traits

The server delegates to a small set of provider traits, each covering a cluster of LSP features:

```rust
/// Lexing — tokenise source text into positioned tokens.
pub trait LexProvider {
    fn lex(&self, uri: &str, text: &str) -> Vec<Token>;
}

/// Parsing — parse source text into a syntax tree + diagnostics.
pub trait ParseProvider {
    fn parse(&self, uri: &str, text: &str) -> ParseResult;
}

/// Semantic analysis — type checking, refinement, cross-file resolution.
pub trait SemanticProvider {
    fn analyze(&self, uri: &str, text: &str) -> SemanticResult;
}

/// Completion — context-aware completions at a position.
pub trait CompletionProvider {
    fn completions(&self, uri: &str, text: &str, offset: usize) -> Vec<CompletionItem>;
}

/// Hover — documentation / type info at a position.
pub trait HoverProvider {
    fn hover(&self, uri: &str, text: &str, offset: usize) -> Option<Hover>;
}

/// Document symbols — outline structure.
pub trait DocumentSymbolProvider {
    fn symbols(&self, uri: &str, text: &str) -> Vec<DocumentSymbol>;
}

/// Semantic tokens — syntax highlighting.
pub trait SemanticTokenProvider {
    fn tokens(&self, uri: &str, text: &str) -> Vec<SemanticToken>;
}

/// Diagnostics — errors and warnings.
pub trait DiagnosticProvider {
    fn diagnostics(&self, uri: &str, text: &str) -> Vec<Diagnostic>;
}

/// Folding ranges, selection ranges, document highlight, etc.
/// Grouped into a "structural" provider since they all operate on
/// token/tree structure without semantic analysis.
pub trait StructuralProvider {
    fn folding_ranges(&self, uri: &str, text: &str) -> Vec<FoldingRange>;
    fn selection_ranges(&self, uri: &str, text: &str, offsets: &[usize]) -> Vec<SelectionRange>;
    fn document_highlight(&self, uri: &str, text: &str, offset: usize) -> Vec<DocumentHighlight>;
}

/// Cross-file features — go-to-definition, rename, call hierarchy.
/// These need workspace context (multiple files, module graph).
pub trait CrossFileProvider {
    fn definition(&self, uri: &str, offset: usize, ws: &Workspace) -> Vec<Location>;
    fn references(&self, uri: &str, offset: usize, ws: &Workspace) -> Vec<Location>;
    fn rename(&self, uri: &str, offset: usize, new_name: &str, ws: &Workspace) -> WorkspaceEdit;
    fn call_hierarchy(&self, uri: &str, offset: usize, ws: &Workspace) -> Vec<CallHierarchyItem>;
}
```

The exact trait set is not final — some may merge, some may split. The principle is: **one trait per cluster of LSP features that shares the same language dependencies.**

### The server

`host-lsp` becomes:

1. **Protocol layer** — LSP JSON-RPC dispatch, message framing, lifecycle. Unchanged. No language knowledge.
2. **Document manager** — tracks open documents, incremental changes, line maps. Unchanged. No language knowledge.
3. **Provider registry** — holds `Box<dyn LexProvider>`, `Box<dyn ParseProvider>`, etc., resolved per workspace from `.handl.toml`. Each LSP handler delegates to the appropriate provider.
4. **Feature handlers** — thin adapters: receive LSP request → extract URI/text/offset → call provider → shape response as LSP type. No `use handl_kit_canonical`, no `use handl_language`, no hardcoded keywords.

### The language crate

A new crate (or extension of an existing one) implements all provider traits for the current handl language. It imports `handl_kit_canonical`, `handl_language`, `plugin_ext_refinement`, etc. It contains the keyword tables, the context detection, the token classification — everything that is currently in `host-lsp` handlers.

This crate is the **language plugin**. The server loads it based on `.handl.toml`. A different language variant would provide a different plugin crate (or the same crate configured with different data files).

### What moves where

| Currently in `host-lsp` | Moves to |
|---|---|
| `const KEYWORDS` in `completion.rs` | `CompletionProvider` impl |
| `detect_context()`, `classify_identifier()` | `CompletionProvider` impl |
| `collect_diagnostics()`, `parse_diagnostics()` | `DiagnosticProvider` impl |
| `is_comment_kind`, `is_identifier_kind` usage | `StructuralProvider` / `SemanticTokenProvider` impls |
| `handl_language::Scheme` import in `refinement.rs` | `SemanticProvider` impl |
| Cross-file def/rename logic | `CrossFileProvider` impl |

### Migration

This is a large refactor (~30 handler modules). Migrate incrementally:

1. Define the provider traits in a new `host-lsp-providers` crate (or in `host-lsp` behind a feature flag).
2. Implement the traits for the current handl language in a `handl-lsp-plugin` crate, initially by moving code verbatim from `host-lsp` handlers.
3. Wire `host-lsp` to use the provider registry, one feature at a time. Start with `DiagnosticProvider` (simplest, fewest dependencies), then `SemanticTokenProvider`, `CompletionProvider`, etc.
4. Each migration step is independently shippable: the server works, tests pass, one less handler is hardcoded.
5. When all handlers are migrated, `host-lsp` has zero `use handl_kit_canonical` or `use handl_language` imports.

### Relationship to existing ADRs

- **ADR-0002** (data-driven editor support): made the *extension* generic. This ADR makes the *server* generic. Same principle, different layer.
- **ADR-0003** (per-workspace language variants): made the server load different *data files* per workspace. This ADR makes the server delegate to different *provider implementations* per workspace. ADR-0003 is the data-level version; this ADR is the code-level version. They compose: `.handl.toml` selects both the data files and the provider plugin.
- **ADR-0069** (arrow-native modules): the `ModuleLoader` and normalisation strategies are language-specific logic that would live behind the `SemanticProvider` / `CrossFileProvider` traits, not in the LSP server.

## Consequences

**Positive:**

- The LSP server is stable across language changes. New keywords, new syntax, new stdlib, new type system features — none touch `host-lsp`.
- Language variants (ADR-0003) can differ structurally, not just in data. A v0.2 language with a fundamentally different parse tree can coexist with v0.1 in the same server.
- The language plugin crate is a clean boundary: everything language-specific lives there, testable in isolation.
- Other tools (CLI, REPL, formatter) can reuse the same provider traits.

**Negative:**

- One more layer of indirection: every LSP request goes through a trait dispatch. The cost is a virtual call per request — negligible for LSP (requests are user-triggered, not hot-path).
- The initial migration is large (~30 modules). Must be done incrementally to avoid a big-bang refactor.
- The trait surface must be designed carefully. If the traits leak language-specific concepts (e.g. `Token` has a `kind: handl_kit_canonical::TokenKind` field), the abstraction is broken. The trait types must be LSP-shaped or generic, not language-shaped.

**Risk:**

- Over-abstraction. If the provider traits end up with one method per LSP feature and no shared logic, the layer adds indirection without simplifying anything. Mitigation: cluster features that share dependencies (the trait list above groups ~30 LSP features into ~8 traits), and let the language plugin share internal state across trait impls (e.g. one parse tree cached and reused by completion, hover, symbols, folding).

## Status

Proposed
