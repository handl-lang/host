# Data-Driven Editor Support

## Context

The handl platform defines languages as data: `lexer_rules.yaml`, `tokens.yaml`, `grammar.yaml`, and `ast.yaml` in `crates/handl-kit-data-canonical/data/`. The Rust LSP server (`handl-host-lsp`) loads these at runtime — changing the language syntax means updating the data and rebuilding the server, not rewriting server logic.

The VS Code extension, however, ships three language-specific artifacts as static files:

1. **TextMate grammar** (`grammars/hnl.tmLanguage.json`) — 163 lines of regex patterns for keywords, operators, comments, strings, type names, object keys, property access, brackets, and identifiers. Generated from data via `scripts/gen-grammar.mjs`, but the output is a static file baked into the `.vsix`. When the syntax changes, you must re-run `gen-grammar` and repackage the extension.

2. **Snippets** (`snippets/hnl.code-snippets.json`) — 18 handl-specific code templates (`loop`, `match`, `do`, `returnA`, `arr`, etc.) shipped as a static file referenced in `package.json`. Adding or changing a snippet requires editing this file and repackaging.

3. **Language configuration** — bracket matching, autoclosing pairs, and comment syntax are not currently declared, but if they were, they would be static in `package.json`.

This means the extension rebuild pipeline has an extra step that should not be necessary:

```
language data changes → re-run gen-grammar → repackage extension → reinstall
```

The server side is already clean:

```
language data changes → rebuild server → done
```

### VS Code constraints

VS Code imposes hard constraints that were verified against the official extension API documentation:

1. **Language ID and file extensions must be statically declared in `package.json`.** There is no `vscode.languages.registerLanguage()` API. The `contributes.languages` contribution point is the only mechanism, read at startup before any extension activates.

2. **TextMate grammar must be a file path in `package.json`.** There is no runtime API to register a TextMate grammar. VS Code loads and compiles grammars at startup, before `activate()` runs. However, a grammar is **not required** — a language can exist without one.

3. **Snippet files must be statically referenced in `package.json`.** Same as grammars — loaded at startup. However, a completion provider registered at runtime can return `CompletionItem`s with `insertTextFormat: 2` (snippet syntax), which are indistinguishable from static snippets to the user.

4. **Language configuration can be set at runtime.** `vscode.languages.setLanguageConfiguration()` is a fully supported runtime API. No static declaration needed.

5. **Semantic tokens can be registered at runtime.** `vscode.languages.registerDocumentSemanticTokensProvider()` is fully supported. Semantic tokens overlay on top of TextMate highlighting. A language with no TextMate grammar still receives full semantic highlighting once a provider is active.

6. **Config settings schema must be static in `package.json`.** No runtime API for declaring config settings.

Constraints 1 and 6 are unavoidable — the extension must be repackaged for new language IDs, file extensions, or config settings. But constraints 2, 3, and 4 all have workarounds that make language *content* fully data-driven.

### What the server already provides

The LSP server already implements:
- **Semantic tokens** (`textDocument/semanticTokens/full`) — full syntax highlighting from the data-driven lexer, including keywords, operators, strings, numbers, comments, type names, object keys, property access. This is already wired and working.
- **Context-aware completion** (`textDocument/completion`) — keyword snippets after `loop`/`match`/`do`/`returnA`, field name completion after `state.`, combinator suggestions. Already wired and working.

These server-side features read from the YAML data files at runtime. The extension just needs to rely on them instead of shipping static duplicates.

## Decision

Eliminate all static language-specific artifacts from the extension. The extension ships zero TextMate grammar, zero snippet files, and zero hardcoded language configuration. Every aspect of the language — including comment syntax — flows through the server's data files.

### Step 1: No TextMate grammar

Remove the `contributes.grammars` section from `package.json`. Delete `grammars/hnl.tmLanguage.json` and `scripts/gen-grammar.mjs`.

All syntax highlighting comes from the server's semantic token provider, which reads `lexer_rules.yaml` and `tokens.yaml` at runtime. This includes comments — the server already emits `comment` and `doc_comment` semantic tokens from the lexer data. Changing comment syntax from `--` to something else is a data file change + server rebuild, with no extension involvement.

VS Code does not require a grammar to be registered for a language. A language ID + file extension with no grammar is valid. Before the server starts, the file appears as plain text. Once the server connects (sub-second), semantic tokens provide full highlighting. VS Code caches semantic tokens across reloads, so subsequent opens are immediate.

**Before:**
```
package.json: "grammars": [{ "language": "handl", "scopeName": "source.handl", "path": "./grammars/hnl.tmLanguage.json" }]
grammars/hnl.tmLanguage.json  (163 lines, generated from data)
scripts/gen-grammar.mjs
```

**After:**
```
package.json: (no grammars contribution)
(no grammars/ directory)
(no gen-grammar script)
```

### Step 2: Snippets via server completion

Move all 18 snippets from `snippets/hnl.code-snippets.json` into the server's completion provider. The server returns snippet-type completion items (`insertTextFormat: 2`) with VS Code snippet syntax for templates like `loop`, `match`, `do`, `returnA`, `arr`, etc.

**Before:**
```
package.json: "snippets": [{ "language": "handl", "path": "./snippets/hnl.code-snippets.json" }]
snippets/hnl.code-snippets.json  (18 templates)
```

**After:**
```
package.json: (no snippets contribution)
(no snippets/ directory)
server completion provider returns snippet items
```

The snippet definitions move to a data file the server reads (e.g., `crates/handl-kit-data-canonical/data/snippets.yaml`) or are hardcoded in the server's completion module.

### Step 3: Language configuration at runtime

Call `vscode.languages.setLanguageConfiguration("handl", config)` at activation with bracket pairs, autoclosing, and comment syntax. The configuration is read from the server or hardcoded in the extension (bracket pairs are universal: `()`, `[]`, `{}`).

**Before:**
```
package.json: (no language configuration declared)
```

**After:**
```
extension.ts: vscode.languages.setLanguageConfiguration("handl", { ... })
package.json: (unchanged)
```

### Step 4: Remove gen-grammar build step

Remove `scripts/gen-grammar.mjs` and the `gen-grammar` script from `package.json`. The `build` script becomes just `vite build`:

```json
"build": "vite build",
```

The extension no longer depends on the `hnl` binary being built first.

### Resulting rebuild matrix

| Change | Rebuild server? | Rebuild extension? |
|--------|----------------|-------------------|
| New keyword/operator | Yes (data) | **No** |
| New token kind | Yes (data) | **No** |
| New snippet | Yes (data) | **No** |
| New bracket pair | Yes (data) | **No** |
| Comment syntax change | Yes (data) | **No** |
| String syntax change | Yes (data) | **No** |
| New file extension | No | **Yes** (package.json) |
| New language ID | No | **Yes** (package.json) |
| New config setting | No | **Yes** (package.json) |

The extension only needs rebuilding for VS Code platform requirements (language ID, file extension, config schema). **Every aspect of the language itself is data-driven through the server.**

### What stays in the extension

| Artifact | Why |
|----------|-----|
| `package.json` language ID + `.hnl` extension | VS Code requires static declaration — unavoidable |
| `package.json` config schema | VS Code requires static declaration — unavoidable |
| `extension.ts` + modules | Client lifecycle, provider registration, language configuration |

### What is eliminated from the extension

| Artifact | Replaced by |
|----------|------------|
| TextMate grammar (all 163 lines) | Server semantic tokens (already implemented) |
| Snippets file | Server completion provider with snippet items |
| `gen-grammar` build step | Not needed — no grammar to generate |
| `grammars/` directory | Not needed |
| `snippets/` directory | Not needed |

## Alternatives Considered

### Alternative A: Minimal TextMate grammar (comments + strings only)

Ship a ~20-line static grammar covering only comments and strings for pre-server highlighting.

**Rejected because:**
- Still hardcodes comment syntax (`--`, `---`) in the extension. Changing comment syntax would require an extension rebuild.
- The server already emits comment and string semantic tokens from data. Any static grammar is a duplicate.
- The pre-server window is sub-second and VS Code caches semantic tokens. The minimal highlighting benefit does not justify shipping a static file that can go stale.

### Alternative B: Keep full static TextMate + gen-grammar pipeline

Keep generating the full TextMate grammar from data at build time.

**Rejected because:**
- The grammar is the most frequently-changing artifact and the main reason the extension needs repackaging when syntax changes.
- The server already provides semantic tokens that cover everything the TextMate grammar covers.
- The `gen-grammar` build step adds complexity (requires the `hnl` binary to be built before the extension can be packaged).
- The generated TextMate regexes are fragile — they duplicate logic that the server's lexer already handles correctly from the same data.

### Alternative C: Generate TextMate grammar at activation time

Write the grammar to the extension's global storage path at activation, then point VS Code to it.

**Rejected because:**
- VS Code loads TextMate grammars from `package.json` paths at startup, before `activate()` runs. A generated file would be too late.
- Would require a two-phase activation: first activation generates the grammar, then the user must reload to pick it up. Poor UX.

### Alternative D: Multi-language manifest

Ship a `languages.json` manifest that declares multiple languages. The extension reads it at activation and creates a client per language.

**Rejected because:**
- There is currently only one language. Building multi-language support is premature.
- VS Code still requires all language IDs and file extensions to be in `package.json` statically. The manifest saves editing TypeScript but not repackaging.
- Can be added later when a second language exists.

## Consequences

### Positive

- **No language-specific static artifacts in the extension.** Zero TextMate grammar, zero snippet files, zero hardcoded syntax. The extension is a pure packaging layer.
- **Syntax changes never require repackaging the extension.** Update data files, rebuild the server, reload the window. This includes comment syntax, string syntax, keywords, operators, tokens, snippets, and bracket pairs.
- **No `gen-grammar` build step.** The extension builds with just `vite build` — no dependency on the `hnl` binary.
- **Single source of truth.** The server's data files are the only source of language syntax. No duplicates, no sync issues.
- **Simpler extension.** Fewer files, fewer build steps, fewer things to keep in sync.

### Negative

- **No highlighting before the server starts.** For ~100-500ms after opening a file, the document appears as plain text. Once the server connects, semantic tokens provide full highlighting. VS Code caches semantic tokens across reloads, so subsequent opens of the same file are immediate. This is the fundamental tradeoff of eliminating the TextMate grammar entirely.
- **Snippets require server round-trip.** Static snippets are available instantly; server-driven snippets require the server to be running. In practice, the server is always running when editing `.hnl` files, so this is not a user-visible delay.
- **Semantic highlighting depends on theme support.** The user's theme must have semantic highlighting enabled (most modern themes do). If it is disabled, the file appears as plain text with no fallback. This can be documented in the README.

### Neutral

- **Bundle size** decreases (no grammar file, no snippets file, no gen-grammar script).
- **Server binary size** may increase slightly if snippet definitions move into the server (negligible).
- **Startup time** is unchanged — the server starts in the same amount of time; highlighting just begins when the server is ready instead of from a static grammar.

## Status

Proposed
