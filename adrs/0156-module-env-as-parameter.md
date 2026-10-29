---
title: Module environments are parameters, not construction state
type: adr
date: 2026-10-21
status: proposed
applies_to: '`host` (`handl-plugin-lib-handl`), `kernel` (`handl-platform` —'
---

# ADR-0156: Module environments are parameters, not construction state

  trait surfaces only if needed)

## Context

A `.hnl` module's dependency environment is baked in at construction:
`HandlModule::with_dependency_arc` appends `Arc<dyn Module>` to a
`dependencies` vec and rebuilds the runtime builder. Two consequences:

1. **No multi-env.** The same source cannot be instantiated under two
   different dep bindings without rebuilding the module from scratch —
   and every sharing layer above (`TableImportResolver.cache`,
   `KitResolver`, `module_ref`, `ModuleLoader.cache`) is a name-keyed
   singleton, so one name can only ever mean one env.
2. **The env is implicit.** The dep set a module *expects* lives in
   call-site order of `with_dependency_arc` calls, not in declared
   data; dep-name discovery is text-scanning
   (`str_handl_import_specs`, `collect_direct_imports`,
   `host_import_bindings`).

The arrow-calculus framing suggests the cleaner model: a file is
`Env ~> Exports` — the environment is an *input*, applied, not
assembled. That is the ML-functor shape (`export` already rewrites to
record construction; `with_dependency_arc` is already
parameterization, just imperative).

## Decision

**Lift the environment into a first-class `ModuleEnv` value and add
`HandlModule::instantiate(&self, env: ModuleEnv) -> HandlModule`.**

- `ModuleEnv` owns `{ dependencies: Vec<Arc<dyn Module>>,
  global_lookup: Option<Arc<dyn ModuleLookup>>,
  import_resolver: Option<Arc<dyn ImportResolver>> }` — the tuple
  `build_runtime` already threads through.
- `instantiate` produces a sibling module sharing the prepared source,
  language, and eval engine, with a **fresh `module_id`**. Per-thread
  caches (`RUNTIME_CACHE`, `MODULE_VALUE_CACHE`, `AST_CACHE`) key on
  `module_id`, so env isolation costs nothing.
- `with_dependency*` / `with_global_lookup` / `with_import_resolver`
  remain as ergonomic sugar over the same path.
- `mod.fn` keeps dispatching through `ModuleLookup` — dep *functions*
  are native `Module` impls and cannot be arrow values today, so the
  registry boundary stays. (Scope-bound arrow environments — `str`
  as a record of arrows in lexical scope — are a separate,
  engine-level decision.)

**Env signatures come from data, not text scans**: a module's declared
deps resolve via `ModuleResolver` against names read from `pkg.hnl`
`deps` (the existing manifest) rather than `str_handl_import_specs`-style
source scanning where the manifest is authoritative. Text-scan
extraction remains for in-body `import`/`from` statements (grammar-level,
unchanged) and as the bootstrap path.

## Consequences

- Multi-env instantiation becomes a one-liner: `instantiate(env_a)` /
  `instantiate(env_b)` — enabling stub/mock/A-B dep surfaces for the
  research eval lanes without parallel module hierarchies.
- No engine, grammar, or `.hnl` source change; the public `HandlModule`
  API is additive.
- Name-keyed singleton caches above (`stdlib` tables, `KitResolver`,
  `ModuleLoader.cache`) still give one env per name — env-keyed
  variants are follow-up work where needed, not forced here.
  *Landed 2026-10-22:* `ModuleEnv` identity (`env_id`/`fork`, host
  `9971ceec`) + additive env-keyed variants — `get_handl_module_with_env`/
  `resolve_with_env` (kit-core `33b25b9`), `module_ref_with_env`/
  `load_hnl_file_with_env` (stdlib, host `a9a5ac22`). `ModuleLoader`
  carries no `ModuleEnv` (deps are `.handl.toml` paths, cache holds
  evaluated binding records) — nothing to key.
- `loader_parity`, `named_import`, and the `handl-runtime` module e2e
  tests pin the existing single-env behavior; `instantiate` adds tests
  for the multi-env case.
