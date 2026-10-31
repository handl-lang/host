+++
id = "ADR-0065"
title = "ADR-0065: Derived-Artifact Cache and Compression"
status = "open"
priority = "medium"
+++

---
title: Derived-Artifact Cache and Compression
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Depends on [ADR-0063 — Scheme-Core Completeness](./0063-scheme-core-completeness.md)
  and [ADR-0064 — Computational Language Authoring](./0064-computational-language-authoring.md).
  Refines [ADR-0044 — Language as Data and Runtime
  Split](./0044-language-as-data-and-runtime-split.md) §6 (fragment sources and
  kit assembly).
---


# ADR-0065: Derived-Artifact Cache and Compression

## Decision


### 1. Cache the merged `Language`, not the authoring source

The cache stores the **output of the assembly pipeline**: the merged,
validated `Language` value, serialized (serde). The input is the full set of
fragment sources + kit manifest + `extends` chain + engine selections. The
cache does not store intermediate fragments or the authoring program; it
stores the final product.

```
cache key → serialized merged Language (optionally compressed)
```

### 2. Cache key is a hash of the full assembly input

The cache key is a cryptographic hash (BLAKE3) of a canonical serialization
of:

1. Every fragment source (YAML file content hash, or handl program content
   hash + the `Language` used to eval it).
2. The kit manifest (ordered fragment-source list + include/exclude filters).
3. The `extends` chain (transitive: parent kits' manifests, recursively).
4. Engine selections (lex/parse/eval engine names + versions, per ADR-0044
   §7 "Engine selection as data").
5. The `scheme_version` declared by the `Language` (ADR-0044 §7).

If any input changes, the hash changes, and the cache misses. This is the
only correct cache key. Hashing a subset (e.g., only the top-level manifest)
risks stale hits when a transitive dependency changes.

```
key = blake3(canonical_serialize(
    fragment_source_hashes,
    kit_manifest,
    extends_chain_transitive,
    engine_selections,
    scheme_version,
))
```

### 3. Compression is applied to the cache payload, not the source

The serialized merged `Language` is compressed with zstd before storage.
Decompression happens on cache hit. The authored source (YAML, handl
program) is never compressed by this system — it remains text on disk.

Compression level is configurable. Default: level 3 (fast, ~2x compression).
Constrained targets may use level 9 (~4x, slower decompression). The
tradeoff is I/O cost vs CPU cost, and it is a runtime config, not a build
decision.

### 4. Cache invalidation is by key, not by TTL

There is no time-based expiry. The cache is valid as long as the key matches.
If the assembly input changes, the key changes, and the old entry is a miss
(not a stale hit). This is correct because the assembly is a pure function
of its inputs — same inputs, same output; different inputs, different output.

TTL-based invalidation is explicitly rejected: it introduces a window where
the cache is stale but the key has not changed (e.g., a file was edited but
the TTL has not expired). This is the silent-drift failure mode.

### 5. Cache storage is pluggable

The cache backend is a trait, not a fixed implementation:

```rust
pub trait LanguageCache {
    fn get(&self, key: &CacheKey) -> Option<CachedLanguage>;
    fn put(&self, key: &CacheKey, lang: &CachedLanguage) -> Result<()>;
}
```

Implementations:

- `NoCache` — always misses (default for compile-time-embedded kits).
- `InMemoryCache` — `HashMap<CacheKey, CachedLanguage>`, process-lifetime.
- `OnDiskCache` — files in a cache directory, persistent across runs.
- (future) `NetworkCache` — shared cache for a fleet of hosts.

### 6. Compile-time embedding remains the primary path

For the canonical kit and any kit known at build time, `include_str!` +
compile-time assembly remains the path. The cache is for **runtime-loaded**
languages only. A compile-time-embedded `Language` is already "cached" in the
binary; the runtime cache adds nothing.


## Why

ADR-0044 §6 made authored data files canonical and embedded them at compile time via `include_str!`. This is compile-time caching: zero runtime parse cost, zero I/O. It works for the canonical kit and any kit known at build time.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Runtime-loaded languages are fast on repeat loads.** The first load
- **No silent drift.** The cache key includes every input that affects the
- **Compression reduces I/O and binary size for cached artifacts.** Modest
- **The authored source stays inspectable.** Compression applies only to the
- **Cache key computation has a cost.** Hashing every fragment source on

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0065-derived-artifact-cache-and-compression.md`](../design/0065-derived-artifact-cache-and-compression.md).
