---
title: Shared Diagnostics Crate (`plugin-ext-diag`)
type: adr
date: 2026-10-15
status: implemented
---

# ADR-0091: Shared Diagnostics Crate (`plugin-ext-diag`)

## Status

Implemented (merged to main). Part of the bootstrap tooling strategy
([ADR-0085](./0085-bootstrap-tooling-strategy.md)).

## Context

Diagnostics-adjacent code — `Diagnostic` shapes, spans, severities, fixes,
rendering to `file:line:col` output — is currently scattered: the
expression-level diagnostics live in `plugin-diag-expression`, while fmt,
lint, typecheck, the LSP features, and the CLI each carry their own partial
variants. Every new tool either forks a diagnostics type or reaches into
another crate's internals. The tools in ADR-0086..0090 all need the same
type family, and the CLI (ADR-0090) needs one renderer for all of them.

## Decision

Create `crates/plugin-ext-diag` as the single home for diagnostics
infrastructure, and move/consolidate existing diagnostic code into it:

- **Types:** `Diagnostic { rule, severity, span, message, fix? }`,
  `Severity { Error, Warning, Info }`, `Span` (byte range + line/col),
  `Fix` (text edit). Owned here; every other crate depends on these.
- **Rendering:** one shared renderer (human `file:line:col: severity
[rule]: message`, plus `--json`), used by `host-cli`, the test runner,
  and the LSP adapter — one output format across all tools.
- **Fix application:** span-ordered, idempotent text-edit application with
  the re-parse fixpoint policy from ADR-0087, shared by lint and fmt.
- **Relationship to `plugin-diag-expression`:** it keeps its
  expression-specific rules/production logic but switches to the shared
  `Diagnostic`/`Span` types from `plugin-ext-diag` (a dependency, not a
  consolidation of its rule logic).
- **Generic, not surface-coupled:** the crate knows spans and text, not the
  v0.1 grammar — it serves every handl language.

Migration: introduce the crate with the types above → switch `plugin-ext-fmt`,
`plugin-ext-lint`, `plugin-engine-typecheck`, and `plugin-lib-test` to the
shared types (deprecating their local variants in the same PR that stops
using them) → switch `host-cli` rendering to the shared renderer.

## Consequences

- All tools print diagnostics identically; CI and editor consumers get one
  stable format.
- Cross-tool features become trivial: lint's `fmt` rule, LSP publishing
  lint/typecheck diagnostics, `hnl check` aggregating all sources.
- One more crate in the dependency graph, but it sits at the bottom of the
  tooling stack (depends only on `platform` contracts) and has no plugin
  logic of its own.

## Status

Implemented (merged to main).
