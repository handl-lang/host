---
title: '`ffi-rust` — Generic Rust Binding Façade'
type: adr
date: 2026-09-02
status: superseded
status_note: |-
  This ADR proposed a Rust closure binding façade (`plugin-ffi-rust`) to
  collapse `Module`/`Function`/`Plugin` boilerplate. It was never built.
  ADR-0083 eliminates the Rust closure layer entirely — native libs bind
  directly to C/POSIX via `ffi(...)` data declarations, not Rust closures.
  The per-library authoring problem this ADR solved is solved more
  completely by making bindings data, not Rust.

  Originally proposed as build step 1 of [ADR-0050 — Plugin System in handl
  Itself](./0050-plugin-system-in-handl.md). ADR-0083 fulfills that role
  instead.
superseded_by: [ADR-0083]
---

# ADR-0051: `ffi-rust` — Generic Rust Binding Façade

## Context

The platform's binding mechanism — `ModuleRuntime` + `ModuleRule` + the `Module` / `Function`
traits in `handl-platform` — works, but every Rust library exposed to handl pays a
fixed boilerplate tax. Concretely, `plugin-lib-hex` (a minimal library: one function that
returns a codec descriptor) requires:

1. A `HexModule` impl of `Module` (`name` + a `function` match arm per callable).
2. A `HexName` impl of `Function` (`call` with arg validation + the actual work).
3. A `LibraryPlugin` impl of `Plugin` (`id`, `name`, `capabilities`, `metadata`,
   `contribute` → `ctx.contribute::<ModuleRuntime>(ModuleRule::new(HexModule))`, `run` →
   `NotImplemented`).
4. A `Cargo.toml` depending on `handl-plugin-module`, `handl-platform`, the wrapped crate
   (`hex`), `serde_json`.

Verified across the workspace: every `plugin-lib-*` crate repeats 1–4. The `Module` match,
the `Function` arg unpacking, and the `LibraryPlugin` contribute call are structurally
identical across libraries; only the function bodies and names differ. This is acceptable
for a curated stdlib, but it is the wrong surface for ADR-0050's goal, where:

- The handl plugin system needs _one_ Rust bridge (`ast.parse`, `ast.lex`) exposed cleanly,
  not a full `plugin-lib-ast` crate with hand-written `Module`/`Function`/`Plugin` impls.
- Future bindings (regex, unicode, fs primitives, …) should be addable by writing the
  function bodies, not by scaffolding a crate per library.
- The binding seam is inherently Rust (it is the boundary to the host language's library
  ecosystem), so it stays Rust per ADR-0050 — but it must be _thin_, not a per-library
  template.

### What already exists and is kept

- `Module` / `Function` / `ModuleLookup` traits in `handl-platform` — the runtime-side
  contract. Unchanged.
- `ModuleRuntime` + `ModuleRule` in `plugin-module` — the registration substrate. Unchanged;
  the façade contributes `ModuleRule`s into it exactly as hand-written `LibraryPlugin`s do
  today.
- `EvalContext::call_function` — the call path. Unchanged.

The façade does not replace any of these. It removes the per-library _authoring_ boilerplate
by generating the `Module` / `Function` / `Plugin` glue from a declarative spec.

### What is missing

A generic primitive that says "register these Rust closures as the functions of a handl
module named `X`" without writing a `Module` impl, a `Function` impl per closure, and a
`LibraryPlugin` impl. Today the only way to register a Rust fn is to write all three.

## Decision

### 1. One new crate: `plugin-ffi-rust`

A single leaf crate `plugin-ffi-rust` provides a declarative binding façade. It depends on
`handl-platform` (for `Module`/`Function`/`EvalContext`/`EvalError`), `plugin-module` (for
`ModuleRuntime`/`ModuleRule`), and `serde_json`. It depends on **no** engine crate and **no**
specific library crate — it is generic.

The crate's public surface is small:

```rust
/// A Rust function callable from handl, expressed as a closure.
pub type HostFn = Box<
    dyn Fn(&[Value], &mut EvalContext) -> Result<Value, EvalError>
        + Send + Sync,
>;

/// A binding spec: module name + its functions.
pub struct ModuleBinding {
    pub name: &'static str,
    pub functions: &'static [(&'static str, HostFn)],
}

/// A plugin that registers one or more `ModuleBinding`s into `ModuleRuntime`.
pub struct FfiPlugin {
    bindings: &'static [ModuleBinding],
}
```

`FfiPlugin` implements `Plugin`; its `contribute` walks `bindings` and contributes one
`ModuleRule` per `ModuleBinding` into `ModuleRuntime`. The `Module` and `Function` impls
that the `ModuleRule` wraps are generated internally by the façade (see §2) — the library
author never writes them.

### 2. Internal generated `Module` / `Function` glue

The façade provides internal types that implement `Module` and `Function` over the
declarative spec, so the trait boilerplate is written once in `plugin-ffi-rust` and reused
by every binding:

```rust
// A `Function` backed by a closure.
struct ClosureFn { f: HostFn }
impl Function for ClosureFn {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        (self.f)(args, ctx)
    }
}

// A `Module` backed by a name + a slice of (name, ClosureFn).
struct BindingModule { name: &'static str, fns: &'static [(&'static str, ClosureFn)] }
impl Module for BindingModule {
    fn name(&self) -> &str { self.name }
    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.fns.iter().find(|(n, _)| *n == name).map(|(_, f)| f as &dyn Function)
    }
    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.fns.iter().map(|(n, f)| (*n, f as &dyn Function)).collect()
    }
}
```

Because `ModuleBinding` uses `&'static` slices and `Box<dyn … + Send + Sync>`, a binding
can be declared as a `static`/`const`-ish table at the call site (see §3) with no heap
allocation per call beyond the closure boxes.

### 3. Authoring a binding becomes a spec, not a crate

A new Rust binding is a `ModuleBinding` table plus one `FfiPlugin` registration at the
host. The minimal `hex` binding — which today is a full `plugin-lib-hex` crate — becomes:

```rust
use plugin_ffi_rust::{FfiPlugin, ModuleBinding, HostFn};
use serde_json::{json, Value};

fn hex_name(_args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
    Ok(json!({ "type": "encoding", "name": "hex" }))
}

static HEX: ModuleBinding = ModuleBinding {
    name: "hex",
    functions: &[("hex", Box::new(hex_name))],
};

// at host composition:
host.register_plugin(FfiPlugin::new(&[HEX]));
```

No `Module` impl, no `Function` impl, no `LibraryPlugin` impl, no per-library `Cargo.toml`
beyond what already exists for the host. The `plugin-lib-hex` crate's `src/plugin.rs`
(`LibraryPlugin`) and `src/lib.rs` (`HexModule`, `HexName`) collapse into the table above.

For libraries that need more than closures (e.g. async functions, stateful codecs like the
real `Hex` `Encoding` impl), the façade exposes extension points (§5) rather than forcing
everything through `HostFn`.

### 4. `ast.parse` / `ast.lex` are exposed via the façade

The first concrete use of the façade is the ADR-0050 bootstrap bridge: a `ModuleBinding`
named `"ast"` with functions `parse` and `lex`, each a closure that calls into the existing
Rust parser (`plugin-engine-parse-pratt`) and returns the AST/tokens as `serde_json::Value`.
No new `plugin-lib-ast` crate with hand-written trait impls; one `ModuleBinding` table in
the host's composition code.

This is the seam ADR-0050 step 2 requires, and it is the proof that the façade is
sufficient for the bootstrap.

### 5. Extension points for non-closure bindings

Not every Rust function is a plain `Fn(&[Value], &mut EvalContext) -> Result<Value,
EvalError>`. The façade provides escape hatches so libraries that need more keep working
without re-introducing per-library crates:

- **Async functions.** A variant `AsyncHostFn` (`Box<dyn AsyncFunction>`) and a
  `ModuleBinding::with_async` constructor, mirroring the existing `AsyncFunction` trait.
  The generated `Module` dispatches `as_async` for async entries.
- **Stateful modules.** A `ModuleBinding` may carry `&'static dyn Module` directly (an
  escape hatch) for modules whose functions share state that cannot be expressed as
  closures over `&'static` data. This is the path the existing `Encoding`-backed codecs
  (`Hex`, `Base64`) use; they keep their `Module` impls and are wrapped, not regenerated.
- **Existing `plugin-lib-*` crates are not deleted.** They continue to work unchanged.
  The façade is the _preferred_ path for new bindings and for the ADR-0050 bootstrap; it
  does not force a migration of the curated stdlib. A library may migrate to the façade
  when its author finds the boilerplate not worth it.

The escape hatches ensure the façade is a convenience layer over the existing trait model,
not a parallel model. Anything expressible as a hand-written `Module`/`Function`/`Plugin`
is still expressible; the façade just removes the tax for the common case.

### 6. Crate placement and dependencies

```
plugin-ffi-rust  (leaf)
  ├─ handl-platform      (Module, Function, EvalContext, EvalError)
  ├─ plugin-module       (ModuleRuntime, ModuleRule)
  └─ serde_json
```

No dependency on `handl-runtime`, no engine crate, no `handl-language`. The façade is
language- and engine-agnostic — it only knows the `Module`/`Function` contract and the
`ModuleRuntime` acceptor. A host that wants `ast.parse` registers the `ast` `ModuleBinding`
via `FfiPlugin`; the engine that actually parses is wired by the host separately, and the
`ast.parse` closure calls into whatever engine handle the host injected.

This keeps the façade a true leaf: it can be used by any host (CLI, LSP, a test harness)
without dragging in engine crates.

## Consequences

### Positive

- **Binding boilerplate collapses.** A new Rust binding is a `ModuleBinding` table + one
  `FfiPlugin` registration. No `Module` impl, no per-function `Function` impl, no
  `LibraryPlugin` impl, no per-library `Cargo.toml` for trivial bindings. The `hex` example
  drops from ~80 lines across two files to ~10 lines.
- **ADR-0050 bootstrap is unblocked.** `ast.parse` / `ast.lex` are exposed as one
  `ModuleBinding`, not a full `plugin-lib-ast` crate. This is the concrete prerequisite
  for the handl plugin system to read ASTs.
- **The façade is a leaf.** No engine or language dependency. Any host can use it; it
  composes with the existing `ModuleRuntime` substrate without replacing it.
- **Existing libraries are not forced to migrate.** `plugin-lib-*` crates keep working;
  the façade is additive. Migration is per-library, when the boilerplate stops being worth
  it.
- **Escape hatches preserve expressiveness.** Async functions and stateful modules have
  explicit paths; the façade does not become a ceiling that forces libraries back into
  hand-written crates when they outgrow plain closures.

### Trade-offs

- **One indirection per call.** A handl call to a façade-registered function goes
  `EvalContext::call_function` → `ModuleLookup::lookup` → `BindingModule::function` →
  `ClosureFn::call` → the closure. This is one extra `dyn` hop versus a hand-written
  `Function` impl. Negligible for host functions (the closure body dominates), but real
  in micro-benchmarks. The escape hatch (hand-written `Module`) remains for hot paths.
- **`&'static` binding tables constrain construction.** A `ModuleBinding` with
  `&'static` slices is built at compile time. Runtime-constructed bindings (e.g. a module
  whose function list is computed from a config file) need the `Box<dyn Module>` escape
  hatch rather than the `ModuleBinding` table. This is intentional — it keeps the common
  case allocation-free — but it means the façade is not a universal replacement for
  hand-written `Module` impls.
- **Two ways to register a module.** After this ADR, a library can be registered via
  `FfiPlugin` (façade) or via a hand-written `LibraryPlugin` (existing). This is a
  deliberate trade-off (no forced migration), but it means the platform has two authoring
  paths where it had one. Mitigation: docs and the `write-a-plugin` how-to recommend the
  façade for new bindings; the hand-written path is presented as the escape hatch.
- **`plugin-ffi-rust` is a new crate to maintain.** Small, but non-zero. It must track
  changes to `Module`/`Function`/`ModuleRuntime` (rare; these are stable platform
  contracts).

## References

- [ADR-0050 — Plugin System in handl Itself](./0050-plugin-system-in-handl.md) — this ADR
  implements its build step 1 (the permanent "binding lib" Rust layer) and its step 2
  (`ast.parse` / `ast.lex` exposure).
- [ADR-0040 — VM is an Operation, not a Subsystem](./0040-vm-is-an-operation-not-a-subsystem.md)
  — the `ModuleRuntime` + `ModuleRule` substrate this façade builds on.
- `crates/platform/src/eval.rs` — `Module` / `Function` / `ModuleLookup` trait
  definitions (lines 299–388).
- `crates/plugin-module/src/lib.rs` — `ModuleRuntime`, `ModuleRule`, `ModuleRegistry`
  (the registration substrate).
- `crates/plugin-lib-hex/src/lib.rs`, `crates/plugin-lib-hex/src/plugin.rs` — the
  per-library boilerplate (`HexModule`, `HexName`, `LibraryPlugin`) this ADR collapses.
