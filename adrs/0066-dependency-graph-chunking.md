---
title: Dependency-Graph Chunking
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed — deferred until a concrete driver exists.

  Depends on [ADR-0063 — Scheme-Core Completeness](./0063-scheme-core-completeness.md),
  [ADR-0064 — Computational Language Authoring](./0064-computational-language-authoring.md),
  and [ADR-0065 — Derived-Artifact Cache and Compression](./0065-derived-artifact-cache-and-compression.md).
  Refines [ADR-0044 — Language as Data and Runtime
  Split](./0044-language-as-data-and-runtime-split.md) and
  [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md).
---


# ADR-0066: Dependency-Graph Chunking

## Decision


### 1. Regions are computed from the fragment dependency graph

A region is a maximal set of fragments where every fragment's `requires`
(ADR-0044 §7) and `replaces` (ADR-0044 §5) targets are within the set, or in
a declared dependency region. Region partitioning is computed from the
fragment graph, not hand-assigned:

```
region_partition(fragments) -> Vec<Region>
  where Region = { fragments: FragmentSet, dep_regions: Set<RegionId> }
```

The partition algorithm is a graph clustering (e.g., Tarjan's strongly
connected components for the `requires`/`replaces` graph, then merge small
SCCs into regions by a size target). The result is deterministic given the
same fragment set.

### 2. Chunks are serializable, compressible units within a region

A chunk is a subset of a region's fragments, serialized independently. A
chunk carries:

- Its fragment data (serde-serializable, per ADR-0044).
- Its dependencies: references to other chunks (by `CacheKey`, per
  ADR-0065) that must be loaded before this chunk can be typechecked or
  eval'd.
- Its `replaces` targets (so the override graph can be resolved across
  chunks).

```
Chunk = {
    id: ChunkId,
    fragments: FragmentSet,
    dependencies: Set<ChunkId>,
    replaces: Map<FragmentId, ChunkId>,
    checksum: Blake3,
}
```

Chunks are compressed (zstd, per ADR-0065 §3) when stored. Decompression
happens on load.

### 3. Lazy load, on-demand typecheck

The runtime loads the entry-point chunk (the one containing the fragments
needed for the program's first eval). On load:

1. Decompress the chunk.
2. Resolve dependencies: load each dependency chunk (recursively, depth-first).
3. Merge the loaded chunks' fragments (ADR-0044 §5 merge semantics).
4. Typecheck the loaded subset (ADR-0044 §1 `AstShapeDef` payload schemas,
   ADR-0054 typecheck engine).
5. Eval.

Unloaded chunks are not in memory. If an eval references a fragment in an
unloaded chunk (e.g., a `Foreign` call to a host function whose capability
declaration is in another region), the runtime loads that chunk on demand.

### 4. The dependency graph is the arrow `>>>` structure, not an approximation

This is the key property. Because the kernel is arrow calculus (ADR-0052),
the compute graph is statically recoverable from the `Language` data. The
chunking layer reads the `>>>` composition structure to determine which
fragments are needed for a given eval, and loads only those chunks. There is
no "load everything and hope" fallback; the dependency graph is precise.

For `app` (dynamic arrow invocation, ADR-0052 §2), the callee is a runtime
value. The chunking layer cannot statically know which chunk the callee's
arrow lives in. Resolution: `app` triggers a load of the chunk containing
the callee's `Language` fragment (the callee carries its chunk id, or the
runtime resolves it by name). This is the one case where chunking falls
back to a runtime lookup — and it is the same case where the arrow model
itself acknowledges dynamic dispatch (ADR-0052 §2).

### 5. Chunking is transparent to engines

Engines (tree-walk, bytecode, cranelift) operate on a merged `Language`
value (ADR-0044 §1). Chunking produces that merged value from a subset of
chunks; the engine does not know whether the `Language` was assembled from
one chunk or fifty. This preserves ADR-0044's engine-swap property: swapping
Pratt for chumsky does not interact with chunking.

### 6. Chunking is opt-in and configurable

A runtime config specifies whether chunking is enabled and the region/chunk
parameters:

```toml
[chunking]
enabled = true
region_size_target = "16KB"   # merge small regions up to this size
chunk_compression = "zstd:3"
```

For compile-time-embedded kits (ADR-0044 §6), chunking is off — the whole
`Language` is in the binary. For runtime-loaded languages, chunking is on
when a driver (memory constraint, network load, large definition) is present.


## Why

ADR-0052 made the kernel arrow calculus: `arr`/`>>>`/`first`/`choice`/`loop`/`app`. The `>>>` combinator composes descriptions without running them (ADR-0052, README "Descriptions, not execution"). This means the compute graph is anything. This is the property arrows have that monads don't (ADR-0041 §"The arrow calculus solves the unpredictable-compute-graph problem"), and it is what makes dependency-aware chunking precise rather than approximated.

## Not doing

- **Multiple large kits** are loaded in one process (each kit is a region;
- **Embedded/constrained targets** with limited memory (load only what's
- **Network-loaded languages** (stream chunks as needed, not the whole
- **Large language definitions** (e.g., a `Language` with a rich standard

## Consequences

- **Memory-bounded language loading.** Only the chunks needed for the
- **Precise dependency loading, not approximate.** The arrow `>>>` graph
- **Composable with caching (ADR-0065).** Chunks are cacheable units; the
- **Transparent to engines.** No engine changes; chunking produces the same
- **Infrastructure overhead.** Region partitioning, chunk serialization,

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0066-dependency-graph-chunking.md`](../design/0066-dependency-graph-chunking.md).
