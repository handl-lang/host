---
title: Modules as First-Class Values
type: adr
date: 2026-09-02
status: superseded
status_note: |-
  This ADR over-engineered the problem by re-inventing environment
  semantics as a separate subsystem (`Module` value type,
  `CLOSURE_REGISTRY`, `ValueModule`/`HandlModuleLookup` adapters,
  `plugin-lib-module` leaf). The reduction in ADR-0049 — a module is a
  named bag of bindings produced by evaluating code; `import`/`export` are
  sugar over evaluate-and-bind; `module.field` is field access on an
  environment value — dissolves most of this ADR's machinery. Kept as a
  historical record of the design path considered and rejected.

  Builds on [ADR-0013 — Standard Library](./0013-standard-library.md),
  [ADR-0040 — VM is an Operation, Not a Subsystem](./0040-vm-is-an-operation-not-a-subsystem.md),
  [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md),
  [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md),
  and [ADR-0047 — Meta Functions as `plugin-lib-*` Crates](./0047-meta-functions-as-plugin-lib-crates.md).
superseded_by: [ADR-0049]
---

# ADR-0048: Modules as First-Class Values

## Context

ADR-0041 made `Language` a first-class value: a `Language` is a tagged
`serde_json::Value` (`language_value` / `as_language` in
`handl-language/src/lib.rs`), and `eval`/`parse`/`lex`/`compile` are
handl-callable functions over it. ADR-0047 is finishing the job for the
_meta functions_ themselves — splitting `MetaHostFunctions` into
`plugin-lib-lex`/`parse`/`eval`/`compile`/`run-compiled` leaf crates so
they compose and tree-shake like every other `plugin-lib-*`.

But one asymmetry remains, and it is the exact asymmetry ADR-0041 fixed
for `Language`. **A `Module` is not a handl value.** Today:

- `Module` and `Function` are Rust trait objects
  (`handl-platform/src/eval.rs`): `Arc<dyn Module>`, `&dyn Function`.
- `ModuleRegistry` (`plugin-module/src/lib.rs`) is a `Vec<Arc<dyn Module>>`
  with a `split_once('.')` lookup. It implements `ModuleLookup`, which
  `EvalContext` holds as `Option<Rc<dyn ModuleLookup>>`.
- Every stdlib module is a Rust `impl Module` / `impl Function`:
  `plugin-lib-hello` (`HelloModule` / `HelloGreet`), `plugin-lib-collections`
  (`CollectionsModule`), and ~40 other `plugin-lib-*` crates. A handl
  program cannot author a module, register one, inspect one, or replace
  one. The module system is a fixed wall above the program, the way `eval`
  was before ADR-0041.

### What this blocks

ADR-0013 already states the end state: the stdlib is handl, with Rust as
the _specification and fallback_ for performance
(<project/adrs/0013-standard-library.md>, "File layout"). That end state
is unreachable today for two reasons, only one of which is this ADR's
subject:

1. **The eval engine cannot run the constructs the stdlib needs.**
   `handl-defining-handl/src/lib.rs` (lines 17–52) is explicit: the
   data-driven engines cover a subset. `match`, `handle`, `field-access`,
   `import`, `module`, first-class function values, and several `Prim` ops
   are still `Scheme::Foreign` dispatches or unimplemented. You cannot
   write `collections.map(items, fn(x) => x * 2)` in handl until
   `field-access`, `match`, and function values are evaluable by the
   data-driven engines. That is unfinished ADR-0044 Phase 4–5 work, **not**
   this ADR. This ADR depends on it but does not do it.

2. **Modules are not values.** Even once the eval engine is complete, a
   handl program has no way to _package_ a set of functions as a module
   value, _register_ it where `math.add`-style qualified calls resolve,
   or _implement_ the registry/lookup logic itself. This is what ADR-0048
   fixes — the same move ADR-0041 made for `Language`, one level down.

### The insight

When `Language` went first-class, the act of interpreting stopped being a
fixed host and became a value-level operation (`eval : Language -> Source
-> Value`). The parallel move for modules: the act of _resolving a call_
stops being a fixed Rust registry and becomes a value-level operation
(`lookup : Module -> Name -> Function`). The thing that hosts the
functions and the thing hosted become the same kind of thing.

| first-class languages (ADR-0041)            | first-class modules (this ADR)                   |
| ------------------------------------------- | ------------------------------------------------ |
| `eval : Language -> Source -> Value`        | `call : Module -> Name -> Args -> Value`         |
| authority lives in the language's behaviors | authority lives in the module's functions        |
| gate the language, not `eval`               | gate the module, not the registry                |
| `self` = the running language               | the running module set = a value you can inspect |
| self-hosting via `eval(self, ...)`          | module system in handl via `lookup(self, ...)`   |
| pure language = safe to eval anywhere       | pure module = safe to call anywhere              |

### The closure-serialization crux

There is one real technical problem ADR-0041 did _not_ face and this ADR
does. `Language` is pure data — `language_value` is `serde_json::to_value`
of a fully-serializable struct. A `Module` whose functions are handl
closures is **not** pure data: a closure captures an environment, and
closures are not `serde`-serializable in general. So a `Module` value
cannot simply be `serde_json` of its functions the way a `Language` value
is `serde_json` of its grammar.

This is the same shape as the _compiled-program_ problem ADR-0041 §5 /
ADR-0047 §3 already solved for `compile`/`run_compiled`: the artifact
(cranelift `CompiledResult`, bytecode `Vec<Instr>`) is not serializable,
so `meta.rs` keeps a thread-local `COMPILED_REGISTRY: RefCell<HashMap<u64,
CompiledProgram>>` and the value handed to handl is a serializable handle
`{"__compiled__": id, "target": ...}`. The non-serializable artifact never
crosses the trait boundary; only the handle does.

This ADR applies the same pattern to closures: a thread-local
`CLOSURE_REGISTRY` holds the actual closure values (or, more precisely,
the eval-engine-side state needed to re-enter evaluation of a closure
body), and a `Module` value carries serializable handles to them. The
`Module` value itself is serializable; the closures it references are not,
and live in the registry for the process lifetime. This is honest about
the constraint and reuses an existing, proven pattern rather than
inventing a new one.

## Decision

Make `Module` a first-class value in handl's value space, mirroring
`Language` (ADR-0041) and the compiled-program handle (ADR-0047 §3). Make
module definition, registration, and lookup handl-callable functions over
it, as `plugin-lib-*` leaf crates per the ADR-0047 convention. The host
shrinks to a kernel that ships the closure registry + a `ModuleLookup`
adapter that can delegate to a handl-defined module map. Everything else —
including the module system itself — can be a handl value built from those
primitives.

### 1. `Module` is a handl-level type

A `Module` is a structured value with two components:

- a **name** (the namespace prefix, e.g. `"math"`), and
- a **function table**: a map from function name to a callable value.

The callable value is one of:

- a **closure handle** — a tagged value `{"__closure__": id}` referencing
  an entry in the thread-local `CLOSURE_REGISTRY` (the non-serializable
  closure lives there; the handle is serializable), or
- a **foreign name** — a tagged value `{"__foreign__": "math.add"}`
  referencing a Rust `HostFn` by qualified name (the existing
  `Scheme::Foreign` / `HostFunctions::lookup` path). This is how a
  handl-written module delegates to a Rust-implemented primitive for
  performance or for things that cannot be expressed in handl (e.g.
  `crypto.sha256`).

The `Module` value itself is a tagged JSON object:

```json
{
  "__module__": {
    "name": "math",
    "functions": {
      "add": { "__closure__": 17 },
      "mul": { "__closure__": 18 },
      "sha256": { "__foreign__": "crypto.sha256" }
    }
  }
}
```

`handl-language` (or a new `handl-module-values` crate, see §4) gains
`module_value` / `as_module` / `is_module_value` mirroring
`language_value` / `as_language` / `is_language_value`.

### 2. The `CLOSURE_REGISTRY`

A thread-local `RefCell<HashMap<u64, ClosureEntry>>` in the runtime,
mirroring `COMPILED_REGISTRY` (`handl-runtime/src/meta.rs`). A
`ClosureEntry` carries whatever the eval engine needs to re-enter
evaluation of a closure body: the `Ast` of the body, the captured
environment (a serializable variable map, since environments _are_
serializable — only the _closure as a whole_ is not, because it pairs a
body with an environment and the body is an arena reference), and the
`Language` the closure was created under (so a closure carries its
defining language, the way a `CompiledProgram` carries its `lang` + `ast`).

Closures are not `Send`; the registry is thread-local, matching the
engines' single-threaded model and the existing `COMPILED_REGISTRY`
precedent. A `ClosureHandle` does not survive across threads/processes —
the same constraint as a compiled-program handle, and for the same
reason.

### 3. `Function` adapter for closure handles

The Rust `Function` trait stays — it is the seam for Rust-implemented
primitives (the ADR-0013 fallback). A new adapter
`ClosureFunction { handle: ClosureHandle }` implements `Function` by:
looking up the `ClosureEntry` in `CLOSURE_REGISTRY`, binding the call
arguments to the closure's parameters, re-entering the eval engine on the
closure body under the closure's defining `Language`, and returning the
result. This is structurally identical to how `RunCompiledHostFn` looks up
a `CompiledProgram` and runs it.

A `Module` value is backed on the Rust side by a `ValueModule` adapter
implementing the `Module` trait: `name()` reads the `__module__.name`
field; `function(name)` reads `__module__.functions[name]` and returns a
`&dyn Function` — either a `ClosureFunction` (for a `__closure__` handle)
or a lookup into the host's `HostFunctions` (for a `__foreign__` name).
The `&dyn Function` borrow is tied to the `ValueModule`'s lifetime, which
is tied to the `Module` value's lifetime in the eval context. This is the
one lifetime wrinkle that needs care; see §"Trade-offs".

### 4. `plugin-lib-module` — the leaf crate

Following ADR-0047, module operations are a `plugin-lib-*` leaf crate,
not a monolithic blob. One crate, one module, namespaced callables:

| Crate               | Module   | Callable                                                                                                 |
| ------------------- | -------- | -------------------------------------------------------------------------------------------------------- |
| `plugin-lib-module` | `module` | `module.define(name, functions)`, `module.register(mod)`, `module.lookup(name)`, `module.functions(mod)` |

- `module.define(name, functions)` → a `Module` value. `functions` is a
  handl object mapping function names to closure values (or foreign-name
  strings). The host function wraps each closure in a `CLOSURE_REGISTRY`
  entry and builds the `__module__` value.
- `module.register(mod)` → registers a `Module` value into the _current_
  module set (the handl-defined registry, see §5). Returns `null`.
- `module.lookup(qualified_name)` → the function value (a closure handle
  or foreign name) for `math.add`-style names, or an error if not found.
  This is the handl-callable read path; the write path is
  `module.register`.
- `module.functions(mod)` → the function table as a handl object, for
  introspection (mirrors `Module::functions()`).

`plugin-lib-module` depends on `handl-runtime` (for the
`CLOSURE_REGISTRY` + `RuntimeFactory` to re-enter eval) + `handl-language`
(for the `__module__` value helpers) only. It is a leaf, like every other
`plugin-lib-*` per ADR-0047.

### 5. The module system can be written in handl

With `Module` as a value and `module.define`/`register`/`lookup` as
handl-callable functions, the _registry logic itself_ — the
`Vec<Arc<dyn Module>>` + `split_once('.')` lookup that today lives in
`plugin-module/src/lib.rs` — can be rewritten in handl as a value: a map
from module name to `Module` value, with a `lookup` function that splits
on `.` and dispatches. The Rust `ModuleLookup` trait stays as the seam
`EvalContext` holds, but a new `HandlModuleLookup` adapter implements it
by _calling back into handl_ — delegating `lookup(qualified_name)` to the
handl-defined registry value via the `module.lookup` host function.

This is the direct payoff of the user's idea: the module system becomes
handl code, backed by a thin Rust adapter, the same way `eval(self, ...)`
made the _language_ self-hosting in `handl-defining-handl`. The module
system is to module resolution what `handl-defining-handl` is to
language evaluation.

### 6. The bootstrap

The module system is needed to load the module system — the same
chicken-and-egg `handl-defining-handl` already tackles for the language
spec. The answer is the same: a minimal Rust `ModuleRegistry`
(`plugin-module`, unchanged) boots the process, loads a handl-defined
module-system value (a `.hnl` file that defines `module.define`/
`register`/`lookup`-based registry logic and registers the stdlib modules
as `Module` values), then hands off: the `HandlModuleLookup` adapter
delegates to the handl registry value from that point on. The Rust
registry is the bootloader; the handl registry is the running system.

### 7. On "an exclusive way for the runtime to access Rust crate/code"

This came up in the design discussion and is left as a **separate
decision** (not resolved by this ADR). The honest assessment: **no new
mechanism is needed.** `Scheme::Foreign { name, .. }` →
`HostFunctions::lookup(name)` → `HostFn::call(&[Value]) -> Result<Value>`
(`handl-runtime/src/meta.rs`) is already the thin Rust-access seam. A
handl-written stdlib means most functions become `Scheme` combinator
trees (compiled by the eval engine); `Foreign` dispatch stays for true
primitives — crypto, perf-critical string ops, anything that cannot be
expressed in handl. The `__foreign__` handle in §1 is exactly this seam,
reused, not a new primitive.

A _new_ `Scheme::RustFfi` variant or a "call arbitrary Rust crate by
path" mechanism would add machinery for no gain over `Foreign` and would
cut against ADR-0044's "language as data" discipline (a `Foreign` name is
data; an arbitrary Rust path is not). This ADR takes no position on
whether such a primitive is ever wanted, but recommends **not** building
it as part of this work.

## Consequences

### Positive

- **The ADR-0013 end state becomes reachable.** Once the eval engine
  covers the needed constructs (a prerequisite, not this ADR), the stdlib
  can be written in handl as `Module` values, with Rust `Foreign` fallback
  for true primitives — exactly what ADR-0013 specified.
- **The module system can be written in handl.** The registry, the
  `split_once('.')` lookup, the merge policy — all handl values, backed
  by a thin `HandlModuleLookup` Rust adapter. This is the user's stated
  goal.
- **Composability restored for modules.** A `Module` is a value you can
  bind, pass, store, merge, restrict, and inspect — the same moves
  ADR-0041 unlocked for `Language`. `merge(base_math, extra_math)` and
  `restrict(mod, ["add", "mul"])` become ordinary handl functions.
- **Tree-shaking per ADR-0047.** Module operations are a `plugin-lib-*`
  leaf, selectable and swappable like every other. A host that wants
  `module.define` but not `module.lookup` (a build-time-only host) can
  express that.
- **Inspection.** `module.functions(mod)` lets a handl program introspect
  a module before calling — the module analogue of ADR-0046's
  `capabilities(lang)`.
- **No new Rust-access primitive.** Reuses `Scheme::Foreign` /
  `HostFunctions`, the existing seam. No new `Scheme` variant, no
  language-level FFI escape.

### Trade-offs

- **Closures are not serializable.** A `Module` value is serializable, but
  the closures it references are not — they live in the thread-local
  `CLOSURE_REGISTRY` for the process lifetime. This is the same
  constraint as a compiled-program handle (ADR-0047 §3) and for the same
  reason. A `Module` value does not survive across threads/processes
  unless its functions are all `__foreign__` names (which _are_
  serializable). This is inherent to first-class closures and is not
  fixable without serializing closures (a research problem).
- **One lifetime wrinkle.** `ValueModule::function(name)` returns a
  `&dyn Function` borrowed from the `ValueModule`, which is borrowed from
  the `Module` value in the eval context. The existing `ModuleLookup`
  contract returns `&dyn Function` with the lifetime of the lookup call,
  so this composes, but it is tighter than `language_value` (which hands
  out owned `Language` clones). The adapter must hold the closure
  `Function` objects alive for the lookup's lifetime — likely by
  constructing them on each `function()` call and returning a reference
  into a scratch slot, or by caching. This is solvable but is real work.
- **Depends on eval-engine completeness.** This ADR does not make the
  stdlib writable in handl by itself — it removes the _module_ blocker.
  The _eval-engine_ blocker (field-access, match, first-class function
  values, the unimplemented `Prim` ops) is unfinished ADR-0044 Phase 4–5
  work and is a hard prerequisite for any non-trivial stdlib module
  written in handl. A `math` module that only uses `binary.add`-style
  `Prim` ops can be written today; `collections.map` cannot.
- **Bootstrap chicken-and-egg.** The module system is needed to load the
  module system. Solved the same way as `handl-defining-handl`: a minimal
  Rust `ModuleRegistry` boots, loads a handl-defined module-system value,
  hands off. Real work, but the pattern exists.
- **Blast radius.** Reifying `Module` as a value, adding the
  `CLOSURE_REGISTRY`, adding `plugin-lib-module`, and adding the
  `HandlModuleLookup` adapter touches `handl-language` (or a new crate),
  `handl-runtime`, `plugin-module`, and the host wiring. Multi-phase, not
  a single PR — like ADR-0041 and ADR-0047 before it.

### Neutral

- **The Rust `Module`/`Function` traits stay.** They are the seam for
  Rust-implemented primitives and for the `HandlModuleLookup` adapter.
  They are not deleted; they are demoted from "the only way to be a
  module" to "the Rust-side interface to a module value." This mirrors
  how ADR-0044 kept `Scheme` as data but kept the engine _traits_ as
  Rust.

## Implementation phases

### Phase 1 — `Module` value + `CLOSURE_REGISTRY`

Add `module_value` / `as_module` / `is_module_value` to `handl-language`
(or a new `handl-module-values` crate if keeping `handl-language`
data-only is preferred — `handl-language` today has `language_value`/
`ast_value`, so the precedent is to put value helpers there). Add the
thread-local `CLOSURE_REGISTRY` + `ClosureEntry` + `ClosureHandle` to
`handl-runtime`, mirroring `COMPILED_REGISTRY`. No handl-callable
functions yet; just the value type and the registry.

### Phase 2 — `plugin-lib-module` leaf crate

`module.define` / `module.register` / `module.lookup` /
`module.functions` as a `plugin-lib-*` leaf per ADR-0047. Depends on
`handl-runtime` + `handl-language` only. Each callable takes/returns
`Module` values; `module.define` populates the `CLOSURE_REGISTRY` and
builds the `__module__` value.

### Phase 3 — `ClosureFunction` + `ValueModule` adapters

The Rust adapters that make a `Module` value quack like a `Module` trait
object: `ClosureFunction` re-enters eval on a closure handle;
`ValueModule` exposes `name()`/`function()` over a `Module` value. Solve
the `&dyn Function` lifetime wrinkle here.

### Phase 4 — `HandlModuleLookup` adapter

A `ModuleLookup` impl that delegates `lookup` to a handl-defined
module-map value via `module.lookup`. This is the seam that lets the
module _system_ (registry + lookup logic) be written in handl. Wire it
into `EvalContext` as an alternative to the Rust `ModuleRegistry`.

### Phase 5 — One stdlib module in handl, as proof

Take a module that only uses already-supported constructs (e.g. a subset
of `math`: `add`, `mul`, `sub`, `div`) and write it as a `.hnl` file that
calls `module.define` and `module.register`. Load it via the bootstrap
path. This proves the end-to-end ADR-0013 path on a small slice and is
the honest deliverable before tackling `collections`/`text` (which need
the eval-engine prerequisites).

### Phase 6 — The module system itself in handl

Write the registry + `split_once('.')` lookup + merge policy as a handl
value, backed by `HandlModuleLookup`. The Rust `ModuleRegistry` becomes
the bootloader only. This is the user's stated goal, realized.

## References

- [ADR-0013 — Standard Library](./0013-standard-library.md) — the end
  state this ADR makes reachable (stdlib in handl, Rust as fallback)
- [ADR-0040 — VM is an Operation, Not a Subsystem](./0040-vm-is-an-operation-not-a-subsystem.md)
  — `Module`/`Function`/`ModuleLookup` are the runtime-side analogue of
  the operation/behavior/rule-acceptor pattern
- [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
  — the move this ADR mirrors, one level down (modules instead of
  languages)
- [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
  — the data model; this ADR's `__foreign__` handle reuses `Scheme::Foreign`
- [ADR-0047 — Meta Functions as `plugin-lib-*` Crates](./0047-meta-functions-as-plugin-lib-crates.md)
  — the leaf-crate convention this ADR follows for `plugin-lib-module`;
  the `COMPILED_REGISTRY` / handle pattern this ADR reuses for closures
- `crates/handl-language/src/lib.rs` — `language_value`/`as_language`/
  `ast_value`/`as_ast` (the pattern `module_value`/`as_module` mirrors)
- `crates/handl-runtime/src/meta.rs` — `COMPILED_REGISTRY`,
  `CompiledProgram`, `compiled_value` (the pattern
  `CLOSURE_REGISTRY`/`ClosureEntry`/`closure_value` mirrors)
- `crates/plugin-module/src/lib.rs` — `ModuleRegistry`, `ModuleRuntime`,
  `ModuleRule` (the Rust registry that becomes the bootloader)
- `crates/platform/src/eval.rs` — `Module`, `Function`, `ModuleLookup`
  traits (the Rust seam that stays)
- `crates/handl-defining-handl/src/lib.rs` — the self-hosting pattern
  this ADR generalizes from languages to modules
