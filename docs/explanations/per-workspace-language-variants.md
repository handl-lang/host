---
title: Per-workspace language variants
---

# Per-workspace language variants

The handl platform defines languages as data — `lexer_rules.yaml`, `tokens.yaml`, `grammar.yaml`. The LSP server loads these at runtime. This means different projects can use different language variants without separate editor extensions, language IDs, or file extensions.

## The problem

Handl syntax evolves. Projects at different stages of migration may use different syntax:

```
workspace/
├── project-a/          ← v0.1: loop, match, do, returnA
│   └── arrows/*.hnl
├── project-b/          ← v0.2: rec, while, for, return
│   └── arrows/*.hnl
```

Both use `.hnl` files. VS Code treats them as one language. Without per-workspace variants, changing syntax for one project affects all projects.

## The solution: `.handl.toml`

A `.handl.toml` manifest at the project root declares which language kit to use:

```toml
# .handl.toml
[language]
kit = "handl-kit-data-canonical"
version = "0.1"
```

The server resolves the kit per file by walking up from the file's path to find the nearest `.handl.toml`. Each file gets the syntax, tokens, and grammar defined by its project's kit.

## How it works

When the server receives a request (e.g., `semanticTokens/full`) for `file:///workspace/project-a/arrows/sort.hnl`:

1. Walk up from `/workspace/project-a/arrows/` looking for `.handl.toml`
2. Find it at `/workspace/project-a/.handl.toml`
3. Read the manifest: kit = `handl-kit-data-canonical`, version = `0.1`
4. Load (or retrieve from cache) that kit's data files
5. Lex and tokenize using those rules
6. Return semantic tokens

When the same request comes for `project-b/arrows/sort.hnl`, the server finds project-b's `.handl.toml`, loads v0.2 data, and returns different tokens. Same language ID, same extension, same server — different syntax.

## What the extension does

Nothing. The extension declares the language ID and file extension, starts the server, and forwards all requests. The server handles per-folder differentiation. The extension does not know or care which kit is used.

This is editor-agnostic. `.handl.toml` is a language-level concept, not a VS Code setting. The same mechanism works in Neovim, Emacs, or any LSP client.

## Custom domain extensions

Projects can define custom syntax without forking the server:

```
workspace/
├── data-pipeline/
│   ├── .handl.toml              ← kit = "./custom-kit"
│   ├── custom-kit/
│   │   ├── lexer_rules.yaml     ← adds: stream, sink, source
│   │   ├── tokens.yaml
│   │   └── grammar.yaml
│   └── arrows/
│       └── ingest.hnl           ← uses custom keywords
```

The server loads the custom kit. `stream`, `sink`, and `source` are highlighted as keywords. Completion offers snippets for them. Diagnostics parse using the custom grammar. No extension changes, no server recompilation.

## Fallback

If no `.handl.toml` is found, the server uses the default kit (the canonical kit compiled into the server). Existing projects without a manifest continue to work unchanged.

## Caching

Kits are loaded once per workspace and cached by `(kit_name, version)`. The cache is invalidated when `.handl.toml` changes (detected via the file watcher) or when the server restarts.

## Why not VS Code settings?

VS Code per-folder settings (`handl.kit` in `.vscode/settings.json`) would tie the concept to one editor. `.handl.toml` works in any LSP client. The server resolves it from the file path alone, with no editor cooperation.

## Why not per-file markers?

A directive at the top of each `.hnl` file would require modifying every file. It also creates a chicken-and-egg problem: the directive must be parsed before the kit is loaded, but parsing requires the kit. Per-directory manifests avoid this — the manifest is not a `.hnl` file and does not need the language's lexer to read.
