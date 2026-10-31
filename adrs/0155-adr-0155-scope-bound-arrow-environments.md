+++
id = "ADR-0155"
title = "ADR-0155: Scope-Bound Arrow Environments"
status = "open"
priority = "medium"
+++

---
title: Scope-Bound Arrow Environments
type: adr
date: 2026-10-22
status: landed
status_note: Landed — dep bindings are arrows carrying their env; all three engines pass the plan gates.
---

# ADR-0155: Scope-Bound Arrow Environments

## Decision

1. **A dependency binding is a callable value** — one payload kind, **arrows**:
   `.hnl`-authored deps and wrapped native deps. No second callable kind
   (ADR-0106).
2. **Name resolution does not change per engine.** `proj_env`'s scope-first
   lookup with string fallback stays; what changes is the environment record a
   arrow carries.
3. **`ModuleEnv` (ADR-0156) becomes the builder** of the lexical environment
   record; its shape is unchanged.
4. **Bootstrap is unchanged**: `HandlLoader::new` builds the loader raw over
   `char` + `str-handl` and runs it on bytecode.
5. **Caching keys on `(module_id, env_id)`**: minted arrows capture the env, so
   `MODULE_VALUE_CACHE` can no longer key on `module_id` alone.
6. **Staged plan**: `callv` node + wrap helpers → engine switch → cache keys,
   each gated by tree-walk and bytecode tests.
7. **Rejected: `TaggedValue::NativeFn(Rc<NativeFnHandle>)`** — a second callable
   kind at the value layer contradicts the pure arrow calculus (ADR-0106).

## Why

Dependency functions were wrapped in engine-specific closures, so a
scope-bound arrow could not capture the environment it was defined in and each
engine resolved names its own way. Making the binding an arrow with its env
record gives one resolution rule and serializable exports.

## Not doing
- **A `NativeFn` value kind** — the arrow is the only function former.
- **Per-engine name resolution** — resolution is shared; env records differ.
- **Copy the environment per engine** — the record is a value, carried by the
  arrow.

## Consequences
- Environment records become values; cache keys include `env_id`.
- All engines share one resolution path and one wrap helper.
- A native dep is wrapped once into an arrow, not once per engine.

## Enforcement
- `kernel/` — `proj_env`, `ModuleEnv`.
- `handl-runtime/` — `MODULE_VALUE_CACHE` keying.
- Tree-walk and bytecode tests from the staged plan.

## Design
+The callable-value shapes, resolution, interop, caching and the staged plan:
+[`design/0155-scope-bound-arrow-environments.md`](../design/0155-scope-bound-arrow-environments.md).
