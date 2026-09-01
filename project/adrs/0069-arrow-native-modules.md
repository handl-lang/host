# ADR-0069: Files Are Arrows — Arrow-Native Modules

## Status

Proposed

Builds on [ADR-0056 — Import / Export and Multi-File Programs as Evaluated Environments](./0056-import-export-and-multi-file-programs.md),
[ADR-0049 — Modules are Evaluated Environments](./0049-modules-are-evaluated-environments.md),
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

### 2. Import is field projection

Importing a name from a file is **projecting a field** from the file's
record arrow:

```handl
import sort from "collection/sort"
```

Is sugar for:

```handl
sort = project(sort, <run collection/sort.hnl's arrow>)
```

You run the file's arrow (producing a record), project the `sort` field,
and bind it as a name in the current scope. This is ordinary arrow
composition — no new concept.

For a single-expression file (no record), the import binds the file's
arrow directly:

```handl
// sort.hnl is just: loop(comp(insert, first(id)))
import sort from "collection/sort"
// sort is now bound to the file's arrow
```

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

### 5. No export, no visibility, no environments

The arrow-native model does not need:

- **`export`** — all named arrows in a record are accessible via
  projection. There is no visibility layer.
- **First-class environments** — arrows are already first-class. A record
  of arrows is just a product type, not a new runtime concept.
- **Field access on modules** — import binds names directly into the
  current scope via projection. No `module.field` syntax needed.

If encapsulation or qualified access becomes necessary later, they can be
added as sugar without changing the core model.

### 6. Multi-file programs are arrow composition

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

Desugared:

```handl
// collection/insert.hnl → arrow A_insert: List α ⇝ List α
// collection/sort.hnl → arrow A_sort: List α ⇝ List α
//   where insert = project(insert, A_insert)  // but A_insert is single-expr, so insert = A_insert
//   and the file's arrow is loop(comp(insert, first(id)))
// main.hnl → arrow A_main: Unit ⇝ List Int
//   where sort = project(sort, A_sort)  // A_sort is single-expr, so sort = A_sort
//   and the file's arrow is const([3, 1, 2]) >>> sort
```

Each file is an arrow. Imports are projections. The whole program is a
composition of file-arrows. No new concepts.

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

Behavior:
1. `main.hnl` imports `sort` from `collection/sort`
2. `collection/sort.hnl` imports `insert` from `collection/insert`
3. `collection/insert.hnl` is parsed → arrow graph cached
4. `insert` contract inferred (e.g., `Sorted(output)`) → cached
5. `sort` graph parsed (with `App { callee: "insert" }`) → cached
6. `sort` contract inferred (using `insert`'s cached contract) → cached
7. `main.hnl` runs: `const([3, 1, 2]) >>> sort` → `[1, 2, 3]`

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

Desugared:
```handl
collection = <run collection.hnl's arrow>  // produces the record
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
re-inference of imported arrows.

### Example 4: import is cached

```handl
import insert from "collection/insert"
import insert from "collection/insert"
```

Behavior:
- The first import resolves, parses, infers, and caches
- The second import returns the cached graph and contract
- The module is not re-parsed or re-inferred

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
- **Import** = `project(name, <file's arrow>)` — field projection, which is `First` + `Arr`
- **Using a name** = `App { callee: "name" }` — reference composition
- **Multi-file program** = composition of file-arrows via projection + App

No function calls. No method dispatch. No inheritance. No statement
ordering. No environments. Just arrows, records, and the 5 combinators.

## Consequences

### Positive

- Multi-file `.hnl` programs work naturally for arrow-native code.
- **No new concepts**: modules are arrows, imports are projections, calls
  are App. The module system is the arrow algebra.
- No first-class environment value needed — arrows are already first-class,
  records are already product types.
- No `export` keyword or visibility layer — simpler syntax.
- Compositional inference: each module inferred once, contracts cached and
  reused.
- The module system and the inference system share the same architecture:
  a module's public surface is its arrow + its inferred contract.

### Negative / trade-offs

- No encapsulation: all named arrows in a record are accessible via
  projection. Private helpers cannot be hidden from importers. (Can be
  addressed later with explicit `export` as sugar.)
- No qualified namespace access (`module.function`). All imports are flat
  name bindings. (Can be added later with `import "m" as alias` sugar.)
- Two caches (graphs + contracts) instead of one. Slightly more machinery
  than ADR-0056's single environment cache.
- The compiler must resolve `App` nodes to cached contracts during
  inference. This is a new responsibility not present in ADR-0056.

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
produces a record of named arrows. Import is field projection on that
record. Using an imported name is App composition. The entire module system
collapses into the arrow algebra — no new concepts, no new runtime objects,
no environments, no field access machinery.

This is simpler than ADR-0056 (fewer concepts, no environment reification)
and integrates naturally with the refinement inference engine (contracts
are cached per module, inference is compositional via App resolution).

## Implementation notes

### Phase 1 — files as arrows

Support single-expression `.hnl` files. Each file parses to one `Scheme`
arrow graph. Running the file runs the arrow.

### Phase 2 — records and multi-name files

Support `name = arrow` syntax in `.hnl` files, desugaring to record
literals. A multi-name file's arrow is `Unit ⇝ Record`.

### Phase 3 — import as projection

Add `import name from "module"` syntax. Resolve module paths, run the
file's arrow to produce the record, project the named field, bind it in
the current scope.

### Phase 4 — contract cache and compositional inference

Infer contracts for each imported arrow on first import. Cache them. When
the inference engine encounters `App { callee: "name" }`, look up the
cached contract instead of tracing through the graph.

### Phase 5 — stdlib as importable arrow files

Move the stdlib arrow definitions (`kit-stdlib/data/arrows/`) to be
importable `.hnl` files. Verify the full cascade works: atoms → insert →
sort → user programs, all via imports with compositional inference.

## Related ADRs

- [ADR-0056 — Import / Export and Multi-File Programs as Evaluated Environments](./0056-import-export-and-multi-file-programs.md)
- [ADR-0049 — Modules are Evaluated Environments](./0049-modules-are-evaluated-environments.md)
- [ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md)
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md)

## Summary

A file is an arrow. A multi-name file is an arrow that produces a record of
named arrows. Import is field projection. Using an imported name is App
composition. The module system is the arrow algebra — no new concepts, no
environments, no export visibility. Inference is compositional: each file
inferred once, contracts cached and reused via App resolution.
