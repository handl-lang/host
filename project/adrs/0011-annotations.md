# Document Annotations

## Context

handl documents carry runtime-relevant fields (field declarations,
expressions, schemas) but have no standard place for human-readable
metadata: descriptions, examples, deprecation notices, authors, tags, or
documentation links. This forces documentation generators, library browsers,
and tooling to infer meaning from identifiers or duplicate metadata in
external files.

Annotations are a language-level feature: any handl document can carry
metadata that the VM ignores but tooling can read. Hosts and applications
(e.g. Peregrin) may define additional annotation keys for their own use, but
the core set is language-level.

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

## Consequences

- Documentation generators can produce rich output from `title`,
  `description`, and `example`.
- Library browsers can filter and search by `tags`.
- The linter can warn on missing `title` or `description` for library
  modules.
- Annotations are a zero-cost abstraction — no runtime impact.
- Hosts get a standard extension point for metadata without inventing their
  own fields.

## Status

Proposed
