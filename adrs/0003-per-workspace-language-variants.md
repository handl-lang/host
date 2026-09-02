# Per-Workspace Language Variants

## Context

The handl platform defines languages as data: `lexer_rules.yaml`, `tokens.yaml`, `grammar.yaml`, and `ast.yaml`. The LSP server loads these at runtime. ADR 0002 eliminated all static language artifacts from the VS Code extension — all syntax highlighting, snippets, and language configuration flow through the server's data files.

This creates a new capability: **different workspaces (or folders within a workspace) can use different language variants** without requiring different VS Code extensions, different language IDs, or different file extensions.

### The problem

Consider a workspace with multiple projects:

```
workspace/
├── project-a/          ← uses handl v0.1 syntax (loop, match, do, returnA)
│   └── arrows/*.hnl
├── project-b/          ← uses handl v0.2 syntax (rec, while, for, return)
│   └── arrows/*.hnl
```

Both projects use `.hnl` files. VS Code treats them all as language `handl`. The extension has one language ID, one server, one set of providers. It cannot distinguish between the two projects.

Without per-workspace language variants, the server would use a single global set of data files. Changing the syntax for one project would affect all projects. Users would need separate VS Code windows with separate server binaries — fragile and inconvenient.

### Why this matters

The handl language is designed to evolve. The syntax has already changed significantly (e.g., `proc (x) -> body` became `x ~> body`; `//` comments became `--`). Different projects may be at different points in the migration. Some may use experimental syntax variants. Some may use domain-specific extensions (custom combinators, custom keywords).

The platform's data-driven architecture makes this possible — the server just needs to load different data files per workspace. The constraint is not technical; it is a matter of defining the resolution mechanism.

## Decision

Use a `.handl.toml` manifest file at the workspace/folder root to declare which language kit (data files) the server should use for files under that root. The server resolves the kit per file by walking up from the file's path to find the nearest `.handl.toml`.

### The manifest file

A `.handl.toml` file at the root of a handl project:

```toml
# .handl.toml
[language]
kit = "handl-kit-data-canonical"    # crate name or path to data directory
version = "0.1"                      # optional: pin a specific version
```

If `kit` is a crate name, the server loads data from that crate's `data/` directory (resolved via the cargo workspace or a configured search path). If `kit` is a path, the server loads data from that directory directly.

If `version` is specified, the server verifies the kit's version matches. This prevents silent syntax drift when a kit is updated.

### Resolution algorithm

When the server receives a request for a file at path `/workspace/project-a/arrows/sort.hnl`:

1. Start at the file's directory: `/workspace/project-a/arrows/`
2. Walk up one directory at a time, looking for `.handl.toml`
3. At `/workspace/project-a/`, find `.handl.toml`
4. Read the manifest: `kit = "handl-kit-data-canonical"`, `version = "0.1"`
5. Load (or retrieve from cache) the kit's data files: `lexer_rules.yaml`, `tokens.yaml`, `grammar.yaml`, `ast.yaml`
6. Use those data files for all language operations on this file: lexing, parsing, semantic tokens, completion, hover, diagnostics, formatting

If no `.handl.toml` is found by the time the walk reaches the filesystem root, the server falls back to a default kit (the canonical kit compiled into the server).

### Caching

The server maintains a cache of loaded kits, keyed by `(kit_name, version)`. Once a kit is loaded for a workspace, subsequent requests for files in that workspace reuse the cached kit. The cache is invalidated when:

- The `.handl.toml` file changes (detected via the file watcher)
- The server is restarted

### What the extension does

Nothing. The extension:
- Declares language `handl`, extension `.hnl` (static, once)
- Starts the server (once)
- Forwards all requests to the server with the file URI

The server resolves the kit per file. The extension does not know or care which kit is used.

### What VS Code does

VS Code's multi-root workspace support means each folder in a workspace can have its own `.handl.toml`. The server handles each folder independently. No VS Code configuration is needed — the manifest is a server-level concept, not a VS Code setting.

### Example: mixed-version workspace

```
workspace/
├── project-a/
│   ├── .handl.toml              ← kit = "handl-kit-data-canonical", version = "0.1"
│   └── arrows/
│       └── sort.hnl             ← uses v0.1 syntax: loop, match, do, returnA
├── project-b/
│   ├── .handl.toml              ← kit = "handl-kit-data-canonical", version = "0.2"
│   └── arrows/
│       └── sort.hnl             ← uses v0.2 syntax: rec, while, for, return
```

When the user opens `project-a/arrows/sort.hnl`, the server loads v0.1 data. Semantic tokens highlight `loop` as a keyword. Completion offers `loop` snippets.

When the user opens `project-b/arrows/sort.hnl`, the server loads v0.2 data. Semantic tokens highlight `rec` as a keyword (and `loop` is not a keyword). Completion offers `rec` snippets.

Both files are language `handl`, extension `.hnl`, same extension, same server. The server transparently handles the variant.

### Example: custom domain extension

```
workspace/
├── data-pipeline/
│   ├── .handl.toml              ← kit = "./custom-kit"
│   ├── custom-kit/
│   │   ├── lexer_rules.yaml     ← adds custom keywords: stream, sink, source
│   │   ├── tokens.yaml
│   │   └── grammar.yaml
│   └── arrows/
│       └── ingest.hnl           ← uses custom syntax with stream/sink/source
```

The server loads the custom kit's data files. `stream`, `sink`, and `source` are highlighted as keywords. Completion offers snippets for them. Diagnostics parse using the custom grammar. No extension changes, no server recompilation — just different data files.

## Alternatives Considered

### Alternative A: VS Code workspace settings

Use `handl.kit` setting in `.vscode/settings.json` per folder.

**Rejected because:**
- VS Code settings are per-folder but the server receives file URIs, not settings. The server would need to query VS Code for per-folder settings via a custom protocol, adding complexity.
- `.handl.toml` is a language-level concept, not an editor concept. It should work the same in VS Code, Neovim, Emacs, or any other editor. Tying it to VS Code settings breaks editor portability.
- The server can resolve `.handl.toml` from the file path alone, with no editor cooperation.

### Alternative B: File-level markers

Use a comment or directive at the top of each `.hnl` file to declare the kit.

**Rejected because:**
- Requires modifying every file. Adding a new file means remembering to add the directive.
- The directive itself must be parsed before the kit is loaded — a chicken-and-egg problem. If the directive uses v0.2 syntax but the server is using v0.1 data, it may not parse.
- Does not scale to large projects with hundreds of files.

### Alternative C: Single global kit via server flag

Pass `--kit handl-kit-data-canonical` as a server launch argument.

**Rejected because:**
- One server instance serves the entire VS Code workspace. A multi-root workspace with different projects cannot use different kits.
- Requires restarting the server to change kits.
- Does not support per-folder variants.

### Alternative D: Separate language IDs per variant

Register `handl-v1` and `handl-v2` as separate language IDs in `package.json`.

**Rejected because:**
- Requires repackaging the extension for every new variant.
- VS Code language IDs are static — cannot be added at runtime.
- Users must manually associate file extensions with the right language ID.
- Fragments the ecosystem — tooling, snippets, themes must be duplicated per variant.

## Consequences

### Positive

- **Multiple language variants in one workspace.** Different folders can use different syntax without separate extensions or language IDs.
- **Editor-agnostic.** `.handl.toml` is a language-level concept. The same mechanism works in VS Code, Neovim, Emacs, or any LSP client.
- **No extension changes.** Adding a new variant is a data file + `.handl.toml` change. The extension is not involved.
- **Custom domain extensions.** Projects can define custom keywords and operators without forking the server or extension.
- **Version pinning.** The `version` field prevents silent syntax drift when a kit is updated.
- **Caching.** Kits are loaded once per workspace and reused, with invalidation on manifest change.

### Negative

- **Server must implement kit resolution.** Walking the filesystem to find `.handl.toml` and loading data files adds complexity to the server. This is a one-time implementation cost.
- **Filesystem access required.** The server must read `.handl.toml` and data files from disk. This works for local files but may need adaptation for remote/virtual filesystems (VS Code Remote, Live Share). Mitigated by the fallback to the default kit when no manifest is found.
- **Cache invalidation.** If a user edits `.handl.toml` or data files without the server's file watcher detecting it, the server may use stale data. Mitigated by the existing file watcher (already implemented for `.hnl` files) — it can be extended to watch `.handl.toml` and `*.yaml` files.
- **No per-file variants.** The manifest is per-directory, not per-file. Two files in the same directory must use the same kit. This is intentional — per-file variants would be confusing and hard to tool.

### Neutral

- **`.handl.toml` is a new convention.** Projects that don't include one get the default kit. No migration required for existing projects.
- **Server startup time** is unaffected — kits are loaded lazily on first request for a file in that workspace.

## Status

Proposed
