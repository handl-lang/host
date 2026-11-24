+++
id = "ADR-0185"
title = "ADR-0185: Custom Fields"
status = "open"
priority = "med"
+++

---
title: Custom Fields
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — fields are declarative `*.field.yml` files.
---

# ADR-0185: Custom Fields

> Imported from `kit-data-prelude/project` — originally `ADR-0016` (global registry repo `lang`); renumbered `ADR-0185` in the host merge.

## Decision

1. **User-defined fields are declared in `*.field.yml` files**; the core keeps
   only the minimal built-in keys.
2. **Core keys are** `type`, `expr`, `arg`, `schema`, `inheritable`, `mutable`
   and `merge`; every other field key is user- or library-defined.
3. **Field types are `sugar` and `handler`**: sugar expands to a core-language
   tree before evaluation; handler is a host-provided implementation whose YAML
   file is the manifest.
4. **A field file declares its schema and expansion**, and the resolver loads
   it by name (built-ins first, then `*.field.yml`).

## Why

Every new convenience field meant a core-language change. Declarative field
files keep the core minimal and let libraries ship fields without forking it.

## Not doing
- **Hardcode convenience fields** — the seven core keys express any field.
- **Runtime code per field** — sugar expands; handler is registered.
- **A second schema language** — field schemas are the same schema.

## Consequences
- Fields are data; libraries ship them.
- Sugar keeps runtime cost at zero; handlers need host registration.
- The resolver must order built-ins before files.

## Enforcement
- `crates/handl-kit-data-prelude/` — field parser/resolver.
- `fields/*.field.yml` — the shipped fields.
- Tests for sugar expansion and handler registration.

## Design
+Format, core keys, field types and resolution:
+[`design/0185-custom-fields.md`](../design/0185-custom-fields.md).
