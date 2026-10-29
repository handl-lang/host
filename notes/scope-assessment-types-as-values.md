---
title: 'Scope Assessment: Types as handl Values'
type: note
date: 2026-10-15
---

# Scope Assessment: Types as handl Values

## Current Architecture

### Type System (Rust-native)

| Crate                       | Role                                                                      | Lines (approx) | Key Types                                                                                                                |
| --------------------------- | ------------------------------------------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `typecheck-core`            | Type enum, unification, compatibility, trait registry, type env           | ~800           | `Type`, `FunctionType`, `TypeParam`, `Variance`, `Kind`, `Substitution`, `TypeEnv`, `TraitRegistry`, `SchemaInterpreter` |
| `typecheck-infer`           | Expression parsing, type inference, call checking                         | ~580           | `ExprAst`, `FunctionTable`, `ParseCache`, `infer()`, `check_call_detailed()`                                             |
| `typecheck-schema-json`     | JSON Schema → `Type` interpretation                                       | ~300           | `JsonInterpreter`                                                                                                        |
| `typecheck-key-expr`        | Check `expr:` fields                                                      | ~120           | `check_field()`, `check()`                                                                                               |
| `typecheck-key-arg`         | Check `arg:` fields                                                       | ~100           | `check()`                                                                                                                |
| `typecheck-key-merge`       | Check `merge:` fields                                                     | ~150           | `check()`                                                                                                                |
| `typecheck-key-inherit`     | Check `inheritable:` fields                                               | ~200           | `check()`                                                                                                                |
| `typecheck-key-mutable`     | Check `mutable:` fields                                                   | ~80            | `check()`                                                                                                                |
| `typecheck-key-termination` | Call-graph cycle detection                                                | ~190           | `check()`, `extract_function_bodies()`                                                                                   |
| `typecheck-import`          | Cross-module type checking                                                | ~200           | `check_imports()`                                                                                                        |
| `compiler-pass-typecheck`   | Orchestrator — assembles env, function table, runs all checks in parallel | ~190           | `run_typecheck()`                                                                                                        |

**Total: ~2,900 lines of Rust typechecking code**

### Host System (already decoupled)

| Crate                                                                          | Role                                  | Key Types                                                                                 |
| ------------------------------------------------------------------------------ | ------------------------------------- | ----------------------------------------------------------------------------------------- |
| `vm`                                                                           | VM executing `Expr<NonFunctionValue>` | `Host` trait, `Function`, `AsyncFunction`, `LazyFunction`, `GenericFunction`, `Operation` |
| `runtime`                                                                      | Runtime values                        | `NonFunctionValue` (wraps `serde_json::Value`)                                            |
| `runtime-core`                                                                 | Module system                         | `Module` trait, `ModuleHost`                                                              |
| `host`                                                                         | Default pure host                     | `DefaultHost` (add, mul, concat, len)                                                     |
| `lib-utils`                                                                    | Utility functions                     | string, array, math functions                                                             |
| `lib-datetime`, `lib-intl`, `lib-crypto`, `lib-rand`, `lib-kv`, `lib-platform` | Capability-scoped libraries           | Various host functions                                                                    |

### What Already Works for "Types as Values"

1. **Host is generic over capabilities** — `Host` trait has no built-in functions. A pure host (only add/mul/concat/len) is already valid.
2. **Function values are JSON** — `{ "params": [...], "body": "mul(x, 2)" }` evaluates under any host providing `mul`.
3. **Same expression syntax** — `${...}`, `expr:`, `body:` work identically regardless of host.
4. **Depth limit exists** — runtime enforces a depth limit (currently 64/256) for termination.
5. **`NonFunctionValue` wraps `serde_json::Value`** — types as JSON values would be native runtime values.

## What Would Need to Change

### Phase 1: Type Representation (Medium — ~500 lines new, ~800 lines retired)

**Replace:** `Type` enum in `typecheck-core/src/type_.rs`

```rust
// Current: Rust enum
enum Type { String, Number, Array(Box<Type>), Object(BTreeMap<String, Type>), ... }
```

**With:** JSON values interpreted by a "type host"

```json
// New: JSON values
{ "kind": "string" }
{ "kind": "number" }
{ "kind": "array", "element": { "kind": "string" } }
{ "kind": "object", "properties": { "name": { "kind": "string" } } }
{ "kind": "function", "params": [...], "return": {...}, "typeParams": [...] }
{ "kind": "var", "name": "T" }
{ "kind": "unknown" }
```

**Affected:**

- `typecheck-core/src/type_.rs` — Type enum → JSON encoding/decoding
- `typecheck-core/src/unify.rs` — `Substitution` becomes `HashMap<String, JsonValue>`
- `typecheck-core/src/compat.rs` — `is_compatible()` becomes an handl function
- `typecheck-core/src/trait_registry.rs` — trait registry becomes host config
- `typecheck-core/src/env.rs` — `TypeEnv` becomes `HashMap<String, JsonValue>`
- `typecheck-core/src/interpreter.rs` — `SchemaInterpreter` already returns `Type`; would return `JsonValue`

### Phase 2: Type Operations as handl Functions (Large — ~1,000 lines new)

**Replace:** Rust functions with handl function values running under a pure "type host"

| Rust function                        | handl equivalent                | Host primitives needed                            |
| ------------------------------------ | ------------------------------- | ------------------------------------------------- |
| `is_compatible(actual, expected)`    | `compatible(actual, expected)`  | None — pure structural recursion                  |
| `unify(actual, pattern, subst)`      | `unify(actual, pattern, subst)` | None — pure structural recursion                  |
| `substitute(ty, subst)`              | `apply_subst(subst, type)`      | None — pure tree walk                             |
| `infer_ast(ast, env, functions)`     | `infer(expr, env, functions)`   | `parse(expr)` — already exists in dialect         |
| `check_bounds(subst, type_params)`   | `check_bounds(subst, params)`   | `trait_implements(trait, type)` — registry lookup |
| `check_call_detailed(fn_type, args)` | `check_call(fn_type, args)`     | Calls `unify`, `substitute`                       |

**The "trusted core"** — irreducible host primitives the type host must provide:

1. `parse(expr_string)` → AST value (already exists as `parse_expr` in dialect)
2. `lookup(env, name)` → type value (environment lookup)
3. `trait_implements(trait_name, type)` → boolean (registry lookup)

Everything else (`compatible`, `unify`, `substitute`, `infer`, `check_call`) can be written in handl itself.

### Phase 3: Type Host Configuration (Small — ~200 lines)

A `TypeHost` struct implementing `Host` that provides:

- Pure functions only (add, mul, concat, len, map, filter)
- Type manipulation primitives (unify, compatible, infer, subst, lookup, trait_implements)
- No I/O, no operations, no async functions
- Depth-limited (existing runtime mechanism)

### Phase 4: Migration of Key Checkers (Medium — ~600 lines rewritten)

Each `typecheck-key-*` crate would become an handl function:

- `typecheck-key-expr` → `check_expr(field, schema, env, functions)` as handl
- `typecheck-key-arg` → `check_arg(field, schema, env)` as handl
- `typecheck-key-merge` → `check_merge(field, schema, env)` as handl
- etc.

### Phase 5: Bootstrapping (Hard — design problem)

The type host's own primitives need types. Options:

1. **Untyped trusted core** — accept that `unify`, `infer`, etc. are untyped host functions (like Agda's trusted core)
2. **Self-typing** — the type host can type itself (circular, but finite — the primitives are a fixed set)
3. **External metatheory** — types for type operations are defined in a separate metalevel (like Lean's universe hierarchy)

Option 1 is most practical. The trusted core is small (~5 functions) and can be verified by inspection.

## Migration Strategy

### Incremental (recommended)

1. **Keep the Rust typechecker as-is** — it's fast (benchmarked, optimized) and correct
2. **Add a `type-host` crate** — implements `Host` with type manipulation primitives
3. **Add type-as-value encoding** — JSON encoding of `Type` alongside the Rust enum
4. **Port one operation** — start with `is_compatible` as an handl function, verify it matches the Rust version
5. **A/B test** — run both Rust and handl versions, compare results
6. **Gradually replace** — port one key checker at a time, keeping Rust as fallback
7. **Eventually retire** Rust typechecker when handl versions are proven equivalent

### All-at-once (not recommended)

Would require rewriting ~2,900 lines simultaneously with no fallback. High risk.

## Size Estimates

| Component                    | New lines  | Retired lines | Risk   |
| ---------------------------- | ---------- | ------------- | ------ |
| Type encoding (JSON ↔ Type)  | ~200       | ~200          | Low    |
| Type host                    | ~200       | 0             | Low    |
| `compatible` as handl        | ~80        | ~60           | Medium |
| `unify` as handl             | ~100       | ~80           | Medium |
| `substitute` as handl        | ~50        | ~30           | Low    |
| `infer` as handl             | ~150       | ~200          | High   |
| `check_call` as handl        | ~100       | ~80           | Medium |
| Key checkers (6)             | ~600       | ~600          | Medium |
| Trait registry → host config | ~100       | ~190          | Low    |
| Bootstrapping                | ~100       | 0             | High   |
| **Total**                    | **~1,680** | **~1,440**    |        |

## Key Challenges

### 1. Performance

The Rust typechecker is optimized (HashMap TypeEnv, ParseCache reuse, single-scan unify). handl interpretation adds overhead:

- JSON value manipulation vs direct enum matching
- VM evaluation overhead per function call
- No compile-time optimization of handl functions

**Mitigation:** The type host can cache results. Type checking is deterministic — same input always produces same output. A memoization layer on the type host would recover most of the overhead.

### 2. Generic Functions / HKTs

The current system supports type parameters, trait bounds, variance, and higher-kinded types. Reimplementing these as handl functions requires careful encoding:

- `TypeParam` → `{ "name": "T", "bounds": [...], "variance": "invariant" }`
- `App(ctor, args)` → `{ "kind": "app", "constructor": {...}, "args": [...] }`
- `Kind` → `{ "kind": "type" }` or `{ "kind": "constructor", "args": [...], "result": {...} }`

### 3. Termination

The depth limit (64/256) is crude. The current Rust typechecker has compile-time termination analysis (call-graph cycle detection). For handl type functions:

- The depth limit still applies (runtime enforcement)
- The termination analysis can check type-level functions too (same mechanism)
- Structural recursion checks (like Agda) would be a future improvement

### 4. Bootstrapping

The type host provides `unify`, `infer`, etc. as host functions. These need types themselves. Practical approach: treat them as untyped primitives (like Rust's `Box<dyn Function>`). The type host is a trusted core, verified by inspection and testing, not by self-typing.

## What Doesn't Need to Change

- **VM** — already generic over `Host`, no changes needed
- **Runtime** — `NonFunctionValue` already wraps `serde_json::Value`, JSON types are native
- **Expression parser** — `parse_expr` already produces `ExprAst`, usable from any host
- **Dialect** — syntax definitions are host-independent
- **Compiler pipeline** — `compiler-pass-typecheck` would call the type host instead of Rust functions, but the pipeline structure stays the same
- **Frontend** — YAML/JSON/TOML parsing is unaffected
- **Codebase/AST** — `Expr` type is unchanged

## Summary

| Dimension              | Assessment                                                                        |
| ---------------------- | --------------------------------------------------------------------------------- |
| **Feasibility**        | High — the host decoupling mechanism already exists                               |
| **Effort**             | ~1,680 new lines, ~1,440 retired lines, ~2,900 lines affected                     |
| **Risk**               | Medium — incremental migration with A/B testing mitigates                         |
| **Performance impact** | Negative short-term (interpretation overhead), recoverable via memoization        |
| **Bootstrapping**      | Solvable — small trusted core (~5 untyped primitives)                             |
| **Key benefit**        | User-extensible type-level computation in the same language — no typefest problem |
| **Timeline**           | 5 phases, each independently shippable                                            |
