# ADR-0009: Strategy C — Runtime Module Loading as Handler Interpreted Ops

## Status

Proposed

Builds on [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md) (§5 Strategy C).
References [ADR-0008 — Pluggable LSP](./0008-pluggable-lsp-language-agnostic-server.md).

## Context

ADR-0069 defines three normalisation strategies for module imports. Strategies A (compile-time desugar) and B (whole-program flattening) are implemented: they eliminate `module_ref`/`project` ops from the `Scheme` graph before the handler sees them. Strategy C (runtime module loading) is described but not implemented.

ADR-0069 §5 describes Strategy C as "imports are resolved on first access" via a runtime module loader. The original framing implied a separate mechanism — a runtime loader, a directive, a capability flag. That framing is wrong. It adds concepts the language doesn't need.

### The insight

Module loading is already two `arr` ops:

```
module_ref("collection/sort")  →  arr { op: "module_ref", lit: "collection/sort" }
project(record, "sort")        →  arr { op: "project", args: [record], lit: "sort" }
```

These are ordinary `arr` ops. A handler that has operation clauses for `module_ref` and `project` interprets them at runtime — loading the file, evaluating it, returning the arrow record, extracting the field. A handler that doesn't have those clauses either normalises them away first (Strategy A/B) or refuses to interpret the graph.

**The handler's operation table is the opt-in.** No directive, no capability flag, no new mechanism. The same machinery that handles `arr add`, `arr const`, `arr list.head` handles `arr module_ref` and `arr project`.

### What module loading actually is

Module loading is sugar for projection of evaluated code:

- `module_ref(path)` = "evaluate this file, give me the record of public arrows"
- `project(record, field)` = "give me this field from the record"

That's it. The handler's `module_ref` clause calls the resolver, evaluates the module, returns the record. The `project` clause extracts the field. Existing handler machinery, no new language concept.

### What about static analysis?

A common objection to runtime loading: "you lose static whole-program analysis." This is false. The `module_ref`/`project` ops are visible in the `Scheme` graph. You can:

- **Detect cycles statically:** walk the graph, collect `module_ref(path)` nodes, build the import graph, detect cycles. No execution needed.
- **Infer contracts:** register `module_ref` and `project` in `ContractRegistry` with permissive contracts (input: Any, output: Any). For tighter contracts, load referenced modules' contracts at analysis time and compose — without inlining code.
- **Type check:** the ops have types, the graph is closed, analysis works.

What you lose is *precision* (permissive contracts for the ops vs. exact contracts for inlined arrows), not the capability itself. And precision is recoverable by loading referenced modules' contracts without inlining their code.

## Decision

Strategy C is **not a separate strategy**. It is what happens when you don't normalise `module_ref`/`project` away and the handler interprets them. The handler's operation table is the opt-in mechanism.

### 1. The ops are the mechanism

`module_ref` and `project` are `arr` ops, already emitted by `rewrite_imports` in the parser. They are already in the `Scheme` graph. No new `Scheme` variants, no new language constructs.

A handler with operation clauses for these ops:

```
handle module_ref(path) {
  -- resolve path, load file, evaluate, return record of public arrows
  loader.load(path) >>> evaluate
}

handle project(record, field) {
  -- extract field from record
  record.field
}
```

A handler without these clauses: the graph must be normalised (Strategy A/B) before interpretation, or the handler refuses with "unknown op: module_ref".

### 2. No directive, no capability flag

The original Strategy C framing proposed:
- A `-- @strategy runtime` program directive → **removed.** The handler decides by having (or not having) clauses.
- A `supports_runtime_loading` handler capability → **removed.** That's just "does the operation table include `module_ref`?"
- A separate "Strategy C" code path → **removed.** It's the same graph, interpreted by a handler that handles the ops.

### 3. Governance is handler configuration

The handler's `module_ref` clause is where governance lives:

- **Path allowlist:** the clause checks `path` against `allowed_roots` before resolving. Paths outside → error.
- **Depth limit:** the clause tracks the active load stack. Exceeds `max_depth` → error.
- **No network by default:** which resolvers are wired into the handler's loader is handler config. `FileResolver` only by default; `NetworkResolver` only if explicitly configured.
- **Cycle detection at runtime:** the loader tracks the active load stack. A cycle → `LoadError::Cycle`, not an infinite loop.

This is handler config — the same place every other capability is governed. No workspace-level policy file, no program-level directive.

### 4. Static analysis passes (no execution needed)

Two static passes over the `Scheme` graph, both already implementable with the current op encoding:

**Cycle detection:**
```
fn detect_cycles(scheme: &Scheme) -> Result<(), CycleError>
  - walk scheme, collect all module_ref(path) nodes
  - build import graph: path → set of paths it imports
  - detect cycles via DFS
```

**Contract inference:**
- Register `module_ref` and `project` in `ContractRegistry` with permissive contracts.
- For tighter contracts: load referenced modules' contracts (via `ModuleLoader`'s contract cache), compose without inlining code.
- `infer_refinement` works on the whole graph without modification.

### 5. The `Scheme` variant question

`module_ref`/`project` now reach interpretation under this design. They remain `arr` ops, not real `Scheme` variants. The handler matches on `op: "module_ref"` stringly, same as every other op.

Graduate to real `Scheme::ModuleRef`/`Scheme::Project` variants only if:
- A second handler needs to interpret them (one handler matching a string is fine; two duplicating the convention is the signal), AND
- The stringly-typed matching causes real problems (bugs, missed cases).

Until then, the closed kernel principle (ADR: closed arrow calculus kernel) holds: six combinators, no more.

## Consequences

**Positive:**

- No new language mechanism. Module loading uses the existing handler `op` machinery. Zero new concepts.
- Opt-in is automatic: a handler that handles `module_ref`/`project` does runtime loading; one that doesn't doesn't. No configuration beyond the operation table.
- Static analysis is preserved. Cycle detection and contract inference work on the graph without execution.
- Enables REPLs, hot-reload, plugin systems, dynamic loading — all via handler clauses, no special-casing.
- Composes with Strategies A/B: normalise when you want a closed artifact, don't normalise when you want runtime loading. Same graph, different handler.

**Negative:**

- Less precise contracts than inlining (permissive `Any` contracts for the ops vs. exact contracts for inlined arrows). Recoverable by loading referenced modules' contracts without inlining code.
- Runtime loading overhead (amortised by the module loader's cache).
- The handler must implement governance (allowlist, depth, cycle detection) in its `module_ref` clause. This is handler code, not language machinery — but it's still code someone must write and get right.

**Risk:**

- Handler authors must implement governance correctly. A handler that resolves `module_ref` without checking the allowlist or depth limit is a security hole. Mitigation: provide a standard `module_ref` clause implementation in the stdlib that handles governance, so handler authors don't write it from scratch.

## Implementation (what's left)

The infrastructure already exists from ADR-0069 implementation:

- `ModuleResolver` trait + `FileResolver`/`MemoryResolver`/`ChainResolver`/`CachingResolver` — done
- `ModuleLoader` with graph + contract caches — done
- `module_ref`/`project` op encoding (`rewrite_imports`) — done
- `normalize_compile_time` / `flatten_program` (Strategies A/B) — done

What's left:

1. **Handler `module_ref`/`project` clauses** — a standard implementation in the stdlib that calls the resolver, evaluates, returns the record/field, with governance (allowlist, depth, cycle detection).
2. **`module_ref`/`project` contracts in `ContractRegistry`** — permissive contracts for static inference without inlining.
3. **Static cycle detection pass** — walk the graph, collect `module_ref` nodes, build import graph, detect cycles.
4. **Handler config for governance** — allowlist, depth limit, resolver wiring.

## Relationship to ADR-0069

This ADR refines ADR-0069 §5 Strategy C. It does not contradict ADR-0069 — the semantic model (import = projection) is unchanged. It clarifies the implementation: Strategy C is not a separate mechanism but the handler interpreting ops that Strategies A/B would eliminate. ADR-0069's description of Strategy C as "imports are resolved on first access" is accurate; this ADR specifies *how* (handler clauses, not a directive or capability flag).
