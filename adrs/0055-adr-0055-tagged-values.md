+++
id = "ADR-0055"
title = "ADR-0055: Tagged Values"
status = "open"
priority = "medium"
+++

---
title: Tagged Values
type: adr
date: 2026-09-01
status: accepted
status_note: Accepted — tagged values; forgery is impossible.
---

# ADR-0055: Tagged Values

## Decision

1. **Tag registries** define the legal tags per host; a tag is registered once
   and carries its rules.
2. **`tag` / `match_tag` / `tag_registry` are host capabilities**, not syntax.
3. **Representation**: a tagged value is a `serde_json::Value` carrying its tag
   and payload; no new value kind.
4. **Two equality flavors, one primitive**: structural equality and
   tag-identity equality share one comparison primitive.
5. **Forgery is impossible** — a tag can only be minted through the registered
   capability.
6. **Sugar**: `type` (and future `unit`) declares tags; the primitive is the
   capability.
7. **Untagged values are unchanged**; tagging is additive.

## Why

Types and branded values were Rust enums, so a tag could be forged by
constructing the variant and new tags meant engine changes. Registry-minted
tagged values make tags unforgeable and additive.

## Not doing
- **A new value kind** — tags ride on `serde_json::Value`.
- **Constructor-based tags** — only the registry mints them.
- **One equality for everything** — two flavors share one primitive.

## Consequences
- Tags are data; forging requires the capability, not the constructor.
- Equality is explicit: structural vs tag identity.
- Untagged code is unaffected.

## Enforcement
- `kernel/` — tag registry and minting capability.
- `plugin-lib-tag/` — the host primitive.
- Typechecker tests for forgery/equality.

## Design
+Registries, capabilities, representation, equality and the sugar:
+[`design/0055-branded-values.md`](../design/0055-branded-values.md).
