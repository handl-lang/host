# ADR-0095: Formatter as a Production Pipeline (`FormatEngine` stage contracts)

## Status

Proposed. Refines [ADR-0086](./0086-bootstrap-formatter.md) and the generic
design in [ADR-0014](./0014-formatter.md) (both stay valid) by applying the
production-pipeline discipline [ADR-0089](./0089-bootstrap-test-runner.md)
brought to the test runner. An instance of rule 2 of
[ADR-0085](./0085-bootstrap-tooling-strategy.md) ("best practices in Rust,
now") and of the 2026-09 amendment ("the Rust substrate is kept and improved,
not deleted"). The future handl port remains
[RFC-0002](../rfcs/0002-formatter-in-handl.md).

> **Scope.** This ADR governs the **format engine** — the Rust machinery that
> turns source text into canonical source text (parse → layout → render →
> post-process). It does **not** change the canonical v0.1 style
> (ADR-0074/ADR-0086) nor the CLI contract (`hnl fmt [--check] [--write]`).

## Context

The test-runner rewrite in [ADR-0089](./0089-bootstrap-test-runner.md)
named the anti-pattern it was avoiding: **building a toy and bolting
production features on later** is exactly what ADR-0085 rule 2 exists to
prevent, and a handl port is only low-risk if the Rust original is already
structured the way the handl version will be. The formatter is today in the
shape ADR-0089's Context criticized:

1. **The canonical layout is a hardcoded monolith.** In
   `plugin-ext-fmt/src/lib.rs`, `format_source` is one function chain —
   `parse → lex → format_tokens → post_process` — over 500+ lines of inline
   rules: the `TokenCat` enum, `needs_space`, `indent_str`, `post_process`,
   and `decl_type`. Every canonical-style rule (spacing, chain-breaking,
   blank-line policy, trailing-NL) is baked into that walk. There are no
   stage boundaries and no seams. A style change is surgery on the chain.
2. **The generic formatter capability is disconnected.** `crates/platform/
   src/format.rs` ships `BasicFormatter` / `FormatBehavior` / `FormatRequest`
   — ADR-0014's "generic core, pluggable rules" — but they key rules by
   `AstNode.kind`, operate on a single node payload, and are exercised only by
   echo-style unit tests. The real canonical formatter never goes through
   them. ADR-0014's generic purpose was never connected to ADR-0086's
   concrete implementation.
3. **Two engines, not a pipeline.** `plugin-engine-fmt` already has a
   data-driven AST→source emitter (`FormatEngine` + a `NodeEmitter` trait:
   `PlainEmitter` / `PreserveEmitter`, per ADR-0044). The token-stream layout
   in `plugin-ext-fmt` reimplements the leaf/spacing decisions separately.
   They are alternatives, not stages of one pipeline.
4. **Future work is a catch-all, not a contract.** ADR-0086 deferred config,
   plugin-contributed rules, and comment reflow to RFC-0002. That deferral is
   fine — but without stage/rule boundaries in Rust now, RFC-0002 stays a
   rewrite instead of becoming the port ADR-0085 rule 6 wants.

What the formatter already has that the runner did not: a real generic
parse/lex substrate (reused, not owned), a data-driven AST emitter with a
`NodeEmitter` seam, and a golden corpus + idempotence test suite. So this is
primarily a **land-the-boundaries** change, not a rebuild.

## Decision

Refactor the formatter into a **production pipeline with stable contracts
between stages** — `parse → layout → render → post-process` — land the
boundaries now, and ship the existing implementations behind them. Future
features (config, plugin rules, a handl engine, comment reflow, incremental
LSP formatting) become new implementations/compositions, not architectural
surgery.

### Crate boundary

| Crate                           | Role                                                                         | Fate                                                  |
| ------------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------- |
| `plugin-engine-fmt` (**core**)  | Format **engine**: the staged pipeline plus the `NodeEmitter` and `FormattingRule` seams. Language-generic (ADR-0044/ADR-0041). | Improved in place; hosts the pipeline and traits.    |
| `plugin-ext-fmt` (**surface**)  | Canonical v0.1 **surface**: assemble the canonical kit, select the canonical rule set, expose `format_source` / `check_source` / `format_ast` as thin wrappers + CLI contract + golden/idempotence tests. | Slimmed to a thin wrapper over the engine pipeline.  |
| `crates/platform/format.rs`     | The generic `Format` capability (`BasicFormatter` / `FormatBehavior`).        | **Decision point below.**                             |

### Pipeline and stage contracts

```
Source text
   │  parse                          (reuse lex + Pratt; data-driven, ADR-0044)
   ▼
[tokens, AST]                        ── stage 1 · parse
   │  layout
   ▼
LayoutStream (breaks / indents / spacing, per-rule decisions)
                                     ── stage 2 · layout
   │  render
   ▼
rendered text                        ── stage 3 · render   (Plain | Preserve, via NodeEmitter)
   │  post-process
   ▼
canonical source                     ── stage 4 · post-process (blank-lines, trailing NL/WS)
```

Each stage has a typed input/output contract. The initial implementation of
each stage is exactly the logic that exists today, moved behind the boundary:

- **parse** — reuse `plugin-engine-lex-*` and `plugin-engine-parse-pratt`
  (already generic; not reimplemented).
- **layout** — `needs_space` / indentation / chain-breaking move from the
  monolith into composable rules.
- **render** — already exists as `FormatEngine` + `PlainEmitter` /
  `PreserveEmitter` in `plugin-engine-fmt`.
- **post-process** — `post_process` / `decl_type` (blank-line and trailing
  policy) move behind the boundary.

### Rule seam: `FormattingRule` (replaceable, composable)

Introduce `pub trait FormattingRule` in `plugin-engine-fmt`. Each
canonical-style rule (indentation, chain-breaking, blank-line policy,
spacing) is a separate implementation behind the trait, selected/composed by
the surface rather than hardcoded inside one walk — the formatter analog of
the test runner's `TestProvider` seam (ADR-0089 §"Providers (extensible from
day one)"). Disabling or reordering a rule is composition, not surgery; a
new style for another handl dialect is a new rule set, not a core change.

### Config seam

`FormatOptions` (already a `{ indent_size }` record) becomes the config
record RFC-0002 specifies (`{ indent, width, ... }`), threaded as pipeline
**input** — never read from a file by the engine. A later `hnl` config layer
supplies overrides without changing the engine.

### Engine seam: `NodeEmitter` stays the swap point

The existing `NodeEmitter` trait (`PlainEmitter` / `PreserveEmitter`) remains
the engine boundary. The LSP incremental path (RFC-0002) and the CLI
normalizing path share one contract; a future handl / compiled engine is a
new `NodeEmitter` implementation, keeping RFC-0002 a port.

### Generic capability: connect or retire `platform/format.rs`

`BasicFormatter` / `FormatBehavior` must be one of:

1. **Wired** to the real engine — `BasicFormatter` dispatches to the
   `FormatEngine` pipeline so ADR-0014's "generic core, pluggable rules" is
   real and the plugin-registry path formats actual `Ast` / `Language` input;
   or
2. **Retired** — documented explicitly that `FormatEngine` /
   `plugin-ext-fmt`'s direct API is the canonical path and the `Format`
   capability is deprecated for host use.

Leaving it disconnected (today's state) is rejected: it preserves ADR-0014's
facade while guaranteeing drift between the generic and canonical paths.

### Language-as-data style

Continue the discipline `classify_token` already applies (`TokenKindDef`
`category` / `label`, ADR-0044): spacing and line-break decisions consult
`Language` data rather than hardcoded spellings, so other handl languages get
a formatter for free (ADR-0041 "one formatter formats any language you have a
value for").

### Performance

Three levers fall out of the stage boundaries, mirroring the runner's
scheduler-based parallelism:

1. **Parallel `--write` / `--check`** across many files via
   `plugin-lib-scheduler::BoundedScheduler` (reused, as in ADR-0089).
2. **Stage-boundary caching** — a content-hash key on the parse stage
   (reusing `plugin-exec-cache`'s `CacheControl` idea); only re-layout /
   re-render changed files.
3. **Incremental per-document formatting** for the LSP, enabled by the
   existing `PreserveEmitter` stage.

Determinism: parallelism / caching change timing, never output ordering —
the ADR-0089 guarantee, applied to formatting.

### Public API (preserved)

```rust
pub fn format_source(src: &str) -> Result<String, FormatError>;
pub fn check_source(src: &str) -> Result<bool, FormatError>;
pub fn format_ast(lang: &Language, ast: &Ast, root: AstRef) -> Result<String, LangError>;
```

The existing names stay as thin wrappers over the pipeline for callers.
`FormatError` and diagnostics integration (ADR-0091 `plugin-ext-diag`) are
unchanged.

### Testing

100% coverage (ADR-0085 rule 4). Keep the golden corpus
(`plugin-ext-fmt/tests/golden/`) and the idempotence property test. Add
per-stage contract unit tests and property tests asserting idempotence and
determinism as stage laws (`fmt(fmt(src)) == fmt(src)`; parallel output ==
serial output).

## Consequences

- **The formatter is production-architected, not a throwaway.** RFC-0002
  becomes a port of already-generic, well-bounded Rust code — the lowest-risk
  kind of port per ADR-0085 rule 6.
- **Style is composable, not monolithic.** Changing one style rule is editing
  one rule implementation + bumping goldens, not surgery on a 500-line walk.
- **RFC-0002's deferrals become seams.** Config, plugin rules, comment
  reflow, incremental LSP formatting each map to a stage or rule contract.
- **The generic capability is either real or honestly retired** — no more
  facade drift between `platform/format.rs` and the canonical path.
- **Performance and determinism are guaranteed**, not incidental.
- **Generic infrastructure is reused, not reinvented** (scheduler, cache,
  lex/parse engines, diagnostics) — the engine owns only formatting glue.
- **Other handl languages gain a formatter** as the style follows `Language`
  data.

## Prerequisites

1. Extract `needs_space` / indentation / chain-breaking into `FormattingRule`
   implementations in `plugin-engine-fmt`; keep the current behavior
   byte-identical on the golden corpus.
2. Move `post_process` / `decl_type` behind the post-process stage contract.
3. Decide and execute the `platform/format.rs` option (connect or retire).
4. Keep 100% coverage and the golden + idempotence suites green through each
   step.
5. No change to ADR-0086's canonical style or the `hnl fmt` CLI contract.

## Status

Proposed. Companion to the test-runner rewrite (ADR-0089) applying the same
pipeline discipline to the formatter; the handl port is RFC-0002.