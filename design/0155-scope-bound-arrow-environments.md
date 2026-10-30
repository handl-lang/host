---
title: Scope-bound arrow environments — deps as lexical arrow records, no registry dispatch — design detail
type: design
date: 2026-10-22
updated: 2026-10-29
refs: [ADR-0155]
---

# ADR-0155: Scope-bound arrow environments — deps as lexical arrow records, no registry dispatch — design detail

Moved verbatim from ADR-0155 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

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
`arr_proj_env_unbound_returns_name`, `engine-exec-tree-walk/src/lib.rs:4705`).
All three engines carry the same fallback: the shared op, bytecode
`Instr::Load` (`engine-exec-bytecode/src/vm.rs:678-685`), and cranelift's
`helper_v2_load_var`/`helper_v2_load_payload_var`
(`engine-exec-cranelift/src/lib.rs:1270-1304`). `field-access` lowers to
`field_access` (`kit-prelude-lang/.../data/eval.yaml:441-455`), whose
`(Str, Str)` arm concatenates `mod.f` → `"mod.f"` (`native_ops.rs:2248-2250`;
same arm in `FpHostFunctions`, `host/crates/handl-runtime/src/fp.rs:92-96`,
wired as every bridge runtime's globals layer by
`canonical_fp_host_functions`, `fp.rs:152`, `plugin-lib-handl/src/lib.rs:1869`).

**The `call` scheme already splits on callee type** (`eval.yaml:334-368`):
`route_on_type` sends `string` callees to `call_host`, everything else to
`!App` — value callees already apply today. `call_host` is a `(name,
packed-args-array)` leaf: `incoming` is the callee string, `args[0]` the arg
list (tree-walk `lib.rs:2463-2547`, incl. the `lang.op_decl` grade path
`:2476-2510`; bytecode `apply_native_v2`, `vm.rs:1432-1472`). `flow` keeps a
string-callee arm in both (tree-walk `:2762-2764`, bytecode `:1564-1576`);
the tree-walk `App` arm dispatches string callees via `call_host_fn`
(`:1302-1314`, `:2822-2845`) — but bytecode's `apply_app` has **no**
Str-callee arm (`vm.rs:1698-1717`; unreachable today since `route_on_type`
never sends a string to `!App`). The chain in a `HandlModule` runtime is
`LayeredLookup` — dep `ModuleRegistry` first, injected global lookup second
(`plugin-lib-handl/src/lib.rs:1886-1900`), a linear scan plus split-on-dot
(`plugin-module/src/lib.rs:114-132`).

**`import`/`from`.** The `"import"` native op (`native_ops.rs:2464+`,
ADR-0096) asks the engine's `ImportResolver` for the module's bindings
record and splices/projects its fields into the importer's scope (no scope
push, `:2462-2463`; already on the tagged fast path `resolve_import_tagged`,
`:2505-2507`). For *host/bridged* modules the record is built by
`module_bindings_object`: **function exports as string callees `"mod.fn"`,
value exports as values** (`plugin-lib-handl/src/lib.rs:176-190`; same in
`TableImportResolver`, `plugin-lib-stdlib/src/packages.rs:1273-1293`). The
doc comment justifying this — "a handl closure cannot cross that
`serde_json` boundary as a callable" (`lib.rs:149-153`) — is **stale for
arrows** since ADR-0106: `TaggedValue::Arrow` is self-contained serializable
data (`kernel/crates/handl-value/src/lib.rs:455-506`, `:1227-1293`) and a
JSON-round-tripped arrow applies without its frame
(`handl-runtime/tests/arrow_serializability.rs`). The encoding persists
because the `Module` trait surface is name → `&dyn Function`
(`platform/src/eval.rs:391-429`) — a bridge limitation, not a value-layer
one. Native Rust functions cannot cross — but §1 shows they never needed
to: their *name* is the payload.

**Two regimes already coexist.** File→file imports through `ModuleLoader`
hand over real values — the cache holds the evaluated export record as a
shared `Rc<TaggedValue>` (`load_module_tagged`,
`handl-runtime/src/lib.rs:1696-1710`; `collect_module_bindings_tagged`,
`:1856`) — so `from "./x.hnl" import f` binds an actual arrow. Host-module
imports in loaded files are *rewritten*: `strip_module_decl_lines`
(`:1887-1928`) lowers non-file `from "<host>" import name` declarations to
string bindings (`f := "spec.f"`, `* as m` → `m := "spec"`;
`module_prep.rs:749-791`). Bridged `str-handl` modules instead run the
runtime `import` op — `from "char" import * as char`
(`lib-str-handl/src/trim.hnl:3`) binds `char` to a record of `"char.at"`
strings, and the call still lands in the registry. `loader.hnl` has no
import lines; its `str.*`/`char.*` calls are pure
unbound-identifier→registry (`lib-loader/src/index.hnl:87-123`).

**Callable value repr.** There is none for native functions — no
`__native__`-style tag exists anywhere in `TaggedValue`'s variants
(`handl-value/src/lib.rs:429-532`). `ValueCallable`
(`platform/src/eval.rs:148-152`) is a *callback trait* — host functions call
**arrow** values via the engine (`TreeWalkCallable`, tree-walk
`lib.rs:4004-4017`, which rejects non-`Arrow` callees) — not a value repr.
`check_portable` rejects `__envref__`, dangling `__bindsref__`,
`__resource__`, `__compiled__` (`:1323-1347`) — and says nothing about
`__arrow__`, which passes as-is. `COMPILED_REGISTRY`
(`handl-runtime/src/lib.rs:560-597`, ADR-0047 §3) is the precedent for
"serializable handle, non-serializable artifact".

**ADR-0106 substrate.** Arrows are `{params, rest, array_pattern, body_def,
binds, handlers}`; at mint time free vars substitute with definition-site
values (a `var` substitutes only when present in the mint env, else copies
through free — `handl-language/src/instantiate.rs:79-123`); `binds` is the
residual map for non-substitutable names (`handl-value/src/lib.rs:462-474`);
`body_def` is `{"__ast__": <Ast>}` via `ast_value`
(`handl-language/src/lib.rs:204-238`), memoized per `Rc` at application
(`cached_def_arena`, tree-walk `:304-340`; bytecode `def_arenas`/`def_cache`,
`vm.rs:400-418`). `HandlFunction::call` binds call-frame names via
`eval_shared_with_call_args` (`__mod`/`__arg_N`;
`plugin-lib-handl/src/lib.rs:2038-2057`; `engine-exec/src/lib.rs:146-157`;
tree-walk merges them into `ScopeStack.globals`, `:3420+`). Cranelift
inherits the `with_global`-clone default and delegates arrow *application*
to the tree-walk path with the caller's `modules`
(`engine-exec-cranelift/src/lib.rs:1718-1740`; `ScopeStack` mirrors at
`:653`, `vm.rs:131-151`).

**Mint-by-hand precedent.** `app_closure_applies_body` (tree-walk
`lib.rs:4915-4951`) builds `{"__arrow__": true, params: ["x"], body_def:
ast_value(arena)}` in a test and applies it through `App`; `def_with_body`
(`:7384-7389`) mints a one-node `body_def` the same way. And
**`Arrow{first, second}` already carries a native fn as a name**: applying
a `>>>` composite record runs `first` then `second`; a bare-string `second`
is a host-function name dispatched through `call_host_fn` (tree-walk
`:2901-2916`; bytecode `vm.rs:1647-1676`). Composition already treats
"native function by qualified name" as value-level data — the wrapper-arrow
encoding is that precedent, generalized.

**ADR-0156 landed state.** `ModuleEnv` with `env_id`/`fork`
(`plugin-lib-handl/src/lib.rs:2094-2115`, `:2200`, `:2210`),
`from_manifest`/`from_pkg_hnl` over `pkg.hnl` `deps` (`:2148`, `:2187`),
`instantiate` (`:2620`), env-keyed sharing above (`module_ref_with_env`,
`plugin-lib-stdlib/src/packages.rs:1467`; `KitResolver::resolve_with_env`,
`kit-core/crates/kit-core/src/lib.rs:251`). Per-thread caches
(`RUNTIME_CACHE`/`MODULE_VALUE_CACHE`/`AST_CACHE`) key on `module_id`
(`plugin-lib-handl/src/lib.rs:1981-2046`).

## Decision

A `.hnl` module's environment is a **record of arrow values bound in
lexical scope** at instantiation. `str.trim(s)` is `field_access(str,
"trim")` on a scope binding whose field is an arrow, applied by the
engine's ordinary application path — the `route_on_type` → `!App` branch
`eval.call` already has. No `ModuleLookup` dispatch for dependencies; no
string callees as the runtime encoding. Envs are first-class language
values, per the pure-arrow-calculus philosophy: explicit, serializable,
no hidden registries — and **the arrow is the only function former**.

### 1. Callable values — what a dep binding *is*

One payload kind — **arrows**:

- **`.hnl`-authored deps** (dep `HandlModule`, file imports): the export
  record *already is* a record of arrows — `ModuleLoader` hands it over
  as a shared `Rc<TaggedValue>` (`handl-runtime/src/lib.rs:1696-1710`).
- **Native dep functions** (Rust `Function` impls — `char`, FFI floors):
  a **wrapper arrow** synthesized per `(module, function)` name, the
  qualified name embedded *inside* the body as ordinary AST data:

  ```text
  TaggedValue::Arrow { params: [], rest: "args",
                       body_def: <callv node: [lit "mod.fn", var "args"]>,
                       binds: {}, handlers: [] }
  ```

  `wrap_dep_fn(mod_name, fn_name)` builds the `body_def` arena directly
  (`Ast::push` two leaves + the head node, wrap via `ast_value`; the
  `def_with_body` precedent). No `Ast::instantiate`, no scope capture —
  construction is pure data and forces no evaluation.

  **The splat subtlety — why not `x ~> mod.fn(x)`.** `Function::call` is
  `call(&[Value])` with **no arity metadata** (`platform/src/eval.rs:311-316`),
  so `x*` cannot be a fixed param list in general; and a `call`-node body
  `mod.fn(args)` does *not* splat — `proj_rest` re-packs the single `args`
  child into `[Arr[…]]`, so the fn would receive one array argument (the
  `args[0]`-as-list protocol cited above). The working body shape is a node
  whose scheme hands `call_host` the *evaluated* args array directly — a
  new **node kind** (data, per ADR-0044/0081), not an op:

  ```yaml
  # eval.yaml — leaf splat-call: child 0 evals to the callee name,
  # child 1 evals to the argument array.
  - id: eval.callv
    node_kind: callv
    scheme: !Seq
      fst: !Op  { name: proj_child, args: [], source: Native, lit: 0 }  # → "mod.fn"
      snd: !Op  { name: call_host, source: DynamicForeign,
                  args: [ !Op { name: proj_child, args: [], source: Native, lit: 1 } ] }  # → Arr[…]
  ```

  This composes `proj_child`, `Seq`, and the existing `call_host` leaf —
  whose `(name, packed-array)` protocol tree-walk (`lib.rs:2537-2541`) and
  bytecode (`vm.rs:1455-1466`) already implement; cranelift never sees it
  because arrow application delegates to the tree-walk path
  (`engine-exec-cranelift/src/lib.rs:1718-1740`), so wrapper bodies always
  evaluate under the interpreter. For unary deps the simpler
  `x ~> mod.fn(x)` suffices — the P0 smoke test uses it; a `match`-fan body
  (`[] => mod.fn(), [a] => mod.fn(a), …`) is the no-new-kind fallback with
  an arity ceiling.

- **What a wrapper *is* semantically**: a name indirection, not a closure
  over a function pointer — applying it re-resolves `"mod.fn"` through the
  *applying runtime's* module chain at `call_host`, the exact leaf today's
  unbound-identifier path lands on. In `instantiate` this is consistent by
  construction (record and chain derive from the same `ModuleEnv`); see
  §Consequences for the cross-runtime caveat.
- **Marshal story is trivial.** The name literal lives inside `body_def`
  (`{"__ast__": …}` data); the whole arrow is the `{"__arrow__": …}` wire
  form that already encodes/decodes (`handl-value/src/lib.rs:1227-1293`,
  `:774-817`) and already passes `check_portable` — **no new rule** needed
  (`:1331-1347`). A wrapper survives a serde round-trip; the name
  re-resolves through the applying runtime's module chain.
- **The stale comment, reframed**: `module_bindings_object`'s justification
  is not merely stale for arrows — the string was never needed as the
  *payload* at all: it was always a name handle for the leaf, and embedding
  it in an arrow body keeps it a name handle while making the field a real
  callable value. `ValueCallable` stays unchanged; `Arrow{first, second}`
  composites need no new form — the bare-string `second` *is* already
  "native fn as name data".

### 2. Name resolution — what actually changes per engine

The framing correction stands: **identifier resolution does not change at
all.** `proj_env`'s scope-first lookup with string fallback stays
byte-for-byte — load-bearing for every dotted path and for host-module
names in loaded files. What changes is only the *payload* of env bindings
(arrows instead of strings):

1. **Zero new apply arms.** A record field holding `TaggedValue::Arrow`
   routes `route_on_type` → `!App` → `apply_app`'s Arrow arm (tree-walk
   `:2938-2960`; bytecode `Instr::Call*` → `apply_app`, `vm.rs:780-882`;
   cranelift via the delegation). The wrapper's `callv` leaf is a scheme
   over existing ops — `call_host` dispatch is *kept anyway* as the leaf
   for ops/FFI (`ffi.call`, `zlib.*`, the `op_decl` grade path,
   `FpHostFunctions`' `fn.fanout_fn`/`field_access`). The string callee
   remains the fallback and the `host_import_lines` lowering's encoding;
   an arrow record never reaches the string arm, so rollout is additive.
2. `field_access` is unchanged: an `Obj` target returns the field as-is
   (`native_ops.rs:2181-2184`); an `Arrow` field applies ordinarily. The
   `(Str, Str)` concat arm stays — the unbound-path fallback, and what a
   `mod.fn` inside a fixed-arity wrapper body still uses.
3. **Bytecode caveat**: bytecode `apply_app` has no Str-callee arm
   (`vm.rs:1698-1717`) — fine under `callv` (the leaf is `call_host`), but
   it rules out a `pipe`-shaped body (`args |> "mod.fn"`): dispatchable in
   the tree-walk (`:1302-1303`), an error in the VM. A Str arm in bytecode
   `apply_app` mirroring `flow`'s is a one-arm fix if ever wanted —
   flagged, not required.
4. **What breaks if the fallback is later removed**: nothing in the
   engines' resolution, but the *ambient* std surface — today any unbound
   dotted callee silently resolves through `TableModuleLookup` ("stdlib
   packages reach every table module without pre-listing `deps`",
   `plugin-lib-stdlib/src/packages.rs:1295-1304`). Under scope-bound envs
   the ambient becomes an explicit `std` binding in the default env
   (shadowed by declared deps and imports): `.hnl` sources that never
   declared `fn`/`str` keep working because the default env binds them;
   *undeclared* names fail rather than resolve.
5. **Data-driven compliance**: unchanged eval schemes plus the `callv`
   *addition* (§1) — `AstShapeDef` + scheme, both YAML data. No `kind ==`
   shape checks, no Rust combinator.

### 3. Interop — `ModuleEnv` builds the lexical env record

`ModuleEnv` (ADR-0156) is unchanged in shape and becomes the **builder** of
the env record:

- `instantiate(env)`'s `build_runtime` (`plugin-lib-handl/src/lib.rs:1802`)
  materializes, alongside (then instead of) the `ModuleRegistry` +
  `LayeredLookup`, a `TaggedValue::Obj` mapping each dep name to its
  bindings record — the export record for `.hnl` deps, `wrap_dep_fn`
  wrappers for native/bridged ones — and hands it to the runtime as
  **call-frame bindings**, the channel `HandlFunction::call` already uses
  for `__mod`/`__arg_N` (`:2050-2057`). Modules with no import lines
  (`loader.hnl`) get their floors this way; `import`/`from` modules get
  them through the unchanged import op.
- `Module` gains an additive **`bindings_tagged(&self) ->
  Option<Rc<TaggedValue>>`** surface (default `None`): the evaluated
  export record for `.hnl`-backed modules (today reachable only via
  `Module::value` projections + `HandlFunction` re-entry). Native
  modules' default is a synthesized record of wrapper arrows over
  `functions()` names (`platform/src/eval.rs:404-406`).
  `module_bindings_object` migrates to this payload;
  `DependencyImportResolver` (`plugin-lib-handl/src/lib.rs:159-209`) and
  `TableImportResolver` (`packages.rs:1245-1293`) emit records of arrows.
- Migration is per-resolver and additive: a `Str` field still routes to
  `call_host`, an `Arrow` field to `!App` — both encodings apply while any
  consumer still binds strings. `LayeredLookup`/`ModuleLookup` are **not
  removed** — they remain the leaf dispatch for effect ops/FFI, the
  typechecker's `lookup_module_exports`/`lookup_type_signature` channel
  (`platform/src/eval.rs:449-466`), and the leaf wrapper bodies re-resolve
  through. Dep modules stay registered in the chain even when scope-bound:
  the record is the *scope* path, the chain is the *leaf*.

### 4. Bootstrap — the loader chain

The bootstrap paradox (the loader needs `str`/`char` before it can prepare
`str`) is unchanged in shape: `HandlLoader::new` builds the loader raw over
`char` + `str-handl` dep modules and runs it on bytecode
(`plugin-lib-handl/src/lib.rs:2808-2815`). What changes is *wiring*: the
loader's runtime binds `char`/`str` records on its call frame, so
`str.find(...)` in `loader.hnl` resolves scope-first with no registry
dependency — the registry is no longer load-bearing for the bootstrap, only
for the leaf. Strictly **simpler**: a loader runtime needs (lang,
env-record, leaf-chain), and wrapper construction forces **no evaluation** —
an arrow is data; there is not even the `HandlFunction::new`
closure-over-`builder` indirection (`:1933-1947`). Each call still re-enters
the dep's cached per-thread runtime exactly as today — the leaf is still
`modules.lookup` → `Function::call`.

**Cycles/lazy envs**: an env record is built before the module body
evaluates, from already-resolved `Arc<dyn Module>` deps — a module's env
cannot contain itself, and the file loader's cycle detector
(`handl-runtime/src/lib.rs:1721-1729`) is unchanged. The laziness gap (an
expensive dep floor built even if never referenced) is expressible later *in
the language* — a thunk field is a nullary arrow — but changes call sites;
out of scope.

### 5. Caching and performance

- **Keys stay identity-based.** The evaluated module record becomes
  env-dependent (minted arrows' `binds` capture the env record), so
  `MODULE_VALUE_CACHE` keyed by `module_id` must key `(module_id, env_id)` —
  which `instantiate` already provides by re-keying
  (`plugin-lib-handl/src/lib.rs:2547-2571`, ADR-0156). No structural env
  fingerprint: `TaggedValue` hashing walks `body_def` arenas structurally
  (`handl-value/src/lib.rs:552+`), and `ModuleEnv::clone` already shares one
  `env_id` for identical envs.
- **The serde-boundary concern shrinks.** Arrows already serialize, and
  `resolve_import_tagged`/`load_module_tagged` already carry
  `Rc<TaggedValue>` records across `Function::call` without a serde
  round-trip (`native_ops.rs:2505-2507`,
  `handl-runtime/src/lib.rs:1708-1710`, `:2043-2050`). What remains
  non-portable at `MODULE_VALUE_CACHE` is only arrow `binds` holding
  `Resource` payloads (finalizer dropped on decode,
  `handl-value/src/lib.rs:818-830`) — a pre-existing ADR-0106 caveat, not
  new risk.
- **Record path vs registry path is neutral-to-faster.** Today's dotted
  call pays a per-callsite `format!` concat (`native_ops.rs:2249`,
  `fp.rs:96`) plus a linear scan over dep modules
  (`plugin-module/src/lib.rs:115-132`). The record path pays one scope
  lookup + one `FxHashMap` get, then **one extra hop**: the wrapper's apply
  runs `body_def` (`cached_def_arena`-memoized) and lands on the same
  `call_host` leaf — the indirection is the call, not the lookup. Measured
  risk is per-call arrow-apply overhead in hot paths: `loader.hnl`'s
  per-character `str.*`/`char.*` calls on bytecode are the workload to watch
  (`examples/loader_bench.rs` is the harness). Wrappers carry empty `binds`
  (no capture growth); module-exported arrows still carry the env record in
  `binds`, amortized by `__bindsref__` interning.

### 6. Staged plan

| Phase | Change | Test gate |
|---|---|---|
| 0 | `callv` node kind (`ast.yaml` shape + `eval.yaml` scheme — data only); `wrap_dep_fn`/`wrap_native_dep` mint helper; unit test proving a programmatically minted `x ~> char.at(x)` applies through the chain (precedent: `app_closure_applies_body`) | tree-walk + bytecode unit tests; `handl-runtime` build green |
| 1 | `Module::bindings_tagged`; `DependencyImportResolver`/`TableImportResolver` emit arrow records; `module_bindings_object` migrates; import op unchanged | `named_import` (`plugin-lib-stdlib/tests/named_import.rs`), `import_all_engines`, `module_env`/`env_manifest`/`multi_env` suites; `loader_parity` still green on the string path; new `scope_bound_env` test asserting both encodings |
| 2 | Call-frame env binding in `build_runtime`/`HandlFunction::call`; loader + `str-handl` floors resolve scope-first; registry demoted to leaf dispatch | `loader_parity` (`plugin-lib-handl/tests/loader.rs:1101`, ~68 s), `dependency_import`, module e2e, `import_bytecode` parity |
| 3 | Default env binds the std table explicitly; undeclared names fail; string callees remain only as ops/FFI leaf dispatch + `host_import_lines` lowering | full workspace sweep (`cargo nextest run --workspace -j 2`) |

Smallest safe first step is **Phase 0**: a data-only node kind plus a mint
helper — no consumer changes, and it unblocks everything else. Scope-first
is not flag-gated in Rust — the presence of arrow values in scope *is* the
flag (§2.1); a host pins the old encoding by continuing to hand out
string-callee records. Notably absent vs the superseded plan: **no kernel
change at all** — `handl-value`/`handl-language` gain nothing.

### 7. Rejected alternatives

- **`TaggedValue::NativeFn(Rc<NativeFnHandle>)` — the superseded design.**
  A second callable kind at the value layer contradicts the pure arrow
  calculus: the arrow is the only function former (ADR-0106), and a
  `NativeFn` is a function that is not an arrow. It would have cost a new
  kernel value shape, `PartialEq`/`Hash`/`type_tag` arms
  (`handl-value/src/lib.rs:547-614`), a `__nativefn__` wire key +
  `is_reserved_value_key` entry (`handl-language/src/lib.rs:325`), a new
  `check_portable` rejection, and `NativeFn` callee arms in `App`/`flow`/
  `call_host` on all three engines — real machinery for **zero behavioral
  gain**: wrapper arrows deliver "dep binding is a callable value" through
  the existing `!App` path and the `call_host` leaf that stays anyway. The
  only thing `NativeFn` pins that a wrapper cannot is the resolved
  `&dyn Function` itself — and that pinning is neither needed for
  `instantiate` nor portable (a `NativeFn` cannot cross `serde_json`; a
  wrapper can).
- **Registry handles for native fns** (`{"__nativefn__": id}` +
  thread-local registry, ADR-0047 §3 pattern): doubly obsolete — the
  wrapper's embedded name *is* the portable handle, and a registry
  reintroduces the hidden-registry state ADR-0106 Phase 5 removed
  (`__envref__`) on the every-import path. Registries remain for what is
  inherently process-bound (compiled artifacts).
- **Scope-parameterize `ModuleLookup`** (env as a lookup layer, dispatch
  still string-keyed): keeps deps opaque and callees strings; multi-env
  stays a host-side trick; ADR-0156's research lanes still can't stub a
  dep *value*. Rejected for not actually dissolving the registry — though
  note the chain survives as the leaf dispatcher (§3); what this ADR
  removes is the *encoding*, not the trait.
- **Structural env fingerprints for caching**: identity (`env_id`) is
  cheaper and dedups the same cases; arrow hashing is expensive (§5).
- **Do nothing (ADR-0156 status quo)**: the honest costs of *this* ADR
  are now small — a new node kind (YAML data), a mint helper, resolver
  output changes, call-frame env wiring — but `loader_parity`-class
  coverage per phase still applies. The status quo leaves deps as the one
  non-value, non-serializable-as-callable, registry-dispatched construct
  in an otherwise explicit-calculus language, and keeps ADR-0049's
  "environments as values" unimplemented on the dep side.
- **`Module` as a first-class value type** (ADR-0048): replaced by
  ADR-0049; this ADR completes 0049 §1 for dependencies rather than
  reviving a parallel module value system.

## Consequences

- **Dependencies become ordinary language values**: records of arrows —
  self-contained for `.hnl` deps, name-indirecting for native ones.
  `str.trim(s)` is `record.field + application` in every engine; envs
  compose like any record (merge, shadow, project) and multiple envs per
  file fall out — the env is a function argument (ADR-0156's framing, now
  at the language layer).
- **Wrappers are ambient re-resolution, not pinning.** A wrapper carries
  `"mod.fn"` and resolves it against the *applying* runtime's chain —
  inside `instantiate` that chain is built from the same `ModuleEnv`, so
  record and leaf agree. A wrapper carried into a runtime whose chain
  lacks (or shadows) `mod` rebinds or fails *at call time* ("function
  'mod.fn' not found"), not at a boundary check — later than a `NativeFn`'s
  `check_portable` rejection would have surfaced it. Inherent: a pure
  arrow cannot close over a Rust `fn`, so the qualified name is the only
  portable handle — the same handle today's string callees use, hence no
  new hazard vs. status quo; the divergence is only vs. the superseded
  pinned-fn design.
- **Ad-hoc injected fns need a chain-resolvable name.** Every `Module`
  has `name()`/`functions()` (`platform/src/eval.rs:391-406`), so dep
  modules mint well-named wrappers — including test stubs (`StubDep`,
  `plugin-lib-handl/tests/env_manifest.rs:60-71`; `EnvStubDep`,
  `kit-core/src/lib.rs:801`; `OtlpModule` over `StubOtlpState`,
  `lib-otlp-c/src/otlp_module.rs:48`; `MockModule`/`NoopModule`,
  `handl-test-utils/src/lib.rs:126`, `:294`) — provided they stay
  registered in the chain they are wrapped against. The genuine gap: a
  `Function` reachable only through a bespoke `ModuleLookup` not backed by
  a named `Module`, or under an alias the wrapper cannot guess — wrappers
  need *some* name for the leaf. `wrap_dep_fn` should assert at mint time
  that `chain.lookup("name.fn")` returns the same function; where that
  fails, mint a scoped alias name or refuse — a name-less native fn cannot
  become an env value, which is the honest limit of categorical purity.
- **Bootstrap simplifies**: loader runtimes need (lang, env record, leaf
  chain); the registry survives for effect ops/FFI, the typechecker
  channel, and as the leaf.
- **Caching stays identity-keyed** (`(module_id, env_id)`); the
  `MODULE_VALUE_CACHE` serde risk narrows to `Resource`-bearing `binds` —
  a pre-existing arrow caveat, nothing new.
- **Cost concentrates in the leaf-name indirection**: record lookup +
  arrow apply + `call_host` leaf vs. today's `format!` + scan — likely
  neutral; `loader_bench` is the arbiter for the per-character `str.*`
  workload.
- **Migration risk concentrates in Phase 3** (ambient std → explicit
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
- ADR-0096 (unified import model), ADR-0060 (generic call dispatch — the
  `route_on_type` split this rides), ADR-0047 §3 (`COMPILED_REGISTRY` —
  the rejected handle pattern), ADR-0044/ADR-0081 (engine genericity —
  `callv` is data)
- ADR-0120 Phase 5 / ADR-0153 (arrow values as cell data — the payload
  regime env records extend)
