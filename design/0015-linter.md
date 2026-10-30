---
title: handl Linter — design detail
type: design
date: 2026-08-24
updated: 2026-10-29
refs: [ADR-0015]
---

# handl Linter — design detail

Moved verbatim from ADR-0015 (ADR-0161 size budget). The ADR carries the rules;
this page carries the context, tables, phases and trade-offs.

## Context

`handl validate` checks that a document loads and that basic structural
invariants hold: fields are well-formed, schemas are valid, expressions
parse. As the language and standard library grow, there are more conventions
that are not strictly load errors but do affect quality, maintainability,
and safety:

- Library modules should carry annotations (`title`, `description`, `author`,
  `since`, `see`) per ADR-0011.
- Documents using non-stdlib modules should declare their dependencies.
- Field names should follow naming conventions.
- Deprecated fields or schema keywords should be flagged.
- Documents should be consistently formatted (ADR-0014).

These checks are too opinionated for `validate`, which must stay focused on
"can the VM load this document?". They are also too handl-specific for a
generic linter. A dedicated `handl lint` command is the natural place for
convention and best-practice checks.

## Decision

handl provides an `handl lint` command that checks documents against an
extensible rule set.

### Command surface

```bash
handl lint document.yaml                  # lint a single file
handl lint document.yaml --fix            # auto-fix where possible
handl lint document.yaml --fail-on-warn   # exit non-zero on warnings too
handl lint document.yaml --except style   # skip a rule
handl lint libraries/**/*.yaml            # lint many files
```

### Severity levels

- `error` — the document violates a hard convention and must not pass CI.
- `warn` — the document works but is missing a recommended practice.
- `info` — a reminder or style suggestion.

By default `handl lint` exits `0` when only warnings and info are present and
`1` when any `error` is present. `--fail-on-warn` makes warnings fail too.

### Rule architecture

Each rule is a pure function from the parsed document and file metadata to
a list of `LintIssue`s:

```rust
pub struct LintIssue {
    pub rule: String,
    pub severity: Severity,
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
    pub fix: Option<Fix>,
}
```

Rules are registered in a rule registry. The linter runs all enabled rules
and collects issues. `--fix` applies auto-fixable issues in a stable order.

### Initial rule set

- `naming-convention` (`error`): field names do not follow the naming
  convention (snake_case, no reserved keywords).
- `missing-annotations` (`warn`): a library module is missing one of
  `title`, `description`, `author`, `since`, `see`.
- `deprecated-keyword` (`warn`): a schema uses a deprecated keyword.
- `unused-field` (`info`): a field is declared but never referenced.
- `style` (`warn`, auto-fixable): the file is not in canonical format
  (ADR-0014). The fix reuses the formatter module.
- `missing-version` (`info`): a library module has no `version` field
  (ADR-0012).

### Plugin architecture

The rule registry is extensible. Hosts and applications (e.g. Peregrin) can
register additional rules:

- Peregrin might add rules for event contract validation, resilience
  defaults, or orchestration-specific naming conventions.
- A custom host might add rules for capability declarations or security
  policies.

Host-specific rules are namespaced (e.g. `peregrin.event-contract`) to
avoid collisions with language-level rules.

## Consequences

- CI can enforce handl conventions without making `validate` opinionated.
- Library authors get clear, rule-based feedback on naming, annotations, and
  versioning.
- The rule set grows incrementally as new ADRs add conventions.
- Auto-fix reduces the friction of adopting style rules.
- `validate` and `lint` remain separate: `validate` is a fast load-only
  check; `lint` is a slower, more thorough quality gate.
- Hosts get a standard extension point for their own lint rules.
