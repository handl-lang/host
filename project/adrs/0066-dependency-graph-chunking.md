# ADR-0066: Dependency-Graph Chunking

## Status

Proposed — deferred until a concrete driver exists.

Depends on [ADR-0063 — Scheme-Core Completeness](./0063-scheme-core-completeness.md),
[ADR-0064 — Computational Language Authoring](./0064-computational-language-authoring.md),
and [ADR-0065 — Derived-Artifact Cache and Compression](./0065-derived-artifact-cache-and-compression.md).
Refines [ADR-0044 — Language as Data and Runtime
Split](./0044-language-as-data-and-runtime-split.md) and
[ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md).

## Context

### The insight: arrow calculus gives a real dependency graph

ADR-0052 made the kernel arrow calculus: `arr`/`>>>`/`first`/`choice`/`loop`/`app`.
The `>>>` combinator composes descriptions without running them (ADR-0052,
README "Descriptions, not execution"). This means the compute graph is
**statically recoverable** — you know the dependencies before you run
anything. This is the property arrows have that monads don't (ADR-0041 §"The
arrow calculus solves the unpredictable-compute-graph problem"), and it is
what makes dependency-aware chunking precise rather than approximated.

Lambda-calculus languages approximate dependencies via control-flow analysis
or escape analysis. Arrow-calculus languages **read them off the `>>>`
structure**. This is the foundation for the chunking technique in this ADR.

### The technique: regions and chunks, borrowed from game optimization

Game engines partition worlds into regions and chunks for streaming: load
only the chunks near the player, decompress on demand, unload when distant.
The technique works because the dependency graph (what's in a chunk, what a
chunk needs from neighbors) is known.

Applied to `Language` data:

- A **region** is a cluster of fragments with high internal dependency and
  low cross-region dependency (e.g., "arithmetic operators," "statement
  forms," "type definitions for the standard library").
- A **chunk** is a serializable, compressible unit within a region. A chunk
  carries its dependencies (references to other chunks, per ADR-0044 §7
  `requires` and the `replaces` override graph).
- The runtime loads chunks on demand: load the entry-point chunk, pull in
  its dependencies, typecheck the loaded subset, eval. Unloaded chunks
  are not in memory.

### Why this is deferred

The canonical `Language` is ~180KB. Chunking infrastructure (region
partitioning, chunk serialization, lazy load, on-demand typecheck, reference
resolution) has non-trivial overhead. At 180KB with one canonical kit,
chunking likely costs more than it saves in memory, I/O, and startup time.

Chunking pays off when:

- **Multiple large kits** are loaded in one process (each kit is a region;
  load only the active kit's chunks).
- **Embedded/constrained targets** with limited memory (load only what's
  needed for the current program).
- **Network-loaded languages** (stream chunks as needed, not the whole
  `Language`).
- **Large language definitions** (e.g., a `Language` with a rich standard
  library whose type definitions alone are megabytes).

None of these drivers exist today. This ADR records the design — because the
arrow-DAG chunking model is non-obvious and arrow-specific — but marks it
**deferred until a concrete driver appears**. Building it speculatively is
premature optimization.

### What ADR-0044 already provides

ADR-0044 §7 specifies fragment-level `requires` (validated at merge time)
and ADR-0044 §5 specifies the `replaces` override graph. These are the
dependency edges chunking uses. The chunking layer does not invent a new
dependency model; it partitions the existing one.

ADR-0044 §7 "Compilers erase data dispatch" notes that bytecode and cranelift
engines compile away per-node scheme lookup. Chunking is orthogonal: it
partitions the _data_ (the `Language` value); the engines still compile
whatever subset is loaded.

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

## Consequences

### Positive

- **Memory-bounded language loading.** Only the chunks needed for the
  current eval are in memory. This makes large `Language` values (rich
  standard libraries, multi-kit systems) practical on constrained targets.
- **Precise dependency loading, not approximate.** The arrow `>>>` graph
  gives exact dependencies. No over-loading, no under-loading.
- **Composable with caching (ADR-0065).** Chunks are cacheable units; the
  cache key (ADR-0065 §2) applies per-chunk, not per-whole-`Language`. A
  change in one chunk invalidates only that chunk's cache entry.
- **Transparent to engines.** No engine changes; chunking produces the same
  merged `Language` the engines expect.

### Negative

- **Infrastructure overhead.** Region partitioning, chunk serialization,
  lazy load, on-demand typecheck, reference resolution — this is a
  subsystem, not a wrapper. The overhead is justified only when a driver
  is present; at 180KB with one kit, it is not.
- **`app` requires runtime chunk resolution.** Dynamic arrow invocation
  cannot be statically chunked. The runtime must resolve the callee's chunk
  at eval time. This is a fallback, not a failure — but it is a case where
  the precise-dependency property does not hold.
- **Cross-chunk `replaces` is complex.** A fragment in chunk A that
  `replaces` a fragment in chunk B requires B to be loaded before A's
  override is resolved. The override graph (ADR-0044 §5) spans chunks; the
  merge must handle partial graphs and load dependencies on demand.
- **Typecheck is per-loaded-subset, not whole-`Language`.** A fragment's
  `requires` (ADR-0044 §7) may be in an unloaded chunk. The typecheck must
  either load the chunk (defeating laziness) or defer the check (risking a
  late error). The design chooses: load the chunk. `requires` is a hard
  dependency; if it's not loaded, the fragment cannot be safely used.

### Neutral

- **Deferred.** This ADR records the design but does not mandate
  implementation. A concrete driver (multi-kit, embedded target,
  network-loaded languages, megabyte-scale definitions) must appear before
  implementation begins.

## Implementation phases (deferred)

### Phase 1 — Region partitioning

Implement `region_partition(fragments)` using SCC + size-target merging.
Test: the canonical kit's fragments partition into regions; cross-region
dependencies are correctly identified.

### Phase 2 — Chunk serialization and lazy load

Serialize chunks independently. Implement lazy load with dependency
resolution. Test: load an entry-point chunk, pull in dependencies, produce
the same merged `Language` as whole-`Language` assembly.

### Phase 3 — On-demand typecheck

Typecheck the loaded subset. Test: a program that uses only arithmetic
loads only the arithmetic region; a program that uses `match` loads the
`match` region on demand.

### Phase 4 — `app` chunk resolution

Implement runtime chunk resolution for dynamic arrow invocation. Test: a
program that `app`s an arrow from an unloaded chunk loads it correctly.

## References

- [ADR-0052](./0052-arrow-calculus-kernel.md) — Arrow Calculus Kernel (`>>>`
  gives the static dependency graph; `app` is the dynamic case)
- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as Data
  and Runtime Split (§5 `replaces` graph; §7 `requires`, engine selection)
- [ADR-0065](./0065-derived-artifact-cache-and-compression.md) —
  Derived-Artifact Cache and Compression (chunks are cacheable units;
  per-chunk cache keys)
- [ADR-0063](./0063-scheme-core-completeness.md) — Scheme-Core Completeness
  (chunking operates on a complete `Language`; incomplete cores produce
  chunks with unresolved `Foreign` dependencies)
- [ADR-0054](./0054-generic-typecheck-engine.md) — Generic Typecheck Engine
  (per-chunk typecheck uses the same engine)
- Game engine chunking (e.g., Unreal Engine World Partition, Minecraft
  chunk streaming) — the streaming/load-on-demand technique this ADR adapts
