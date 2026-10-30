---
title: Strategy C — Runtime Module Loading as Handler Interpreted Ops — design detail
type: design
date: 2026-10-16
updated: 2026-10-29
refs: [ADR-0009]
---

# ADR-0009: Strategy C — Runtime Module Loading as Handler Interpreted Ops — design detail

Moved verbatim from ADR-0009 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

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

## Why

ADR-0069 defines three normalisation strategies for module imports. Strategies A (compile-time desugar) and B (whole-program flattening) are implemented: they eliminate `module_ref`/`project` ops from the `Scheme` graph before the handler sees them. Strategy C (runtime module loading) is described but not implemented.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- No new language mechanism. Module loading uses the existing handler `op` machinery. Zero new concepts.
- Opt-in is automatic: a handler that handles `module_ref`/`project` does runtime loading; one that doesn't doesn't. No configuration beyond the operation table.
- Static analysis is preserved. Cycle detection and contract inference work on the graph without execution.
- Enables REPLs, hot-reload, plugin systems, dynamic loading — all via handler clauses, no special-casing.
- Composes with Strategies A/B: normalise when you want a closed artifact, don't normalise when you want runtime loading. Same graph, different handler.

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0009-strategy-c-runtime-module-loading.md`](../design/0009-strategy-c-runtime-module-loading.md).
