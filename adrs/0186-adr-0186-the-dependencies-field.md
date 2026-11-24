+++
id = "ADR-0186"
title = "ADR-0186: The `dependencies` Field"
status = "open"
priority = "med"
+++

---
title: The `dependencies` Field
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — `dependencies` declares a document’s custom fields.
---

# ADR-0186: The `dependencies` Field

> Imported from `kit-data-prelude/project` — originally `ADR-0017` (global registry repo `lang`); renumbered `ADR-0186` in the host merge.

## Decision

1. **A built-in `dependencies` field lists the custom field names a document
   requires** before validation or evaluation.
2. **It is allowed** at the document top level, on a field declaration, in a
   `*.field.yml` file, and in library modules.
3. **Names are dotted**: bare (`delay`) for project-local fields, namespaced
   (`resilience.timeout`) for library fields.
4. **Resolution collects** dependencies from the document, field declarations
   and nested modules, resolves built-ins then `*.field.yml`, fails with the
   missing list (and requesting files), accepts built-ins, and de-duplicates per
   document.
5. **Tooling**: `handl info` shows the list; `handl lint` may warn when a custom
   field is used without being declared.

## Why

A document could use a custom field that the resolver did not know, failing
late and without naming the requester. An explicit dependency list makes the
requirement checkable before validation.

## Not doing
- **Implicit discovery** — the document declares what it needs.
- **Fail on the first missing field** — report the full list with requesters.
- **Load a field twice** — de-duplicate across scopes.

## Consequences
- Missing fields fail early with a precise list.
- Libraries state their field requirements.
- Tooling can display and lint the declaration.

## Enforcement
- `crates/handl-kit-data-prelude/` — dependency collection/resolution.
- `handl info` / `handl lint` integration.
- Tests for missing, built-in and duplicate entries.

## Design
+Allowed positions, syntax, resolution order and tooling:
+[`design/0186-dependencies-field.md`](../design/0186-dependencies-field.md).
