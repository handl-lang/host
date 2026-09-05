---
title: VS Code extension constraints
---

# VS Code extension constraints

The handl VS Code extension is a thin packaging layer over the LSP server. The server is fully data-driven — it reads `lexer_rules.yaml`, `tokens.yaml`, and `grammar.yaml` at runtime. The extension's job is to start the server and register VS Code providers.

However, VS Code imposes hard constraints on what an extension can do at runtime versus what must be declared statically at build time. These constraints shape the architecture of the extension and the division of responsibility between the extension and the server.

## What must be static

These are declared in `package.json` and read by VS Code at startup, before any extension code runs. There is no runtime API to change them.

### Language ID and file extensions

VS Code requires every language to have an ID (e.g., `"handl"`) and associated file extensions (e.g., `[".hnl"]`) declared in `contributes.languages`. There is no `vscode.languages.registerLanguage()` API.

Adding a new language or file extension requires editing `package.json` and repackaging the extension. This is a platform limitation with no workaround.

### Config settings schema

Configuration settings (e.g., `handl.enableInlayHints`) must be declared in `contributes.configuration.properties`. There is no runtime API for declaring settings.

Adding a new setting requires editing `package.json` and repackaging.

### TextMate grammar path

If a TextMate grammar is used, its file path must be declared in `contributes.grammars`. VS Code loads and compiles grammars at startup, before `activate()` runs. There is no runtime API to register or swap a TextMate grammar.

However, a grammar is **not required**. A language can exist without one. The handl extension ships no TextMate grammar — all highlighting comes from the server's semantic token provider.

### Snippet file path

If static snippet files are used, their paths must be declared in `contributes.snippets`. Same as grammars — loaded at startup.

However, a completion provider registered at runtime can return `CompletionItem`s with `insertTextFormat: 2` (snippet syntax), which are indistinguishable from static snippets. The handl extension ships no snippet files — all snippets come from the server's completion provider.

## What can be runtime

These have full runtime API support. The extension registers them in `activate()`.

### Semantic tokens

`vscode.languages.registerDocumentSemanticTokensProvider()` registers a provider at runtime. Semantic tokens overlay on top of TextMate highlighting (if any). A language with no TextMate grammar still receives full semantic highlighting once a provider is active.

The handl server provides semantic tokens via the LSP `textDocument/semanticTokens/full` request. The `vscode-languageclient` library auto-registers the provider when the server declares the capability. This is how all syntax highlighting works in handl — keywords, operators, comments, strings, type names, and more all come from the server's data-driven lexer.

### Completion (including snippets)

`vscode.languages.registerCompletionItemProvider()` registers a provider at runtime. Completion items can include snippet-syntax templates (`insertTextFormat: 2`).

The handl server provides completions via `textDocument/completion`. Snippets like `loop`, `match`, `do`, and `arr` are returned as completion items with snippet syntax, not as static files.

### Language configuration

`vscode.languages.setLanguageConfiguration()` sets bracket matching, autoclosing pairs, comment toggling, and indentation rules at runtime. Returns a `Disposable`.

The handl extension calls this in `activate()` to configure `()`, `[]`, `{}` autoclosing and comment toggling.

### All other LSP features

Hover, go-to-definition, references, rename, formatting, code actions, code lens, inlay hints, folding ranges, document symbols, workspace symbols, diagnostics, and all other LSP features are registered at runtime by `vscode-languageclient` based on the server's declared capabilities. None of these require static declarations in `package.json`.

## The division of responsibility

The constraints create a clear split:

| Concern                       | Owner                                          | Why                                 |
| ----------------------------- | ---------------------------------------------- | ----------------------------------- |
| Language ID, file extension   | Extension (`package.json`)                     | VS Code requires static declaration |
| Config settings schema        | Extension (`package.json`)                     | VS Code requires static declaration |
| Syntax highlighting           | Server (semantic tokens)                       | Runtime API, data-driven from YAML  |
| Snippets                      | Server (completion provider)                   | Runtime API, data-driven            |
| Bracket matching, autoclosing | Extension (runtime `setLanguageConfiguration`) | Runtime API, universal brackets     |
| All LSP features              | Server                                         | Runtime, data-driven                |

The extension is a packaging layer: it declares the language ID and file extension, starts the server, and registers a handful of runtime providers. The server is the single source of truth for everything about the language.

## Why this matters

When the handl language syntax changes — a new keyword, a new operator, a new comment style, a new snippet — only the server needs rebuilding. The server reads the updated YAML data files at runtime. The extension does not ship any language-specific content, so it does not need repackaging.

The only changes that require repackaging the extension are VS Code platform requirements: a new file extension, a new language ID, or a new config setting. These are structural changes to the extension manifest, not changes to the language itself.
