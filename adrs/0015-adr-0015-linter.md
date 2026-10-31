+++
id = "ADR-0015"
title = "ADR-0015: Linter"
status = "open"
priority = "medium"
+++

---
title: Linter
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — `handl lint` is an extensible rule registry with auto-fix.
---

# ADR-0015: Linter

## Decision

1. **`handl lint` checks documents against an extensible rule set**; paths may
   be files or globs, and `--fix` applies auto-fixes.
2. **A rule is a pure function** from the parsed document + file metadata to
   `LintIssue[]` (rule, severity, file, line, message, optional fix).
3. **Severity is `error` / `warn` / `info`**; exit 0 unless an `error` is
   present, and `--fail-on-warn` escalates warnings.
4. **The initial rule set** is naming-convention, missing-annotations,
   deprecated-keyword, unused-field, style (auto-fixable via the formatter) and
   missing-version.
5. **Rules are namespaced and extensible** — hosts register their own
   (`peregrin.event-contract`), and `--except` disables one by name.

## Why

The linter had a fixed rule list, so every new convention meant editing the
linter and host-specific policy had no home. A registry of pure rules keeps the
linter generic and lets hosts extend it without forking.

## Not doing
- **A fixed rule list** — hooks and hosts need their own rules.
- **Fixing in rule order** — `--fix` applies in a stable order across all rules.
- **Warnings failing by default** — only errors gate; `--fail-on-warn` opts in.

## Consequences
- Rules are data/functions, registered; the linter is a driver.
- Auto-fix reuses the formatter for style.
- Namespacing prevents host/language collisions.

## Enforcement
- `crates/plugin-ext-lint/` — registry and the initial rules.
- `plugin-engine-lint` — the generic driver.
- Rule tests and CLI tests.

## Design
+Command surface, severities, issue shape, the initial rules and plugin model:
+[`design/0015-linter.md`](../design/0015-linter.md).
