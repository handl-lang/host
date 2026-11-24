+++
id = "ADR-0184"
title = "ADR-0184: Annotations"
status = "open"
priority = "med"
+++

---
title: Annotations
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — annotations are pass-through metadata.
---

# ADR-0184: Annotations

> Imported from `kit-data-prelude/project` — originally `ADR-0011` (global registry repo `lang`); renumbered `ADR-0184` in the host merge.

## Decision

1. **An optional `annotations` object is allowed on any document**; the VM
   passes it through without interpretation.
2. **Conventional keys** are `title`, `description`, `author`, `tags`, `since`,
   `deprecated`, `example` and `see`; other keys are allowed but ignored by the
   VM and default tools.
3. **Annotations cost nothing at runtime or typecheck**; the linter may validate
   the conventional keys and the formatter preserves them.
4. **Hosts may define additional conventional keys**, namespaced to avoid
   collisions with language tooling.

## Why

Documents needed human metadata (title, author, deprecation) but the language
had no place for it, so tools parsed ad-hoc comments. A pass-through object
gives tooling one surface and keeps the VM and typechecker untouched.

## Not doing
- **Typed annotation fields** — they are documentation, not data.
- **VM interpretation** — zero runtime cost is the point.
- **A closed key set** — hosts extend it.

## Consequences
- Tooling reads one known object; the VM ignores it.
- Lint/formatter own validation and placement.
- Host keys stay namespaced.

## Enforcement
- `crates/handl-kit-data-prelude/` — document parsing (pass-through).
- Linter rules for conventional keys (ADR-0015).
- Formatter placement tests (ADR-0014).

## Design
+Example, conventional keys, semantics and host extensions:
+[`design/0184-annotations.md`](../design/0184-annotations.md).
