---
title: Data-Driven AST Child Roles — Closing the Last Engine Hardcoding
type: adr
date: 2026-09-04
status: accepted
status_note: |-
  Accepted. Phases 1–4 are implemented and the Phase 4 cross-language acceptance test passes (renamed kinds `set`/`block2`/`ret`/`params2`/`func2` with identical roles run through unchanged tree-walk and bytecode engines). Phase 5 (v1 deletion) remains coordinated with the [ADR-0052](./0052-arrow-calculus-kernel.md) cutover and is not blocking acceptance — the v2 engines are role-aware and the v1 path is no longer extended.
---


# ADR-0081: Data-Driven AST Child Roles — Closing the Last Engine Hardcoding

## Decision


Add **named child roles** to `AstShapeDef`; resolve them to positional indices at assembly time. Engine, native-op set, and kernel combinators stay unchanged.

### 1. `AstShapeDef` gains optional `children`

```yaml
- id: ast.assign
  node_kind: assign
  children:
    - role: target
    - role: value
- id: ast.block
  node_kind: block
  children:
    - role: statements
      variadic: true
```

`children: Option<Vec<ChildRoleSpec>>`. Absent = opaque, positional fallback (backward compatible). `variadic: true` resolves to `[start_index..]`.

### 2. `proj_child` accepts a role name

```
arr_native_lit("proj_child", [], 0u64)        # positional (existing)
arr_native_lit("proj_child", [], "target")    # by role (new)
```

The `lit` payload is already `Value`; accepting `String` is a payload-shape extension, not a new op.

### 3. Resolve at assembly time (preferred)

`handl-kit-data-canonical` rewrites `proj_child("role")` → `proj_child(index)` by looking up the role in `AstShapeDef`. The runtime engine sees only positional `proj_child(u64)` — zero engine changes. (Fallback: runtime resolution in the `proj_child` handler if assembly-time proves awkward. Settled in Phase 1.)

### 4. Rewrite the violating primitives

Replace `kind ==` checks with role lookups. `eval_block` uses `proj_child("statements")`; each statement's own eval scheme handles binding vs return. `closure` uses `proj_child("params")`. Typecheck uses `synth_child("role")` / `check_child("role")`.

### 5. Declare roles in `data/ast.yaml`

Add `children` to `assign`, `block`, `proc`/`function`, `paren`, `return`, `type-annot`.

### 6. Acceptance: a second language

Define a language with renamed kinds (`set`/`params`/`body`) but identical roles; run it through the **unchanged** engines. If they handle it without edits, the hardcoding is gone. Gate for marking this ADR Accepted.


## Why

ADR-0058 (Accepted) migrated four primitives (`var`/`call`/`object`/pattern-match) to data and declared "no new language-specific logic in engines." That claim holds for those four. But `eval_block`, `closure`, and typecheck shape dispatch still hardcode `kind ==` checks ADR-0058 didn't cover:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- See the design doc.

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0081-data-driven-ast-child-roles.md`](../design/0081-data-driven-ast-child-roles.md).
