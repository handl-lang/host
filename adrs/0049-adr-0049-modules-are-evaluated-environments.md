+++
id = "ADR-0049"
title = "ADR-0049: Modules are Evaluated Environments"
status = "open"
priority = "medium"
+++

---
title: Modules are Evaluated Environments
type: adr
date: 2026-09-02
status: proposed
status_note: Proposed — modules are environments evaluated from source.
---

# ADR-0049: Modules are Evaluated Environments

## Decision

1. **Environments are first-class values** — a module is the environment its
   source evaluates to.
2. **`import` / `export` / `module` / `field-access` move off `Foreign`** and
   become language-level operations over environments.
3. **The import resolver is the loader** — resolution and evaluation are one
   pipeline, not two mechanisms.
4. **The stdlib is `.hnl` files** evaluated like user modules.
5. **The module system is written in handl**, on top of the environment value.
6. **Per-language module systems**: each language defines its own import/export
   rules over the same environment primitive.

## Why

Module operations were host calls, so the language could not define its own
module system and the stdlib needed a separate path. Environments as values
dissolve most of the machinery ADR-0048 proposed and make modules ordinary
data.

## Not doing
- **A `Module` value type plus registry** — ADR-0048's rejected design; an
  environment is enough.
- **Host-only module ops** — they are language operations.
- **One module system for every language** — each language defines its own.

## Consequences
- Modules are environments; import is resolve + evaluate.
- The stdlib shares the mechanism.
- Each language owns its import/export rules.

## Enforcement
- `handl-runtime/` — environment values and the resolver.
- `kit-prelude-lang/` — the `.hnl` stdlib.
- Module-system tests per language.

## Design
+Environment values, the four operations, resolver and per-language systems:
+[`design/0049-modules-are-evaluated-environments.md`](../design/0049-modules-are-evaluated-environments.md).
