---
title: Versioning for Language Artifacts
type: adr
date: 2026-08-24
status: superseded
status_note: |-
  The semver `version` field
  is no longer package identity; content hashes are. The `@v1.2.0`
  specifier syntax is retained as a tag lookup, and `version` may still
  appear on documents as metadata only.
superseded_by: [ADR-0010]
---

# Versioning for Language Artifacts

## Context

The package manager (ADR-0010) needs a way to identify and pin specific
versions of handl modules. Without versions, `handl install org/repo` cannot
know which revision to fetch, and `import()` cannot reliably reference a
module that may change over time.

Versioning is a language platform concern: handl modules, libraries, and
packages need a consistent version identity that works across local files,
installed packages, and registries.

## Decision

handl supports an optional `version` field on any document. The field is
a string following semver format. It is used by the package manager, the
module resolver, and the lockfile to identify and pin reusable units.

### Example

```yaml
version: "1.2.0"
```

### Specifier grammar

The install and import systems use a versioned specifier:

```text
<name-or-path>[@<version>]
```

Examples:

```bash
handl install 9aia/deploy-patterns
handl install 9aia/deploy-patterns@v1.2.0
handl install gitlab:acme/campaigns@1.0.0
```

```handl
import("9aia/deploy-patterns@1.2.0")
import("./local/patterns/upgrade@2.0.0")
```

A missing version uses the latest resolved tag or the default branch. An
exact version pins the package or file.

### Lockfile

`handl install` writes a lockfile at `.handl/lock.json`:

```json
{
  "packages": {
    "9aia/deploy-patterns@1.2.0": {
      "resolved": "https://github.com/9aia/deploy-patterns",
      "ref": "v1.2.0",
      "sha": "abc123...",
      "into": ".handl/packages/9aia/deploy-patterns"
    }
  }
}
```

The lockfile makes `handl install` reproducible without checking installed
packages into version control.

### Semantics

- `version` is optional. Documents without it default to `"0.0.0"`.
- The VM ignores `version` — it has no runtime cost.
- The module resolver uses `version` to validate that an imported module
  matches the expected version when a specifier includes one.
- `annotations.since` and `annotations.deprecated` (ADR-0011) can refer to
  `version` values.

## Consequences

- handl modules can be versioned and pinned.
- `import()` can reference stable versions of installed or remote packages.
- `handl install` becomes deterministic when a lockfile is present.
- The module resolver must be able to load a module from a versioned install
  path.
- Documents gain a `version` field — a minimal addition to the data model.
