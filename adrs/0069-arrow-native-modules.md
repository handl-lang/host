# ADR-0069: Files Are Arrows — Arrow-Native Modules

## Status

Accepted

Supersedes the module model in [ADR-0056 — Import / Export and Multi-File Programs as Evaluated Environments](./0056-import-export-and-multi-file-programs.md)
for arrow-native programs. Builds on [ADR-0049 — Modules are Evaluated Environments](./0049-modules-are-evaluated-environments.md),
[ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md),
and [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md).

## Context

ADR-0056 proposed a module system for handl based on evaluated environments:
a module is the result of evaluating a source file, producing an environment
(a binding table mapping names to values). Import binds that environment,
field access reads from it, and export controls visibility.

That model assumes a **statement-based** language: files contain `let`
bindings, `export` declarations, and `import` statements that execute
sequentially and produce an environment of named values.

Handl is **arrow-native**. A program is not a sequence of statements — it is
a declarative arrow graph. There are no statements, no sequential execution
of bindings, no environment produced by running statements in order. The
fundamental unit is the arrow, not the binding.

This creates a mismatch:

1. **No statement semantics to produce an environment.** ADR-0056's model
   requires evaluating statements to build an environment. Arrow-native
   programs don't have statements — they have graphs.
2. **No first-class environment needed.** ADR-0056 requires environments as
   first-class values so modules can be bound, stored, and field-accessed.
   Arrow-native programs already have first-class arrows — that's the entire
   language.
3. **Two caches, not one.** ADR-0056 caches evaluated environments (runtime
   values). Arrow-native programs need both arrow graphs (for execution) and
   inferred contracts (for verification). The module system must handle both.

The question: what does "module" mean when there are no statements, only
arrow graphs?

## Decision

**A file is an arrow.** Modules, imports, and multi-file programs are not a
new concept — they are the arrow algebra itself.

The key refinement since the original proposal: **imports normalize to
projections**. This normalization is the semantic abstraction — the meaning
of `import` is `project`. How that normalization is realized (compile-time
desugaring, whole-program flattening, or runtime module loading) is an
implementation choice, not a semantic one.

### 1. A file is an arrow

A `.hnl` file is an arrow. A single-expression file is that expression's
arrow:

```handl
// collection/sort.hnl
loop(comp(insert, first(id)))
```

This file IS the arrow `loop(comp(insert, first(id)))`. Its type is
`List α ⇝ List α`. Running the file runs the arrow. Importing the file
imports the arrow.

A multi-name file is an arrow that produces a **record** of named arrows:

```handl
// collection.hnl
cons = arr("list.cons")
head = arr("list.head")
sort = loop(comp(insert, first(id)))
```

This is sugar for:

```handl
{ cons: arr("list.cons"), head: arr("list.head"), sort: loop(comp(insert, first(id))) }
```

The file's arrow is `Unit ⇝ { cons: Arrow, head: Arrow, sort: Arrow }`. It
produces a record of named arrows. No statements, no environment, no
sequential evaluation — just a record literal built from arrow expressions.

### 2. Import normalizes to projection

The **semantic model**: importing a name from a file is projecting a field
from the file's record arrow.

```handl
import sort from "collection/sort"
```

Normalizes to:

```handl
sort = project(sort, <collection/sort.hnl's arrow>)
```

You take the file's arrow (producing a record), project the `sort` field,
and bind it as a name in the current scope. This is ordinary arrow
composition — no new concept.

For a single-expression file (no record), the import binds the file's
arrow directly:

```handl
// sort.hnl is just: loop(comp(insert, first(id)))
import sort from "collection/sort"
// normalizes to: sort = <collection/sort.hnl's arrow>
```

**The normalization is the abstraction.** The import syntax has exactly one
meaning: project a name from a module's record arrow. Everything else —
when projection happens, whether the graph is flattened, whether modules
are loaded lazily — is implementation strategy (see §5).

### 3. Using a name is App

There are no function calls. Only arrow composition. `sort([3, 1, 2])` is
sugar for:

```handl
const([3, 1, 2]) >>> sort
```

A constant producer composed with the sort arrow. In the `Scheme` IR, this
becomes `App { callee: "sort", args: [const([3, 1, 2])] }`.

At runtime: `App` resolves `"sort"` to the cached arrow, runs it on the
input.

At compile time: `App` resolves `"sort"` to the cached contract, uses it
for compositional inference.

### 4. Two caches: graphs and contracts

The module system maintains two caches, both keyed by module path:

- **Graph cache**: `module path → Scheme` — the parsed arrow graph, used
  for execution
- **Contract cache**: `module path → Contract` — the inferred contract,
  used for verification

Both are populated when a module is first imported. Subsequent imports
return the cached entries.

This enables **compositional inference**: each module is inferred once, and
its contract is reused by importers. The inference engine does not re-trace
imported arrows — it looks up their cached contracts.

### 5. Normalization strategies — one abstraction, many implementations

The semantic abstraction is fixed: **import = projection**. But the
realization of that projection is flexible. Three strategies, all valid
implementations of the same abstraction:

#### Strategy A: Compile-time desugar (eager, static)

Imports are resolved during parsing/normalization. Each `import` becomes a
`project(name, record_arrow)` node in the Scheme IR. After normalization,
there are no imports — only projections and App references.

```
source: import sort from "collection/sort"
  ↓ normalize
Scheme:  sort = Project { field: "sort", record: <collection/sort's Scheme> }
```

- **Pros**: No runtime module loading. The IR is self-contained. Inference
  sees the full graph structure (projections are transparent).
- **Cons**: Every import is resolved upfront. Large dependency trees produce
  large IR. No dynamic loading.

#### Strategy B: Whole-program flattening (eager, inlined)

The entire multi-file program normalizes into a single arrow graph. All
imports resolve to projections, all projections resolve to inlined
subgraphs, all App references resolve to their target arrows. One graph,
no module boundaries.

```
main.hnl + collection/sort.hnl + collection/insert.hnl
  ↓ flatten
single Scheme graph (no Project, no App — all inlined)
```

- **Pros**: Maximally simple IR — one graph, no indirection. Inference
  traces the entire program in one pass. No cache needed.
- **Cons**: No separate compilation. Re-flatten on any change. The graph
  can be very large. Loses module boundaries for debugging.

#### Strategy C: Runtime module loading (lazy, cached)

Imports are resolved on first access. The module loader parses the file,
infers its contract, caches both, and returns the projected arrow. This is
the strategy described in §4 (two caches).

```
source: import sort from "collection/sort"
  ↓ at first access
load collection/sort.hnl → parse → infer → cache
return Project { field: "sort", record: <cached Scheme> }
```

- **Pros**: Lazy — only load what's used. Supports dynamic loading and
  REPL workflows. Module boundaries preserved for debugging. Cache is
  shared across imports.
- **Cons**: Runtime loading overhead (amortized by cache). Inference must
  handle App references to cached contracts (cannot always trace through).

#### Choosing a strategy

The three strategies are not mutually exclusive. A compiler might use
Strategy A (compile-time desugar) for production builds, Strategy B
(whole-program flattening) for whole-program inference, and Strategy C
(runtime loading) for REPL/development. The semantic abstraction — import
= projection — is the same in all three. Only the timing and representation
differ.

The inference engine is agnostic to the strategy: it sees either
`Project` nodes (A, C) or inlined subgraphs (B), and either `App` nodes
(A, C) or resolved arrows (B). The contract cache (§4) is used in all
strategies — even flattening benefits from caching inferred contracts for
shared subgraphs.

### 6. No export, no visibility, no environments

The arrow-native model does not need:

- **`export`** — all named arrows in a record are accessible via
  projection. There is no visibility layer.
- **First-class environments** — arrows are already first-class. A record
  of arrows is just a product type, not a new runtime concept.
- **Field access on modules** — import binds names directly into the
  current scope via projection. No `module.field` syntax needed.

If encapsulation or qualified access becomes necessary later, they can be
added as sugar without changing the core model.

### 7. Multi-file programs are arrow composition

A multi-file program is just arrows composed across files:

```handl
// collection/insert.hnl
loop(comp(compare, choice(cons, cons)))

// collection/sort.hnl
import insert from "collection/insert"
loop(comp(insert, first(id)))

// main.hnl
import sort from "collection/sort"
sort([3, 1, 2])
```

Normalized (Strategy A — compile-time desugar):

```handl
// collection/insert.hnl → arrow A_insert: List α ⇝ List α
// collection/sort.hnl → arrow A_sort: List α ⇝ List α
//   where insert = project(insert, A_insert)  // A_insert is single-expr, so insert = A_insert
//   and the file's arrow is loop(comp(insert, first(id)))
// main.hnl → arrow A_main: Unit ⇝ List Int
//   where sort = project(sort, A_sort)  // A_sort is single-expr, so sort = A_sort
//   and the file's arrow is const([3, 1, 2]) >>> sort
```

Each file is an arrow. Imports normalize to projections. The whole program
is a composition of file-arrows. No new concepts.

## Behavior examples

### Example 1: simple import

Filesystem layout:
```text
project/
  main.hnl
  collection/
    insert.hnl
    sort.hnl
```

`collection/insert.hnl`:
```handl
loop(comp(compare, choice(cons, cons)))
```

`collection/sort.hnl`:
```handl
import insert from "collection/insert"
loop(comp(insert, first(id)))
```

`main.hnl`:
```handl
import sort from "collection/sort"
sort([3, 1, 2])
```

Behavior (Strategy C — runtime loading):
1. `main.hnl` imports `sort` from `collection/sort`
2. `collection/sort.hnl` imports `insert` from `collection/insert`
3. `collection/insert.hnl` is parsed → arrow graph cached
4. `insert` contract inferred (e.g., `Sorted(output)`) → cached
5. `sort` graph parsed (with `App { callee: "insert" }`) → cached
6. `sort` contract inferred (using `insert`'s cached contract) → cached
7. `main.hnl` runs: `const([3, 1, 2]) >>> sort` → `[1, 2, 3]`

Under Strategy A (compile-time desugar), steps 2–6 happen at normalization
time, producing a self-contained IR with `Project` and `App` nodes. Under
Strategy B (flattening), steps 2–6 produce a single inlined graph with no
`Project` or `App` nodes.

### Example 2: multi-name file (record arrow)

```handl
// collection.hnl
cons = arr("list.cons")
head = arr("list.head")
tail = arr("list.tail")
sort = loop(comp(insert, first(id)))
```

This file's arrow is `Unit ⇝ { cons, head, tail, sort }`.

```handl
// main.hnl
import sort, head from "collection"
head(sort([3, 1, 2]))
```

Normalized:
```handl
collection = <collection.hnl's arrow>  // produces the record
sort = project(sort, collection)
head = project(head, collection)
const([3, 1, 2]) >>> sort >>> head
```

### Example 3: contracts compose across files

```handl
// collection/insert.hnl
loop(comp(compare, choice(cons, cons)))
// Inferred contract: Sorted(output)

// collection/sort.hnl
import insert from "collection/insert"
loop(comp(insert, first(id)))
// Inferred contract: Sorted(output) — derived from insert's cached contract

// main.hnl
import sort from "collection/sort"
import map from "fn/map"
import filter from "fn/filter"
comp(comp(map(double), filter(is_even)), sort)
// Inferred contract: Sorted(output) — derived from sort's cached contract
```

Each file is inferred once. Contracts compose via the cache. No
re-inference of imported arrows. This holds under all three strategies —
even flattening benefits from caching inferred contracts for shared
subgraphs (e.g., when `map` is imported by multiple files).

### Example 4: import is idempotent

```handl
import insert from "collection/insert"
import insert from "collection/insert"
```

Behavior (all strategies):
- The first import normalizes to `project(insert, <collection/insert's arrow>)`
- The second import normalizes to the same projection
- Under Strategy C, the cache ensures the file is parsed and inferred once
- Under Strategies A/B, normalization is deterministic — both produce the
  same projection node, deduplicated by the compiler

## Composition: the only 5 operations

There are exactly 5 ways to compose arrows. No more. The module system
introduces zero new ones:

```
1. Sequential:  a >>> b        (Seq)    — output of a feeds into b
2. Parallel:    first(a)       (First)  — a runs on first component, rest passes through
3. Conditional: a ||| b        (Choice) — run a OR b based on branch
4. Iterative:   loop a         (Loop)   — feedback, repeat until done
5. Reference:   app("sort")    (App)    — insert a named arrow here
```

Files, imports, and modules are sugar over these 5 combinators:

- **File** = an arrow (single expression) or `Unit ⇝ Record` (multi-name)
- **Import** = `project(name, <file's arrow>)` — field projection, which is
  `First` + `Arr`. This is a **normalization**: import syntax desugars to
  projection, and the projection is realized by whichever strategy (§5) the
  implementation chooses.
- **Using a name** = `App { callee: "name" }` — reference composition
- **Multi-file program** = composition of file-arrows via projection + App

No function calls. No method dispatch. No inheritance. No statement
ordering. No environments. Just arrows, records, and the 5 combinators.

## Consequences

### Positive

- Multi-file `.hnl` programs work naturally for arrow-native code.
- **No new concepts**: modules are arrows, imports normalize to
  projections, calls are App. The module system is the arrow algebra.
- **Flexible normalization**: the import→projection abstraction admits
  multiple implementation strategies (compile-time desugar, whole-program
  flattening, runtime loading). The semantics are fixed; the strategy is
  chosen per use case (production build, whole-program inference, REPL).
- No first-class environment value needed — arrows are already first-class,
  records are already product types.
- No `export` keyword or visibility layer — simpler syntax.
- Compositional inference: each module inferred once, contracts cached and
  reused. Works under all three strategies.
- The module system and the inference system share the same architecture:
  a module's public surface is its arrow + its inferred contract.

### Negative / trade-offs

- No encapsulation: all named arrows in a record are accessible via
  projection. Private helpers cannot be hidden from importers. (Can be
  addressed later with explicit `export` as sugar.)
- No qualified namespace access (`module.function`). All imports are flat
  name bindings. (Can be added later with `import "m" as alias` sugar.)
- Two caches (graphs + contracts) instead of one. Slightly more machinery
  than ADR-0056's single environment cache. (Under Strategy B — flattening
  — the cache is optional, but still useful for shared subgraphs.)
- The compiler must resolve `App` nodes to cached contracts during
  inference. This is a new responsibility not present in ADR-0056.
- Strategy choice affects tooling: a REPL wants Strategy C, a release
  build wants Strategy A or B. The toolchain must support multiple
  normalization passes.

### Compatibility

This design supersedes the module model in ADR-0056 for arrow-native
programs. ADR-0056's environment-based model remains valid for any
statement-based evaluation that handl may support in the future, but the
arrow-native module system does not depend on it.

## Rationale

ADR-0056 assumed a statement-based language where modules are produced by
evaluating sequences of bindings. Handl is arrow-native: programs are
declarative graphs, not imperative statement sequences.

The key insight: **a file is an arrow.** A multi-name file is an arrow that
produces a record of named arrows. **Import normalizes to projection** on
that record. Using an imported name is App composition. The entire module
system collapses into the arrow algebra — no new concepts, no new runtime
objects, no environments, no field access machinery.

The second insight: **normalization is an abstraction, not an
implementation.** The meaning of `import` is fixed (project a name from a
record arrow), but the realization is flexible. Compile-time desugaring,
whole-program flattening, and runtime module loading are three valid
strategies for the same abstraction. This lets the toolchain choose the
right strategy per context — eager for production, lazy for development,
flattened for whole-program inference — without changing the semantics.

This is simpler than ADR-0056 (fewer concepts, no environment reification,
flexible implementation) and integrates naturally with the refinement
inference engine (contracts are cached per module, inference is
compositional via App resolution, and the normalization strategy is
transparent to the inference engine).

## Implementation notes

The implementation is organized around the normalization abstraction. Each
phase adds a strategy or refines an existing one.

### Phase 1 — files as arrows

Support single-expression `.hnl` files. Each file parses to one `Scheme`
arrow graph. Running the file runs the arrow. This is the foundation all
strategies build on.

### Phase 2 — records and multi-name files

Support `name = arrow` syntax in `.hnl` files, desugaring to record
literals. A multi-name file's arrow is `Unit ⇝ Record`.

### Phase 3 — import normalization (Strategy A: compile-time desugar)

Add `import name from "module"` syntax. During normalization, resolve
module paths and emit `Project { field, record }` nodes in the Scheme IR.
After normalization, no import syntax remains — only projections and App.
This is the default strategy for compiled builds.

### Phase 4 — contract cache and compositional inference

Infer contracts for each imported arrow on first import. Cache them. When
the inference engine encounters `App { callee: "name" }`, look up the
cached contract instead of tracing through the graph. This works under
both Strategy A (Project nodes reference cached records) and Strategy C
(runtime-loaded records).

### Phase 5 — runtime module loading (Strategy C)

Add a runtime module loader for REPL and development workflows. The loader
parses files lazily, infers contracts on first access, and caches both
graphs and contracts. Imports normalize to the same `Project` nodes, but
the record arrow is produced by the loader at runtime rather than embedded
in the IR at compile time.

### Phase 6 — whole-program flattening (Strategy B)

Add a whole-program flattening pass that resolves all `Project` and `App`
nodes to their target subgraphs, producing a single self-contained Scheme
graph with no indirection. Useful for whole-program inference (the engine
traces the entire program in one pass) and for producing minimal artifacts.
Shared subgraphs are deduplicated; their inferred contracts are reused from
the cache.

### Phase 7 — stdlib as importable arrow files

Move the stdlib arrow definitions (`kit-stdlib/data/arrows/`) to be
importable `.hnl` files. Verify the full cascade works: atoms → insert →
sort → user programs, all via imports with compositional inference, under
each of the three strategies.

## Related ADRs

- [ADR-0056 — Import / Export and Multi-File Programs as Evaluated Environments](./0056-import-export-and-multi-file-programs.md)
- [ADR-0049 — Modules are Evaluated Environments](./0049-modules-are-evaluated-environments.md)
- [ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)
- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)

## Summary

A file is an arrow. A multi-name file is an arrow that produces a record of
named arrows. **Import normalizes to projection** — this is the semantic
abstraction. Using an imported name is App composition. The module system
is the arrow algebra — no new concepts, no environments, no export
visibility.

The normalization abstraction admits three implementation strategies:
compile-time desugar (eager, static IR), whole-program flattening (eager,
inlined single graph), and runtime module loading (lazy, cached). All
three realize the same semantics. Inference is compositional under all
three: each file inferred once, contracts cached and reused via App
resolution.
