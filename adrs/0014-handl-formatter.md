+++
id = "ADR-0014"
title = "handl Formatter"
status = "open"
priority = "medium"
+++

---
title: handl Formatter
type: adr
date: 2026-08-24
status: proposed
status_note: Proposed
---


# handl Formatter

## Decision


The formatter is a **tool written in handl itself**, not a host-level Rust
command. It is generic (parameterized by `Language`, per ADR-0041 Phase 1:
"one formatter formats any language you have a value for"), extensible via
plugins, and modular (formatting rules are contributed by separate plugins
rather than monolithic).

### Generic core, pluggable rules

The platform provides the generic formatter primitives — the `Format`
operation, `FormatBehavior` trait, `FormatRequest`, and `BasicFormatter`
rule acceptor in `handl-platform` (re-exported via `plugin-ext-fmt`). These
are the contract; they carry no language-specific formatting logic.

Formatting rules are contributed by plugins that implement `FormatBehavior`,
keyed by `AstNode.kind`. A rule for `if`-expressions, a rule for `import`
statements, a rule for object literals — each is a separate, composable
plugin. Hosts and kits assemble the rule set they want; the canonical kit
ships the canonical handl rules.

### Command surface

```bash
handl fmt document.yaml              # print formatted output to stdout
handl fmt document.yaml --write      # overwrite the file in place
handl fmt document.yaml --check      # exit non-zero if not already formatted
handl fmt libraries/**/*.yaml --write  # format many files
```

### Canonical style

1. **Two-space indentation** (YAML, `.handl`). JSON and TOML follow their
   native conventions.
2. **A single trailing newline** at the end of each file.
3. **No trailing whitespace** on any line.
4. **Canonical key order for mappings.** Unknown keys are kept after known
   keys in their original relative order.
   - Document top-level: `annotations`, `version`, `import`, `fields`,
     `functions`, `data`.
   - Field mapping: `type`, `expr`, `arg`, `schema`, `inheritable`,
     `mutable`, `merge`.
5. **Array order is preserved.** Authors may group items intentionally.
6. **Comments are preserved** where the underlying document model supports
   it.

### Frontend-specific behavior

The formatter dispatches based on file extension:

| Extension        | Formatter                                             |
| ---------------- | ----------------------------------------------------- |
| `.yaml` / `.yml` | YAML formatter (parse, reorder keys, stringify)       |
| `.json`          | JSON formatter (parse, reorder keys, pretty-print)    |
| `.toml`          | TOML formatter (parse, reorder keys, stringify)       |
| `.handl`         | `.handl` formatter (parse AST, emit canonical syntax) |

Each frontend's formatter shares the same canonical key order and style
rules, but implements them through its own parse/stringify pipeline, exposed
as formatting plugins.

### Implementation

- The formatter operates on the parsed document, re-orders mapping keys with
  a schema-aware sorter, then stringifies with the canonical rules.
- `--check` compares the emitted text to the original and exits `1` when
  they differ, but does not write anything.
- `--write` overwrites the source file atomically.
- The formatter does not resolve imports or merge included files — it
  formats only the file being edited.

### Plugin architecture

The formatter is part of the handl toolchain, not a host-specific feature.
The generic core (`BasicFormatter`, `FormatBehavior`) lives in
`handl-platform` / `plugin-ext-fmt`. Language-specific rules are plugins:
hosts and applications (e.g. Peregrin) extend the canonical key order with
host-specific keys via plugin configuration, and kits curate which
formatting plugins are loaded. The formatter is modular — each rule set is
an independent plugin — and extensible — new languages and frontends add
formatting by contributing new `FormatBehavior` plugins, not by editing a
monolithic formatter.


## Why

handl documents can be written in multiple frontends (YAML, JSON, TOML, `.handl`). Within each frontend, the same logical document can be formatted in many different styles: extra blank lines, mixed indentation, keys in arbitrary order, trailing whitespace. This creates noisy diffs, harder reviews, and formatting arguments in pull requests.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- All handl documents can be formatted the same way, reducing style noise
- `handl fmt --check` can run in CI to enforce formatting.
- The formatter stays in sync with the language model, so language changes
- Existing documents need a one-time pass with `handl fmt --write`.
- Each frontend needs its own parse/stringify implementation, but the key

## Enforcement

- `plugin-ext-fmt` — where this is true.
- `plugin-fmt-expression` — where this is true.
- `plugin-fmt-stmt` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0014-formatter.md`](../design/0014-formatter.md).
