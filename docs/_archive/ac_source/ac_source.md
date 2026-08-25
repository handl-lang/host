**handl_source**

# Module: handl_source

## Contents

**Modules**

- [`location`](#location)
- [`scanner`](#scanner) - Shared source-location scanning utilities.
- [`sourcemap`](#sourcemap)

---

## Module: location



## Module: scanner

Shared source-location scanning utilities.

Frontend scanners (YAML, JSON, TOML) share common line-indexing and
value-walking logic. This module provides the reusable primitives;
each frontend supplies its format-specific key-matching behavior.



## Module: sourcemap



