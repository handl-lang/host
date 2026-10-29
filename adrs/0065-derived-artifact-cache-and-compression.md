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

## Context

ADR-0044 §6 made authored data files canonical and embedded them at compile
time via `include_str!`. This is compile-time caching: zero runtime parse
cost, zero I/O. It works for the canonical kit and any kit known at build
time.

It does not cover **runtime-loaded languages**. Once handl can author
`Language` values computationally (ADR-0064) and self-describe (ADR-0063),
languages can be loaded at runtime — user-defined kits, mods, dynamically
generated dialects. Each load requires:

1. Read the authoring source (handl program or YAML).
2. If handl: `eval` the authoring program to produce fragment values.
3. Merge fragments in kit-defined order (ADR-0044 §1, §5).
4. Resolve `replaces` override chains (ADR-0044 §5).
5. Validate `requires` dependencies (ADR-0044 §7).
6. Build the merged `Language` value.

Steps 2–6 are deterministic given the same inputs. Repeating them on every
load is wasted work. This is where caching applies.

### The danger: stale cache = silent semantic drift

ADR-0044 §5 makes composition an **auditable override graph**: fragments
declare `replaces: [id]`, and the merged `Language` carries the full history
of what replaced what. A cache that serves a stale merged `Language` — one
that does not reflect a change in a fragment source, a kit manifest, an
`extends` chain, or an engine selection — produces **silent semantic drift**.
The program runs under a language that is not the one the author intended,
with no error. "Why does this `let` parse differently?" is answerable by
reading the override chain (ADR-0044 §5) — unless the cache served a stale
chain.

This is the dangerous failure mode, not a crash. A crash is recoverable;
silent drift is not. The cache key must be correct, or the cache must not
exist.

### Compression: modest, secondary

The canonical `Language` is ~180KB of YAML. Compressed (zstd, level 3–9),
~30–40KB. This is a real win for binary size and runtime I/O on constrained
targets, but it is not a motivation on its own. Compression is a property of
the cached artifact, not a feature.

The constraint: **compress the derived artifact, never the authored source.**
ADR-0044 §1 claims `Language` is "inspectable, authorable as text." Compressing
the authored source (YAML or handl program) breaks that property. The authored
source stays text; the serialized merged `Language` (the cache payload) may
be compressed.

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

## Consequences

### Positive

- **Runtime-loaded languages are fast on repeat loads.** The first load
  assembles; subsequent loads with the same inputs hit the cache. This makes
  mods, user-defined kits, and dynamic dialects practical.
- **No silent drift.** The cache key includes every input that affects the
  merged `Language`. A change in any input is a guaranteed cache miss.
- **Compression reduces I/O and binary size for cached artifacts.** Modest
  but real, especially for constrained targets or network-loaded languages.
- **The authored source stays inspectable.** Compression applies only to the
  derived artifact; the source remains text.

### Negative

- **Cache key computation has a cost.** Hashing every fragment source on
  every load is not free. For a small kit, this may exceed the cost of
  re-assembling. The cache is a win only when assembly is more expensive
  than key computation — which is true for computationally authored
  languages (ADR-0064) but may not be for small YAML kits.
- **The cache backend is a new subsystem.** Trait + implementations +
  storage management + eviction (for `OnDiskCache`). This is real
  infrastructure, not a wrapper.
- **Correctness depends on the key being complete.** A missing input in the
  key (e.g., forgetting to include the `scheme_version`) is a stale-hit
  bug. The key construction is the crux; it must be audited carefully.

### Neutral

- **No TTL.** This is by design. The cache is a pure-function memoization,
  not a freshness system. If freshness is needed (e.g., "reload if the
  source file changed"), it is handled by detecting the source change and
  computing a new key, not by expiring the old one.

## Implementation phases

### Phase 1 — `LanguageCache` trait and `NoCache`/`InMemoryCache`

Define the trait and the two simple implementations. `NoCache` is the
default. `InMemoryCache` is opt-in. No compression yet.

### Phase 2 — Cache key construction

Implement the BLAKE3 key over the full assembly input. Test: every input
change produces a different key; same inputs produce the same key. This is
the correctness-critical piece.

### Phase 3 — `OnDiskCache` with zstd compression

Add the persistent backend with compression. Test: cache survives process
restart; compressed payload decompresses to the correct `Language`.

### Phase 4 — Integration with runtime kit loading

Wire the cache into the runtime's kit-assembly path. A kit load checks the
cache; on miss, assembles and stores; on hit, loads the cached `Language`.
The compile-time-embedded path bypasses the cache entirely.

## References

- [ADR-0063](./0063-scheme-core-completeness.md) — Scheme-Core Completeness
  (the cache stores a `Language` value; that value must be complete)
- [ADR-0064](./0064-computational-language-authoring.md) — Computational
  Language Authoring (the primary consumer; computationally authored
  languages benefit most from caching)
- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as Data
  and Runtime Split (§5 override graph is the silent-drift risk; §6 fragment
  sources; §7 engine selection as data)
- [ADR-0046](./0046-capability-discipline-at-the-language-level.md) —
  Capability Discipline (cached `Language` values carry their capability set;
  the cache does not bypass capability checks)
