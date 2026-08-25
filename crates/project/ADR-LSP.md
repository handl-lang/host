# ADR: LSP Server as a Tooling Plugin

Status: Accepted

## Context

The original plan described a hardcoded `tool-lsp` crate. The current architecture instead treats LSP support as a set of decoupled tooling plugins that `handl-host-lsp` drives through `handl-tooling`.

## Decision

- LSP features are implemented as separate plugins:
  - `handl-plugin-lsp-completion` — completion candidates.
  - `handl-plugin-lsp-goto` — go-to-definition locations.
  - `handl-plugin-lsp-symbols` — document symbols.
  - `handl-tooling` also includes a small built-in `BasicIncrementalSync` for incremental text sync.
- `handl-tooling` exposes `ToolingHost`, which coordinates parsing, formatting, diagnostics, and the LSP plugin slots.
- `handl-host-lsp` dispatches LSP requests to `ToolingHost` and converts responses to `lsp-types`. It does not know concrete parser or compiler internals.
- The LSP host depends on the canonical expression kit only through `ToolingHost`; alternate dialects can be supported by swapping the kit.
- Formatting and diagnostics are shared with the command-line tooling; they are not duplicated inside the LSP.

## Consequences

- Individual LSP features can be added, removed, or replaced without touching the server crate.
- `handl-host-lsp` stays thin: document store, request mapping, and incremental sync.
- Hover and exact source-position resolution are still placeholders.
