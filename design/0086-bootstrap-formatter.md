---
title: Bootstrap Formatter (`plugin-ext-fmt`) — design detail
type: design
date: 2026-10-15
updated: 2026-10-29
refs: [ADR-0086]
---

# Bootstrap Formatter (`plugin-ext-fmt`) — design detail

Moved verbatim from ADR-0086 (ADR-0161 size budget). The ADR carries the rules;
this page carries the context, tables, phases and trade-offs.

## Context

The v0.1 arrow surface syntax (ADR-0074) is now being used to write the
handl stdlib and examples. There is no formatter for it. The generic,
plugin-driven formatter of ADR-0014 was never implemented and presumes a
pre-arrow document model. We need a real formatter now, coupled to the one
syntax that exists.

## Decision

`plugin-ext-fmt` is the canonical formatter: a Rust implementation, built
on the generic format capability contracts, that formats `.handl` files
written in the v0.1 surface syntax.

- **Input/output:** surface source text → parse (via
  `plugin-engine-parse` over the `handl-language` AST) → canonical layout →
  source text. Formatting is AST-driven; comments and tokens are preserved
  via the token stream, not reflowed.
- **Hardcoded style, no configuration.** Canonical preferences:
  1. Two-space indentation; continuation lines align with enclosing arrows.
  2. One statement per line inside graph blocks; a blank line between
     top-level declarations.
  3. Single trailing newline; no trailing whitespace; no more than one
     consecutive blank line.
  4. `>>>` chains break _before_ the operator, one segment per line, when a
     chain does not fit in 100 columns.
  5. No spaces inside brackets; single spaces around binary operators per
     the ADR-0074 precedence table.
- **Idempotence:** `fmt(fmt(src)) == fmt(src)` — tested exhaustively.
- **API surface (minimal):**
  - `format_source(src: &str) -> Result<String, FormatError>`
  - `check_source(src: &str) -> Result<bool, FormatError>` (true = already
    canonical)
- **CLI contract:** `hnl fmt [--check] [--write] [paths...]`. `--check`
  exits 1 with a diff on stdout if any file is not canonical; `--write`
  rewrites files atomically. Globbing/paths are the caller's job.
- **Testing:** 100% coverage. Golden-file tests under `tests/golden/` (one
  input, one expected output per construct in ADR-0074), plus an idempotence
  property test over all goldens and the repo's own `.handl` examples.

Explicitly out of scope: configuration files, formatting other dialects,
comment reflow, plugin-contributed rules (all RFC-0002).

## Consequences

- stdlib and example code get canonical formatting immediately; CI can gate
  with `hnl fmt --check`.
- Style changes require editing Rust and bumping goldens — acceptable until
  RFC-0002.
- The crate must track ADR-0074 grammar changes (`ARROW_GRAMMAR_V0_1.md`);
  parser changes are a hard dependency, so `plugin-ext-fmt` pins the workspace
  `plugin-engine-parse` version and CI fails loudly on drift. The formatter
  consumes and produces the shared diagnostics types from `plugin-ext-diag`
  (ADR-0091).
