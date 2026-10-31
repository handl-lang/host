+++
id = "ADR-0039"
title = "ADR-0039: Kits Are Curated Plugin Lists, Not Implementations"
status = "open"
priority = "medium"
+++

---
title: Kits Are Curated Plugin Lists, Not Implementations
type: adr
date: 2026-09-02
status: proposed
status_note: Proposed — kits curate plugins; composition is dynamic.
---

# ADR-0039: Kits Are Curated Plugin Lists, Not Implementations

## Decision

1. **A kit is a curated list of plugins, not an implementation.** It selects and
   orders plugins; it does not provide operations itself.
2. **Composition is dynamic**: a kit is resolved when the host loads it, not
   compiled into the binary.
3. **Plugins offer capabilities; kits only curate them** — the capability
   belongs to the plugin, never to the kit.
4. **Naming principle**: a kit is named for the surface it curates
   (`kit-canonical`, `kit-type`), not for the repo or host that uses it.
5. **Migration is four phases**: thin `kit-canonical`, separate kits from hosts,
   make `kit-type` a real kit, update consumers.

## Why

Kits were implementations, so replacing a plugin meant editing the kit and
rebuilding; the same plugin could not be curated differently by two hosts.
Curation as data makes composition a load-time choice and keeps capabilities
with their plugins.

## Not doing
- **Kits that implement operations** — that is the current coupling.
- **Static dispatch baked into the kit** — kits are resolved, not compiled in.
- **Per-host kit copies** — one kit, curated by many hosts.

## Consequences
- Kits become small data; a host chooses a kit, a kit chooses plugins.
- Dynamic loading is required; failures surface at load time.
- Naming discipline keeps the surface obvious.

## Enforcement
- `kit-canonical/`, `kit-prelude/` — the kit manifests.
- `crates/plugin-ext-kit/` — curation and loading.
- Host tests that load a kit and exercise an op.

## Design
+The architecture, dynamic composition, naming and the four phases:
+[`design/0039-kits-are-curated-plugin-lists.md`](../design/0039-kits-are-curated-plugin-lists.md).
