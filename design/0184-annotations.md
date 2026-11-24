---
title: Document Annotations — design detail
type: design
date: 2026-08-24
updated: 2026-10-29
refs: [ADR-0184]
---

> Imported from `kit-data-prelude/project/design/0011-annotations.md` — ADR-0011 renumbered to ADR-0184 in the host merge.

# Document Annotations — design detail

Moved verbatim from ADR-0184 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

## Decision

handl supports an optional `annotations` field on any document. The value
is a plain object that the VM passes through without interpretation. A small
set of conventional keys is recognised by language tooling (docs generator,
library browser, linter).

### Example

```yaml
annotations:
  title: Deploy configuration
  description: Configuration schema for the deploy pipeline.
  author: team-platform
  tags: [deploy, release]
  since: "1.2.0"
  deprecated: false
  see: https://docs.example.com/schemas/deploy
```

### Conventional keys

These keys are reserved for language tooling; other keys are allowed but
ignored by the VM and default tools:

- `title` — short display name.
- `description` — one-sentence explanation.
- `author` — person or team responsible.
- `tags` — list of labels for grouping and search.
- `since` — version when the document was introduced.
- `deprecated` — boolean or deprecation message.
- `example` — short example value or invocation.
- `see` — URL to longer documentation.

### Semantics

- Annotations are optional. Documents without them are unaffected.
- The VM ignores annotations — they have zero runtime cost.
- The typechecker ignores annotations — they do not affect type inference.
- The linter (ADR-0015) may validate conventional keys and warn on missing
  recommended annotations.
- The formatter (ADR-0014) preserves annotations in their canonical position.

### Host extensions

Hosts may define additional annotation keys. For example, a Peregrin host
might define `annotations.kind` to classify documents as workflows, nodes,
or events. Host-specific keys should be namespaced (e.g. `peregrin.kind`)
to avoid collisions with language-level keys.

## Why

handl documents carry runtime-relevant fields (field declarations, expressions, schemas) but have no standard place for human-readable metadata: descriptions, examples, deprecation notices, authors, tags, or documentation links. This forces documentation generators, library browsers, and tooling to infer meaning from identifiers or duplicate metadata in external files.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- Documentation generators can produce rich output from `title`,
- Library browsers can filter and search by `tags`.
- The linter can warn on missing `title` or `description` for library
- Annotations are a zero-cost abstraction — no runtime impact.
- Hosts get a standard extension point for metadata without inventing their

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0184-annotations.md`](../design/0184-annotations.md).
