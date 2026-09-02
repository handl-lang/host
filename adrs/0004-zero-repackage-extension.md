# Zero-Repackage Extension

## Context

ADR 0002 eliminated all static language artifacts (TextMate grammar, snippets, language configuration) from the VS Code extension. ADR 0003 introduced `.handl.toml` for per-workspace language variants.

The remaining extension rebuild triggers are VS Code platform requirements:
1. New file extension
2. New language ID
3. New config setting

Each has a workaround.

## Decision

Apply all three workarounds. The extension is never repackaged for language evolution.

### 1. File extensions via `files.associations`

The extension declares `.hnl` in `package.json` as the default. Additional extensions are mapped at runtime via `files.associations`, read from `.handl.toml`:

```toml
# .handl.toml
[language]
extensions = [".hnl", ".hn2", ".handl"]
```

```typescript
// extension.ts at activation
const manifest = readHandlToml(workspaceRoot);
const config = vscode.workspace.getConfiguration("files");
await config.update("associations", Object.fromEntries(
  manifest.language.extensions.map(ext => [`*${ext}`, "handl"])
), false);
```

No `package.json` edit. No repackage.

### 2. Language ID — not needed

ADR 0003 uses one language ID (`handl`) for all variants. The server differentiates via `.handl.toml`. A new language ID is never needed — a fundamentally different language would be a different extension.

### 3. Config via `.handl.toml` + `initializationOptions`

Remove `contributes.configuration` from `package.json`. Config lives in `.handl.toml`:

```toml
# .handl.toml
[editor]
enableInlayHints = true
enableCodeLens = true
enableRefinementDiagnostics = true
enableSemanticHighlighting = true
```

The extension reads `.handl.toml` and passes values to the server via `initializationOptions` — which accepts arbitrary JSON with no declaration needed:

```typescript
const manifest = readHandlToml(workspaceRoot);
const clientOptions = {
  initializationOptions: manifest.editor,
  // ...
};
```

Tradeoff: no VS Code Settings UI for handl. Users edit `.handl.toml` instead. This is more consistent — all language config in one place.

### Result

```
package.json contributes:
  languages: [{ id: "handl", extensions: [".hnl"] }]   ← frozen, never changes
  configuration: (nothing)                               ← .handl.toml
  grammars: (nothing)                                    ← semantic tokens
  snippets: (nothing)                                    ← server completion
```

The extension is repackaged only for VS Code API breaking changes or extension-level features (commands, views, menus). Language evolution never touches it.

## Consequences

### Positive
- Extension is never repackaged for language changes
- All config in `.handl.toml` — single source of truth, editor-agnostic
- File extensions are per-workspace, not per-extension

### Negative
- No VS Code Settings UI for handl config
- `files.associations` update is per-workspace (must be set in each workspace)
- `.handl.toml` must be read before client start (adds ~1ms to activation)

## Status

Proposed
