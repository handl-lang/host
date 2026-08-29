# ADR-LSP (Remaining): Hover and Exact Source-Position Resolution

## Status

Proposed

## Context

The LSP Server as a Tooling Plugin ADR has been implemented and archived. The current LSP support is provided by separate plugins (`handl-plugin-lsp-completion`, `handl-plugin-lsp-goto`, `handl-plugin-lsp-symbol`) and a thin `handl-host-lsp` that dispatches through `ToolingHost`.

Hover and exact source-position resolution are still placeholders.

## Decision

Implement hover and exact source-position resolution as tooling plugins:

- `handl-plugin-lsp-hover` — hover/type-at-point information.
- `handl-plugin-lsp-source-position` — precise go-to-definition and source-location resolution for cursor positions.

Both plugins will follow the same pattern as the existing completion, goto, and symbol plugins: they contribute LSP rules through `handl-tooling` and return `lsp-types` responses via `handl-host-lsp`.

## Consequences

- LSP users get hover tooltips and reliable go-to-definition/source-position resolution.
- `handl-host-lsp` remains a thin dispatcher.
- New LSP features can still be added or replaced without changing the server crate.

## Related work

- `project/adrs/archived/ADR-LSP.md`
