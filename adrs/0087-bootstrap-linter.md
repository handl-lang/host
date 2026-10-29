---
title: Bootstrap Linter (`plugin-ext-lint`)
type: adr
date: 2026-10-15
status: implemented
---

# ADR-0087: Bootstrap Linter (`plugin-ext-lint`)

## Status

Implemented (merged to main). Part of the bootstrap tooling strategy
([ADR-0085](./0085-bootstrap-tooling-strategy.md); refines the generic
design in [ADR-0015](../0015-linter.md) — which stays valid — into a
concrete implementation). Temporary: will be replaced by the
self-hosted linter described in
[RFC-0003](../rfcs/0003-linter-in-handl.md).

## Context

The stdlib will be the first large body of arrow-surface handl code
(ADR-0077). Writing it without a linter means conventions get enforced by
review instead of by tooling. The generic rule-registry linter of ADR-0015
was never implemented against the arrow AST.

## Decision

The linter work lands in `plugin-ext-lint`: a **canonical rule set** over
the v0.1 surface AST (`handl-language` AST via `plugin-engine-parse`),
running on the crate's existing generic rule/capability registry. No config
files. Diagnostics use the shared types from `plugin-ext-diag` (ADR-0091).

- **Core types:** re-exported from `plugin-ext-diag` (ADR-0091) — this ADR
  fixes their canonical shape:

  ```rust
  pub struct Diagnostic {
      pub rule: &'static str,          // e.g. "unused-import"
      pub severity: Severity,          // Error | Warning | Info
      pub span: Span,                  // byte range + line/col
      pub message: String,
      pub fix: Option<Fix>,            // text edit, applied by `hnl lint --fix`
  }
  ```

- **Initial rule set** (all v0.1-surface-specific):
  - `unused-import` (Warning, fixable) — import projection never referenced.
  - `unbound-wire` (Error) — reference to a name not bound in the enclosing
    graph block or imports.
  - `shadowed-binding` (Warning) — re-binding a name already in scope.
  - `dead-branch` (Warning) — `choice` arm guarded by a constant condition.
  - `naming-convention` (Warning, fixable) — bindings are `snake_case`,
    arrows/types are `kebab-case` per the ADR-0074 spec.
  - `missing-module-doc` (Info) — a module lacks a leading doc comment.
  - `fmt` (Warning, fixable) — file is not canonical per ADR-0086; the fix
    applies the formatter's public output from `plugin-ext-fmt`.
- **Exit contract:** exit 1 iff any `Error` diagnostic; `--fail-on-warn`
  escalates. `--fix` applies fixable diagnostics in span order, re-parses,
  and re-runs to a fixpoint (max 5 iterations, then reports unfixable).
- **Rule architecture:** rules are pure functions
  `(ast, token_map) -> Vec<Diagnostic>` registered through the existing
  generic rule/capability contracts (`plugin-ext-lint`), so hosts can
  contribute additional namespaced rules without touching this crate. The
  _canonical_ rule set above is hardcoded here; the _machinery_ (registry,
  severity, fix application) is the generic substrate.
- **Testing:** 100% coverage; every rule has positive/negative/fix cases;
  lint-clean gate over the repo's own `.handl` files in CI.

Out of scope: user-defined severity config, cross-module analysis (both
RFC-0003). Host-contributed rules remain possible _now_ through the
existing `plugin-ext-lint` registry.

## Consequences

- The stdlib lands with enforced conventions from day one.
- Rules live in Rust until RFC-0003; adding a rule is a code change plus
  tests, reviewed like any language change.
- `unbound-wire` deliberately duplicates a subset of typechecker work in
  v0.1; when the bootstrap typechecker (ADR-0088) matures, the rule delegates to
  it instead of re-implementing scope resolution.

## Status

Implemented (merged to main).
