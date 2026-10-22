# ADR-0155: Scope-bound arrow environments — deps as lexical arrow records, no registry dispatch

- Status: Proposed
- Date: 2026-10-22
- Repos: `kernel` (`handl-value`, `handl-language`), `engine-exec`,
  `engine-exec-tree-walk`, `engine-exec-bytecode`, `engine-exec-cranelift`,
  `host` (`platform`, `plugin-module`, `plugin-lib-handl`, `plugin-lib-stdlib`),
  `kit-core`, `lib-loader`, `lib-str-handl`

## Context

ADR-0156 made a module's dependency environment a first-class `ModuleEnv`
parameter, but kept the *dispatch* regime untouched: `mod.fn` still resolves
as a string callee through the engine's `ModuleLookup` chain. This ADR
designs the end state ADR-0156 §Decision explicitly deferred: **the env
becomes a record of callables bound in lexical scope, and `mod.f(x)` becomes
ordinary `record.field(args)`** — completing ADR-0049's "modules are evaluated
environments" reduction for the *dependency* side. Design doc only; no
implementation is authorized by this ADR.

### Current state (verified)

**Dotted calls `mod.f(x)`.** `mod` is (usually) an unbound identifier; the
`proj_env` native op falls back to the name as a string — "Unbound → name as
a string (allows identifiers like `eval` to flow into `App` host-function
dispatch)" (`engine-exec/src/native_ops.rs:635-639`; pinned by
`arr_proj_env_unbound_returns_name`,
`engine-exec-tree-walk/src/lib.rs:4620-4626`). All three engines carry the
same fallback: the shared op, bytecode `Instr::Load`
(`engine-exec-bytecode/src/vm.rs:674-680`), and cranelift's
`helper_v2_load_var`/`helper_v2_load_payload_var`
(`engine-exec-cranelift/src/lib.rs:1266-1300`). The `field-access` node
lowers to the `field_access` op
(`kit-prelude-lang/crates/handl-kit-prelude-lang/data/eval.yaml:440-455`),
whose `(Str, Str)` arm concatenates: `mod.f` → the string `"mod.f"`
(`engine-exec/src/native_ops.rs:2248-2251`; the same arm exists in the
`FpHostFunctions` foreign fn, `host/crates/handl-runtime/src/fp.rs:92-96`).
`canonical_fp_host_functions` wraps the module chain as every bridge
runtime's globals layer (`fp.rs:152`, `host/crates/plugin-lib-handl/src/lib.rs:1866`).

The string callee then dispatches through the engine's module chain: the
tree-walk's `App` arm (`engine-exec-tree-walk/src/lib.rs:1298-1314`) and
`flow` arm (`:2681-2683`) via `call_host_fn` (`:2741-2764`), bytecode's
`call_host` (`engine-exec-bytecode/src/vm.rs:1428-1450`) and `flow`
(`:1533-1572`), cranelift's `modules` field
(`engine-exec-cranelift/src/lib.rs:157,514`). The chain in a `HandlModule`
runtime is `LayeredLookup` — dependency `ModuleRegistry` first, injected
global lookup second (`host/crates/plugin-lib-handl/src/lib.rs:1883-1897`),
whose `ModuleRegistry::lookup` is a linear scan over modules plus a
split-on-dot lookup (`host/crates/plugin-module/src/lib.rs:115-132`).

**`import`/`from`.** The `"import"` native op
(`engine-exec/src/native_ops.rs:2464+`, ADR-0096) asks the engine's
`ImportResolver` for the module's bindings record and splices/projects its
fields into the importer's scope (no scope push — imported bindings share
the importer's scope, `native_ops.rs:2462-2463`). For *host/bridged* modules
the record is built by `module_bindings_object`: **function exports as string
callees `"mod.fn"`, value exports as values**
(`host/crates/plugin-lib-handl/src/lib.rs:176-190`; same in the stdlib
`TableImportResolver`, `host/crates/plugin-lib-stdlib/src/packages.rs:1276-1294`).
The doc comment justifying this — "a handl closure cannot cross that
`serde_json` boundary as a callable" (`lib.rs:149-153`) — is **stale for
arrows** since ADR-0106 landed: `TaggedValue::Arrow` is self-contained
serializable data
(`kernel/crates/handl-value/src/lib.rs:455-461`, `:1227+`), and a
JSON-round-tripped arrow applies without its frame
(`host/crates/handl-runtime/tests/arrow_serializability.rs`). Arrows *can*
cross; the string encoding persists because the `Module` trait surface is
name → `&dyn Function` (`host/crates/platform/src/eval.rs:391-429`) — a
bridge limitation, not a value-layer one. Native Rust functions genuinely
cannot cross.

**Two regimes already coexist.** File→file imports through the
`ModuleLoader` already hand over real values: the cache holds the evaluated
export record as a shared `Rc<TaggedValue>`
(`host/crates/handl-runtime/src/lib.rs:1452-1475`, `:1669-1671`,
`:2011-2013`), so `from "./x.hnl" import f` binds an actual arrow and `f(x)`
is ordinary application — no registry. Host-module imports
(`from "str" import …`) are stripped as decorative lines in loaded files
(`lib.rs:1720-1727`, `:1825-1833`): the body's `str` stays unbound and rides
the string→registry path. Bridged `str-handl` modules use
`from "char" import * as char` (`lib-str-handl/src/trim.hnl:3`), so `char`
is *bound* — to a record whose fields are the strings `"char.at"`, …
(`module_bindings_object`) — and the call still lands in the registry. The
`loader.hnl` bootstrap has no import lines at all; its `str.*`/`char.*` calls
are pure unbound-identifier→registry (`lib-loader/src/index.hnl:87-123`).

**Callable value repr.** There is none for native functions.
`TaggedValue`'s variants are
`Null/Bool/Int/Float/Sym/Str/Bytes/Arr/Obj/Pair/EitherL/EitherR/Arrow/Resource/Tensor`
(`kernel/crates/handl-value/src/lib.rs:429-532`); no `__native__`-style tag
exists anywhere in the tree. `ValueCallable`
(`host/crates/platform/src/eval.rs:148-152`) is a *callback trait* — it lets
host functions (e.g. `fn.map`) call **arrow** values via the engine
(`TreeWalkCallable`, `engine-exec-tree-walk/src/lib.rs:3925-3933`) — not a
value representation. `Resource` is an opaque C pointer + finalizer
(`handl-value/src/lib.rs:507-514`); `check_portable` rejects `__envref__`/
`__resource__`/`__compiled__` references at process boundaries
(`:1339-1351`). The `COMPILED_REGISTRY` handle pattern
(`host/crates/handl-runtime/src/lib.rs:560-597`, ADR-0047 §3) is the
precedent for "serializable handle, non-serializable artifact".

**ADR-0106 substrate.** Arrows are self-contained:
`{params, rest, array_pattern, body_def, binds, handlers}`; at mint time
free variables in the body are substituted with definition-site values, and
`binds` is the residual map for names that *cannot* be substituted —
`eval_rec`-region binders and "values that do not serialize (e.g.
`Resource`)" (`handl-value/src/lib.rs:462-474`). `HandlFunction::call`
already binds call-frame names directly via
`Runtime::eval_shared_with_call_args` (`__mod`, `__arg_N`;
`host/crates/plugin-lib-handl/src/lib.rs:2040-2054`;
`engine-exec/src/lib.rs:125-157`; the tree-walk override merges args into
the `ScopeStack` globals map, excluded from `capture()`,
`engine-exec-tree-walk/src/lib.rs:3330-3346`). Cranelift does not override
`eval_shared_with_call_args` (it inherits the per-`with_global` clone
default) and keeps a third `ScopeStack` mirror
(`engine-exec-cranelift/src/lib.rs:649`; bytecode's mirror is
`engine-exec-bytecode/src/vm.rs:136`), delegating arrow *application* to the
tree-walk reference path (`:1721`).

**ADR-0156 landed state.** `ModuleEnv` with `env_id`/`fork`
(`plugin-lib-handl/src/lib.rs:2091`, `:2197`, `:2207`), declarative
`from_manifest`/`from_pkg_hnl` over `pkg.hnl` `deps` (`:2145`, `:2184`),
`HandlModule::instantiate` (`:2617`), env-keyed sharing above
(`module_ref_with_env`,
`host/crates/plugin-lib-stdlib/src/packages.rs:1467`;
`KitResolver::resolve_with_env`, `kit-core/crates/kit-core/src/lib.rs:251`).
Per-thread caches (`RUNTIME_CACHE`/`MODULE_VALUE_CACHE`/`AST_CACHE`/
`VALUE_AST_CACHE`) key on `module_id` (`plugin-lib-handl/src/lib.rs:1723-1776`).

## Decision

A `.hnl` module's environment is a **record of callable values bound in
lexical scope** at instantiation. `str.trim(s)` is `field_access(str, "trim")`
on a scope binding whose field is a callable value, applied by the engine's
ordinary application path. No `ModuleLookup` dispatch for dependencies; no
string callees as the runtime encoding. Envs are first-class language values,
per the pure-arrow-calculus philosophy: explicit, serializable where the
payload is data, no hidden registries.

### 1. Callable values — what a dep binding *is*

Two payload kinds, one calling convention:

- **Arrows** (`.hnl`-authored deps): already values (ADR-0106). A dep
  module's export record is handed over as the record of arrows it is; the
  bridge stops flattening it into `Module::functions()` + string callees.
- **Native functions** (Rust `Function` impls — `char`, bridged
  `HandlFunction`s, FFI-backed floors): a new value shape,

  ```rust
  TaggedValue::NativeFn(Rc<NativeFnHandle>)
  // struct NativeFnHandle { name: Rc<str>,
  //                         call: Rc<dyn Fn(&[TaggedValue]) -> Result<TaggedValue, String>> }
  ```

  defined in `handl-value` (the kernel owns value shapes; the callable
  signature is ctx-less — `HandlFunction::call` already ignores its
  `EvalContext`, `plugin-lib-handl/src/lib.rs:1951`). `wrap_native_fn(name,
  f)` adapters live next to `Module` in `platform`. Equality and hashing are
  identity (name + pointer), like `Sym` (`handl-value/src/lib.rs:434-438`);
  `type_tag` is `"type.NativeFn"`. `check_portable` rejects it — same class
  as `__resource__`. The wire form is the **qualified name**:
  `{"__nativefn__": "char.at"}`, added to `is_reserved_value_key`
  (`kernel/crates/handl-language/src/lib.rs:325`) so user literals cannot
  forge it. Decoding re-hydrates by resolving the name against the current
  frame's module chain — in-process round-trips survive; cross-process fails
  loudly (checked by `check_portable`). Note the inversion: **the string
  callee becomes the serialization of a native-fn value, not the runtime
  dispatch encoding.**

- **Capture home.** A native-fn value referenced by an exported arrow rides
  the arrow's `binds` — the documented home for non-substitutable,
  non-serializable captures (`handl-value/src/lib.rs:469-474`), with
  `__bindsref__` interning keeping shared env records one map per env
  (`:496-501`). Arrows stay self-contained: `binds` is explicit data.
- **`ValueCallable` is unchanged** — it already covers the host-calls-arrow
  direction; this ADR adds the arrow/host-calls-native direction at the
  *value* layer.
- **Chosen over a registry handle** (`{"__nativefn__": id}` + thread-local
  registry, the `COMPILED_REGISTRY` pattern): the function is carried inline;
  no global mutable registry to leak, go stale, or reintroduce the
  `__envref__`-style state ADR-0106 Phase 5 removed. Registries remain for
  what is inherently process-bound (compiled artifacts).

### 2. Name resolution — what actually changes per engine

The framing correction: **identifier resolution does not change at all.**
`proj_env`'s scope-first lookup with string fallback stays byte-for-byte —
it is load-bearing for every dotted path and for host-module names in
loaded files. What changes is the *payload* of env bindings (callables
instead of strings) and what application does with a callable value:

1. `App`/`flow`/`call_host` arms gain a `NativeFn` callee arm, ordered
   **callable-value first, string-callee fallback**: tree-walk
   (`lib.rs:1302`, `:2681`, `:2741`), bytecode (`vm.rs:1428`, `:1533`),
   cranelift (its own compiled call sites — the tree-walk delegation at
   `lib.rs:1721` covers only *arrow* application). The string arm remains
   the fallback and the deserialization channel (§1), so staged rollout is
   additive: a record of callables simply never reaches the string arm.
2. `field_access` is unchanged: an `Obj` target already returns the field
   as-is; when the field is a `NativeFn`/`Arrow`, application is ordinary.
   The `(Str, Str)` concat arm (`native_ops.rs:2248-2251`, `fp.rs:92-96`)
   stays — it is the unbound-path fallback.
3. **What breaks if the fallback is later removed**: nothing in the
   engines' resolution, but the *ambient* std surface — today any unbound
   dotted callee silently resolves through `TableModuleLookup` ("stdlib
   packages reach every table module without pre-listing `deps`",
   `host/crates/plugin-lib-stdlib/src/packages.rs:1300-1306"). Under
   scope-bound envs the ambient becomes an explicit `std` binding in the
   default env (shadowed by declared deps and imports), not an implicit
   registry fallback. `.hnl` sources that never declared `fn`/`str` keep
   working because the default env binds them; the honesty gain is that
   *undeclared* names fail rather than resolve. One string-callee use is
   deliberately kept as data, not dispatch: the `Arrow{first, second}`
   composite record, whose bare-string `second` is a host-fn name
   (`engine-exec-tree-walk/src/lib.rs:2824-2832`) — composition carrying a
   native fn as data; it gains a `NativeFn`-valued form with the string as
   its wire encoding.
4. **Data-driven compliance**: the eval schemes in `eval.yaml` are
   unchanged (`field-access`/`call` already lower to ops); the addition is
   a value shape + op arms — calculus vocabulary, which is allowed to be
   fixed in Rust. No `kind ==` shape checks are introduced.

### 3. Interop — `ModuleEnv` builds the lexical env record

`ModuleEnv` (ADR-0156) is unchanged in shape and becomes the **builder** of
the env record:

- `instantiate(env)`'s `build_runtime` (`plugin-lib-handl/src/lib.rs:1799`)
  materializes, alongside (then instead of) the `ModuleRegistry` +
  `LayeredLookup`, a `TaggedValue::Obj` mapping each dep name to its
  bindings record — arrows for `.hnl`-authored deps, `NativeFn` handles for
  native/bridged ones — and hands it to the runtime as **call-frame
  bindings**, the same channel `HandlFunction::call` already uses for
  `__mod`/`__arg_N` (`:2040-2054`; `eval_shared_with_call_args`). Loader
  modules with no import lines (`loader.hnl`) get their floors this way;
  modules with `import`/`from` get them through the unchanged import op,
  which already splices record fields into scope.
- `Module` gains an additive **`bindings_tagged(&self) ->
  Option<Rc<TaggedValue>>`** surface (default `None`): the evaluated export
  record for `.hnl`-backed modules (today reachable only through
  `Module::value` projections and `HandlFunction` re-entry). Native modules
  default to a synthesized record of `wrap_native_fn` handles from
  `functions()`. `module_bindings_object` migrates from string callees to
  this payload; `DependencyImportResolver`
  (`plugin-lib-handl/src/lib.rs:159-209`) and `TableImportResolver`
  (`packages.rs:1247-1294`) emit records of callables when available.
- Migration is per-resolver and additive because §2's dispatch is
  value-first-with-fallback: while any consumer still expects strings, both
  encodings apply. `LayeredLookup` and the `ModuleLookup` trait are **not
  removed** — they remain the dispatch for effect ops/FFI
  (`ffi.call`, `zlib.*`; the ADR-0106 direction of resource access as
  effect operations), the typechecker's `lookup_module_exports` /
  `lookup_type_signature` channel (`platform/src/eval.rs:444-466`), and the
  `NativeFn` re-hydration path. They stop being the *dependency* path.

### 4. Bootstrap — the loader chain

The bootstrap paradox (the loader needs `str`/`char` before it can prepare
`str`) is unchanged in shape: `HandlLoader::new` builds the loader raw over
`char` + `str-handl` dep modules and runs it on bytecode
(`plugin-lib-handl/src/lib.rs:2793-2810`). What changes is *wiring*: the
loader's runtime binds `char`/`str` records on its call frame, so
`str.find(...)` in `loader.hnl` resolves scope-first with no registry
dependency — the registry is no longer load-bearing for the bootstrap, only
for FFI. This is strictly **simpler**: a loader runtime can be built from
(lang, env-record) alone. Env-record construction is cheap and lazy in the
right way — a `NativeFn` handle is a closure over `(module_id, builder)`
(`HandlFunction::new`, `plugin-lib-handl/src/lib.rs:1930`), so building a
dep's record forces no evaluation; each call still re-enters the dep's own
cached per-thread runtime exactly as today.

**Cycles/lazy envs**: an env record is built before the module body
evaluates, from already-resolved `Arc<dyn Module>` deps — a module's env
cannot contain itself, and the file loader's cycle detector
(`handl-runtime/src/lib.rs:1682-1696`) is unchanged. The remaining laziness
gap (an expensive dep floor built even if never referenced) is expressible
later *in the language* — a thunk field is a nullary arrow, and arrows are
now values — but that changes call sites and is explicitly out of scope.

### 5. Caching and performance

- **Keys stay identity-based.** The evaluated module record becomes
  env-dependent (minted arrows' `binds` capture the env record), so
  `MODULE_VALUE_CACHE` keyed by `module_id` must key `(module_id, env_id)` —
  which `instantiate` already provides by re-keying
  (`plugin-lib-handl/src/lib.rs:2561-2567`, ADR-0156). Do **not** build a
  structural env fingerprint: hashing is defined for `TaggedValue`
  (`handl-value/src/lib.rs:553-614`) but `NativeFn` hashes identity, arrow
  hashing walks `body_def` arenas, and `ModuleEnv::clone` already shares one
  `env_id` for identical envs — identity keys dedup everything a structural
  hash would, at zero cost.
- **Record path vs registry path is neutral-to-faster.** Today's dotted call
  pays a per-callsite `format!` concat (`native_ops.rs:2248`, `fp.rs:96`)
  plus a linear scan over dep modules with a per-module lookup
  (`plugin-module/src/lib.rs:115-132`). The record path pays one scope/globals
  lookup plus one `FxHashMap` get on an `Rc`-shared record. The measured
  risk is not lookup cost but **capture growth**: env records ride arrows'
  `binds`, so minted arrows grow by one shared map reference — amortized by
  `__bindsref__` interning, but hot mint loops (`rec` regions) must be
  benchmarked (`examples/loader_bench.rs` is the harness).

### 6. Staged plan

| Phase | Change | Test gate |
|---|---|---|
| 0 | `TaggedValue::NativeFn` + wire key + `check_portable` rejection (kernel only, unused) | `kernel` `handl-value` suites; `cargo test -p handl-value` |
| 1 | `NativeFn` apply arms: tree-walk → bytecode → cranelift (string arm untouched) | engine unit tests; extended `import_all_engines` (`host/crates/handl-runtime/tests/import_all_engines.rs`) |
| 2 | `Module::bindings_tagged`; `DependencyImportResolver`/`TableImportResolver` emit callable records; import op unchanged | `named_import` (`host/crates/plugin-lib-stdlib/tests/named_import.rs`), `import_all_engines`, `loader_parity` still green on the string path, new `scope_bound_env` test asserting both encodings |
| 3 | Call-frame env binding in `build_runtime`/`HandlFunction::call`; loader + `str-handl` floors resolve scope-first; registry demoted to fallback/ops | `loader_parity` (`host/crates/plugin-lib-handl/tests/loader.rs:1101`, ~68 s), `dependency_import`, module e2e, bytecode-loader parity |
| 4 | Default env binds the std table explicitly; undeclared names fail; string callees remain only as ops/FFI dispatch + `NativeFn` serialization | full workspace sweep (`cargo nextest run --workspace -j 2`) |

Smallest safe first step is **Phase 0 + 1** (value repr + apply arm): purely
additive, no consumer changes, and it unblocks everything else. The
scope-first behavior is not flag-gated in Rust — the presence of callable
values in scope *is* the flag (§2.1); a host can pin the old encoding by
continuing to hand out string-callee records.

### 7. Rejected alternatives

- **Registry handles for native fns** (`{"__nativefn__": id}` +
  thread-local registry, ADR-0047 §3 pattern): rejected as the primary
  mechanism — it reintroduces exactly the hidden-registry state ADR-0106
  Phase 5 removed (`__envref__`), on the every-import path. Accepted for
  compiled artifacts, which are inherently process-bound.
- **Scope-parameterize `ModuleLookup`** (env as a lookup layer, dispatch
  still string-keyed): keeps deps opaque and callees strings; multi-env
  stays a host-side trick; ADR-0156's research lanes still can't stub a dep
  *value*. Rejected for not actually dissolving the registry.
- **Structural env fingerprints for caching**: identity (`env_id`) is
  cheaper and dedups the same cases; arrow hashing is expensive and
  `NativeFn` identity forces mixed semantics (§5).
- **Do nothing (ADR-0156 status quo)**: honest costs of *this* ADR are real
  — engine blast radius (three `ScopeStack` mirrors, `App`/`flow`/
  `call_host` arms in all three engines, `native_ops.rs` arms), a kernel
  value-shape addition, portability policing
  (`check_portable`), and a `loader_parity`-class test matrix per phase.
  But the status quo leaves deps as the one non-value, non-serializable,
  registry-dispatched construct in an otherwise explicit-calculus language,
  and keeps ADR-0049's "environments as values" unimplemented on the dep
  side. The costs are bounded and staged; the inelegance is permanent.
- **`Module` as a first-class value type** (ADR-0048): already superseded by
  ADR-0049; this ADR completes 0049 §1 for dependencies rather than
  reviving a parallel module value system.

## Consequences

- **Dependencies become ordinary language values**: serializable records of
  arrows and (process-locally) native handles; `str.trim(s)` is
  `record.field + application` in every engine, with no dotted-call special
  case. Envs compose like any record (merge, shadow, project) and multiple
  envs per file fall out — the env is a function argument (ADR-0156's
  framing, now at the language layer).
- **The serde boundary moves, not grows**: `module_bindings_object`'s string
  callees retire as the runtime encoding; the `Value` boundary at
  `MODULE_VALUE_CACHE`/`Function::call` must gain the tagged fast path
  (binds with `NativeFn` must not be forced through `serde_json`) — the
  `resolve_import_tagged`/`load_module_tagged` precedent
  (`handl-runtime/src/lib.rs:1669`, `:2011`) is the model.
- **Bootstrap simplifies**: loader runtimes need (lang, env record), not a
  `ModuleRegistry`; the registry survives for effect ops/FFI and type
  signatures.
- **Caching stays identity-keyed** (`(module_id, env_id)`); no structural
  fingerprint machinery.
- **Migration risk is concentrated in Phase 4** (ambient std → explicit
  default-env binding); Phases 0-2 are additive with the fallback intact.
- The typechecker's module-signature channel (`lookup_module_exports`)
  continues through `ModuleLookup` until env records grow declared
  signatures — follow-up, not this ADR.

## References

- ADR-0049 (modules are evaluated environments — the reduction this
  completes for deps), ADR-0048 (superseded)
- ADR-0106 (self-contained serializable arrows; `binds` for
  non-serializable captures; Phase 5 env-registry removal)
- ADR-0156 (`ModuleEnv` as parameter, `instantiate`, env identity)
- ADR-0096 (unified import model — the `import` op this builds on),
  ADR-0060 (generic call dispatch), ADR-0047 §3 (`COMPILED_REGISTRY`
  handle pattern — the rejected variant)
- ADR-0120 Phase 5 / ADR-0153 (arrow values as cell data — the payload
  regime env records extend)
