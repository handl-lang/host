# RFC-0006: LSP Server in handl

## Status

Low priority. The hardest self-hosting target, intentionally last. Builds on
the handl plugin system ([RFC-0001](0001-plugin-system-in-handl.md)) and the
handl-native tools ([RFC-0002](0002-formatter-in-handl.md),
[RFC-0003](0003-linter-in-handl.md), [RFC-0004](0004-typechecker-in-handl.md)).
Replaces the Rust `host-lsp` adapter and the `plugin-lsp-*` feature crates.

## Summary

A handl-implemented LSP server: an event loop over editor messages, with
analysis features implemented as handl arrows over document state. Editor
integrations (VS Code extension, tree-sitter/TextMate grammars) remain
external artifacts in every scenario and are unaffected by this RFC.

## Motivation and constraint

An LSP server is stateful and latency-critical: incremental sync, document
caches, request cancellation, debouncing. Handl's kernel has no mutation
primitive — which is the right constraint, not an obstacle: **document
state is a fold**. The server state is the result of folding the message
stream over an initial state, exactly the `loop` feedback pattern of
ADR-0052/0072. This section exists to make that explicit up front, because
it is the design decision everything else depends on.

## Detailed design

- **Session = fold.** `session = loop (handle-message)`: state is an
  immutable record `{ open-docs, caches, version }`; every notification
  produces a new state. No shared mutable store; the fold gives crash-safe
  semantics for free.
- **Features as arrows:** completion, goto-definition, document symbols,
  hover are arrows `{ state, position } ~> FeatureResult` — these are the
  same analysis descriptions the CLI uses, so the LSP and CLI cannot drift.
- **The Rust boundary stays thin.** JSON-RPC framing, stdio transport, and
  cancellation tokens are host concerns — a small Rust adapter (or any
  other language's adapter) that decodes LSP messages into handl values and
  encodes results back. The *server logic* is handl; the *transport* is
  not. This keeps the difficult part (integration plumbing) in the mature
  ecosystem while the language-agnostic logic self-hosts.
- **Latency budget:** analysis features must run on the compiled backend;
  debouncing and incremental re-analysis (only the changed module plus its
  reverse-dependency set, per the ADR-0066 dependency-graph chunking) are
  runner-level policies expressed as arrows.
- **Incremental sync:** text deltas are applied by pure arrows to the
  document value, then re-lex/re-parse only the affected spans.

## Migration

1. Ship the Rust `host-lsp` as-is; add a handl analysis backend behind the
   same `ToolingHost` interface so editor users get handl-typed results
   early.
2. Move the message-handling fold into handl behind the Rust transport.
3. Retire the `plugin-lsp-*` feature crates and `host-lsp`'s embedded
   analysis (Rust versions remain the fallback per ADR-0085 rule 6); keep
   only the transport
   adapter.

## Drawbacks

- Blocked on compiled evaluation being production-ready; interpreted
  analysis on keystrokes will not meet LSP latency expectations.
- A handl server bug degrades every editor session simultaneously; the
  Rust fallback path is kept behind a flag during migration.

## Related

- ADR-LSP (language-agnostic LSP host), ADR-0066 (dependency-graph
  chunking), ADR-0072 (concurrent arrows), RFC-0001.
