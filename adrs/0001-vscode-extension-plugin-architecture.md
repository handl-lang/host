# VS Code Extension Modularization

## Context

The handl VS Code extension (`editor-support/vscode/src/`) currently consists of two source files:

- `extension.ts` (123 lines) — a monolithic `activate()` function that handles server path resolution, LanguageClient creation, formatting provider registration, config reading, error handling, and logging.
- `server.ts` (74 lines) — server path resolution and constants.

This structure has several problems that will worsen as features are added:

1. **Monolithic entry point.** `activate()` does everything: path resolution, client creation, provider registration, config reading, error handling, and logging. Adding any feature requires editing this function.

2. **Formatting provider registered inside a `.then()` callback.** If the client starts slowly or fails, the provider may not register at all. The registration is also asynchronous relative to other features, making ordering unpredictable.

3. **No dependency injection for `extension.ts`.** Only `server.ts` is unit-testable (via `ResolveDeps`); `extension.ts` has zero test coverage because it imports `vscode` directly and constructs the client inline.

4. **Untyped config.** Settings are read inline with `config.get("...", true)` — defaults are duplicated between `extension.ts` and `package.json`, with no compile-time guarantee they match.

5. **Inconsistent error handling.** Three separate try/catch blocks with different error messages and severity levels.

6. **Logging goes nowhere.** `console.log` calls are scattered throughout; output goes to the developer console, not the VS Code Output panel.

The extension currently supports 35 LSP features (all handled by the Rust server and auto-registered by `vscode-languageclient`). The TypeScript side does three things: find the server binary, start the client, and register one custom formatting provider. More client-side features may be added later (status bar, command palette commands, custom views), but none are confirmed.

## Decision

Split the monolith into focused modules. Use existing `vscode` and `vscode-languageclient` primitives instead of building custom infrastructure.

### Directory Structure

```
src/
├── extension.ts       — thin orchestrator: create client, register features, handle errors
├── config.ts          — typed config reader with single source of truth for defaults
├── client.ts          — LanguageClient factory + lifecycle (start, stop, onCrash)
├── formatting.ts      — DocumentFormattingEditProvider (extracted from activate())
├── server.ts          — (existing, unchanged)
└── *.test.ts          — co-located tests
```

### Module Responsibilities

#### `extension.ts` (~30 lines)

Thin entry point. Resolves config, creates the client, registers features, handles errors. No business logic.

```typescript
export async function activate(ctx: vscode.ExtensionContext): Promise<void> {
  const config = readConfig();
  const serverPath = resolveServerPath(depsFromContext());
  if (!serverPath) {
    vscode.window.showWarningMessage("handl: server binary not found...");
    return;
  }

  const client = createClient(serverPath, config);
  ctx.subscriptions.push(client);

  try {
    await client.start();
  } catch (err) {
    vscode.window.showErrorMessage("handl: " + errorMessage(err));
    return;
  }

  registerFormatting(ctx, client);
}
```

#### `config.ts` (~40 lines)

Single source of truth for config shape and defaults. Reads from `vscode.workspace.getConfiguration("handl")` and returns a typed object. Exports a `CONFIG_DEFAULTS` constant that can be checked against `package.json` in a test.

```typescript
export interface HandlConfig {
  readonly serverPath: string;
  readonly enableInlayHints: boolean;
  readonly enableCodeLens: boolean;
  readonly enableRefinementDiagnostics: boolean;
  readonly enableSemanticHighlighting: boolean;
}

export const CONFIG_DEFAULTS: HandlConfig = {
  serverPath: "",
  enableInlayHints: true,
  enableCodeLens: true,
  enableRefinementDiagnostics: true,
  enableSemanticHighlighting: true,
};

export function readConfig(): HandlConfig { ... }
```

#### `client.ts` (~50 lines)

Factory for `LanguageClient`. Takes `serverPath` and `config`, returns a configured `LanguageClient`. Uses `vscode-languageclient`'s built-in middleware option for any cross-cutting concerns (logging, error handling). No custom middleware framework.

```typescript
export function createClient(serverPath: string, config: HandlConfig): LanguageClient {
  return new LanguageClient("handl", "handl", serverOptions, clientOptions(config));
}
```

#### `formatting.ts` (~30 lines)

The formatting provider, extracted from `activate()`. Exports a `registerFormatting` function that creates the provider and pushes it to `context.subscriptions`.

```typescript
export function registerFormatting(ctx: vscode.ExtensionContext, client: LanguageClient): void {
  const provider: vscode.DocumentFormattingEditProvider = { ... };
  ctx.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(DOCUMENT_SELECTOR, provider),
  );
}
```

#### `server.ts` (unchanged)

Existing server path resolution and constants. Already well-structured and tested.

### What we are NOT building

- **No DI container.** Function parameters are sufficient. The existing `ResolveDeps` pattern in `server.ts` is the right level of indirection.
- **No custom event bus.** Use `vscode.EventEmitter` / `vscode.Event` if cross-feature communication is needed later.
- **No custom cache layer.** The LSP protocol has document version tracking; `vscode-languageclient` handles request deduplication. Caching belongs in the server, not the client.
- **No custom health check.** `vscode-languageclient` detects server crashes. If auto-restart is needed later, it has a `connectionOptions` restart strategy.
- **No middleware framework.** `vscode-languageclient` has built-in per-method middleware via `LanguageClientOptions.middleware`.
- **No feature registry.** With 1-2 client-side features, a function call per feature is simpler and more readable than a registry.

### Testing Strategy

- `config.ts` — test that `readConfig()` returns correct values for given config states, and that `CONFIG_DEFAULTS` matches `package.json` defaults.
- `client.ts` — test that `createClient()` produces a client with correct options (server path, document selector, initialization options).
- `formatting.ts` — test that `registerFormatting()` registers a provider that sends the correct LSP request.
- `server.ts` — (existing tests, unchanged).
- `extension.ts` — integration-tested via the existing LSP conformance suite (16 tests).

Co-located test files: `config.test.ts`, `client.test.ts`, `formatting.test.ts`, `server.test.ts`.

### Migration

Single commit. The refactor is small enough to do in one pass:

1. Create `config.ts`, `client.ts`, `formatting.ts`.
2. Rewrite `extension.ts` to use them.
3. Add tests for the new modules.
4. Verify existing `server.test.ts` still passes.
5. Verify LSP conformance suite still passes.

## Alternatives Considered

### Alternative A: Plugin-based architecture with DI container

Full plugin framework with dependency injection container, event bus, error boundaries, cache layer, health check, and middleware pipeline (~30 files).

**Rejected because:**
- The extension does 3 things; 30 files is not justified.
- Reinvents `vscode` and `vscode-languageclient` primitives (`context.subscriptions` is already a disposable pool; `vscode-languageclient` already has middleware and crash detection).
- DI containers with typed tokens are not idiomatic TypeScript.
- Adds indirection without proportional benefit at current scale.
- YAGNI — most proposed abstractions have no confirmed consumer.

### Alternative B: Keep the monolith

Leave `extension.ts` as-is.

**Rejected because:**
- Zero test coverage on the entry point.
- Formatting provider registration is timing-sensitive (inside `.then()`).
- Config defaults duplicated between code and `package.json`.
- Adding the next client-side feature (e.g. status bar) would make it worse.

### Alternative C: Split into 2 files only (extension + config)

Extract just the config, leave the rest in `extension.ts`.

**Rejected because:**
- Does not solve the testability problem for client creation and formatting.
- `extension.ts` would still be ~90 lines doing 3 unrelated things.

## Consequences

### Positive

- **Each module has one responsibility** and is independently testable.
- **Config defaults** are defined once in `config.ts` and verified against `package.json` in a test.
- **Formatting provider** is registered synchronously after client start, not inside a `.then()` callback.
- **Error handling** is centralized in `extension.ts` with one consistent pattern.
- **Adding a feature** means adding a module + one function call in `extension.ts`. Not zero-edit, but minimal and obvious.
- **No new abstractions to learn.** Contributors read 5 small files, not a framework.

### Negative

- **`extension.ts` still imports `vscode` directly** and is not unit-testable in isolation. Mitigated by the conformance suite covering it end-to-end.
- **Adding a 3rd or 4th client-side feature** will make `extension.ts` grow. If it exceeds ~50 lines, consider extracting a feature list at that point — not before.

### Neutral

- **Bundle size and startup time** are unchanged — same code, different files.
- **No runtime behavior change** — the refactor is purely structural.

## Status

Proposed
