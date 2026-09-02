# ADR-0047: Meta Functions as `plugin-lib-*` Crates

## Status

Accepted

Supersedes [ADR-0030 — lib-lang](./0030-lib-lang.md) (stale: pre-0044
`ac-lib-lang` façade over a trait-based AST and Rust emitters, none of which
survive ADR-0044). Refines [ADR-0041 — Languages as First-Class
Values](./0041-languages-as-first-class-values.md) §"Meta host functions" and
[ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
§3 (crate structure).

## Context

ADR-0041 introduced the meta host functions — `lex`, `parse`, `eval`, `run`,
`compile`, `run_compiled` — as the handl-callable surface that makes a
`Language` a first-class value: a handl program can write `eval(self, ast)` and
recursively evaluate. ADR-0044 §3 split each operation into an abstraction
crate (`plugin-engine-*`) and engine crates, but did **not** specify where the
*handl-callable* meta functions live. They were implemented in
`handl-runtime/src/meta.rs` as a single `MetaHostFunctions` struct that
implements `HostFunctions` and is wired in by `build_runtime` / `Kit::new`.

This placement breaks the platform's composability convention. Every other
handl-callable library is a leaf `plugin-lib-*` crate exposing one module with
namespaced functions:

- `plugin-lib-json` → module `json` → `json.*`
- `plugin-lib-toml` → module `toml` → `toml.*`
- `plugin-lib-yaml`, `plugin-lib-collections`, `plugin-lib-crypto`, …

The meta functions are the one exception: they are a monolithic blob inside
`handl-runtime`, not composable, not tree-shakeable, not independently
selectable. A host that wants `eval` but not `lex`/`parse` cannot express
that; a kit that wants to swap the meta `eval` for an instrumented one cannot
do so without forking `handl-runtime`.

### The dependency problem

The obstruction to a naive `plugin-lib-lang` is that each meta function calls
`build_runtime(lang, config, delegate)` to spin up a sub-`Runtime` and recurse
— that recursion *is* the self-hosting property. So the meta functions appear
to need `handl-runtime`. But `plugin-lib-*` crates are leaves: none of them
depend on `handl-runtime` today (verified: zero `handl-runtime` deps across
all `plugin-lib-*` `Cargo.toml`). A `plugin-lib-lang` that depended on
`handl-runtime` while `handl-runtime` registered it would be a cycle.

### The naming collision

`lang.eval` is already taken in the **data** model: engines look up
`lang.eval[node.kind]` → `EvalSchemeDef` → `Scheme` to interpret a node kind
(ADR-0044 §4). Reusing `lang.eval` as a *callable* host-function name makes
"the eval scheme for a node kind" and "call eval on an AST" share a symbol.
The bare names `eval`/`parse`/`lex` currently dodge this, but at the cost of
living outside the `plugin-lib-*` namespace convention.

### `handl-platform` is language-agnostic

`handl-platform` has zero dependency on `handl-language` (verified: no
`handl-language` in its `Cargo.toml` or any source file). It is the
language-agnostic platform layer — `Plugin`, `CapabilityRequest`,
`HostFunctions` (re-exported from `plugin-engine-eval`), `Module`, etc. The
`RuntimeFactory`/`RuntimeHandle` traits take `Language`, `Source`, `Tokens`,
`Ast` as parameters, so they are language-tied and do **not** belong in
`handl-platform`. They need their own crate.

## Decision

### 1. Five new leaf crates, one operation each

Introduce five `plugin-lib-*` crates mirroring `plugin-lib-json`/`toml`:

| Crate                  | Module   | Callable                          |
| ---------------------- | -------- | --------------------------------- |
| `plugin-lib-lex`       | `lex`    | `lex.lex(lang, text)`             |
| `plugin-lib-parse`     | `parse`  | `parse.parse(lang, tokens)`       |
| `plugin-lib-eval`      | `eval`   | `eval.eval(lang, ast)`            |
| `plugin-lib-compile`   | `compile`| `compile.compile(lang, ast, target)` |
| `plugin-lib-run-compiled` | `run_compiled` | `run_compiled.run_compiled(handle)` |

Each is a leaf: depends on `handl-runtime` (the abstraction crate, see §2) +
`handl-language` only. Each exposes one `Module` whose `function(name)`
matches its callable(s), and one `LibraryPlugin` that registers it.
Composition, tree-shaking, and include/exclude work identically to every
other `plugin-lib-*` crate.

The callable namespace `lex.lex` / `parse.parse` / `eval.eval` /
`compile.compile` / `run_compiled.run_compiled` disambiguates from the data
namespace `lang.eval[node.kind]` / `lang.grammar[token]`. No symbol is
overloaded.

### 2. `handl-runtime` becomes the abstraction + `Runtime` impl; engines register by name

The `RuntimeFactory`/`RuntimeHandle` traits are language-tied (they take
`Language`, `Source`, `Tokens`, `Ast`), so they cannot live in
`handl-platform`. They live in `handl-runtime`.

**No `handl-runtime-native` crate.** The existing `handl-runtime` crate keeps
its `Runtime` struct and gains the `RuntimeFactory`/`RuntimeHandle` traits +
`CompiledHandle` + `CompileEngine`/`CompiledArtifact` traits + a generic
`COMPILED_REGISTRY`. The hardcoded `from_config` match (which pulled all 5
engine crates) is **deleted** and replaced with an `EngineRegistry` — a
name→engine map populated by the host, looked up by `Runtime`:

```rust
// host populates the registry — no bundling crate, no hardcoded match
let mut registry = EngineRegistry::new();
registry.register_lex("data", Box::new(LexPatternEngine::new()));
registry.register_parse("pratt", Box::new(PrattEngine::new()));
registry.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
registry.register_eval("bytecode", Box::new(BytecodeEngine::new()));
registry.register_compile("native", Box::new(CraneliftCompileEngine::new()));

// name-based selection — kit manifests keep `eval = "tree-walk"`
let runtime = Runtime::from_registry(lang, &registry, RuntimeConfig {
    lex: "data", parse: "pratt", eval: "tree-walk",
});
```

The engine crates (`plugin-engine-lex-pattern`, `plugin-engine-parse-pratt`,
`plugin-engine-eval-tree-walk`, `plugin-engine-eval-bytecode`,
`plugin-engine-eval-cranelift`) are **already separate** and already implement
`LexerEngine`/`ParserEngine`/`EvalEngine` traits. Each registers itself by
name; `handl-runtime` names no engine crate. Adding an engine = register it;
no `handl-runtime` edit.

**`handl-runtime`'s `Cargo.toml` drops all 5 engine impl crates.** It depends
only on `handl-language` + the three abstraction crates (`plugin-engine-lex`,
`plugin-engine-parse`, `plugin-engine-eval`) for the engine trait definitions
and `HostFunctions`/`HostFn`. The engine *impls* (tree-walk, bytecode,
cranelift, pratt, lex-pattern) are deps of the **host**, not of
`handl-runtime`. This is the key decoupling: a `handl-runtime` consumer is
not forced to link cranelift.

`RuntimeConfig` **stays** — but it is now just names (`{ lex: "data", parse:
"pratt", eval: "tree-walk" }`), resolved against the registry at
`from_registry` time, not a hardcoded match. Kit manifests keep their
`eval = "tree-walk"` selection. The registry is the indirection that makes
name-based selection composable without `handl-runtime` knowing any engine.

`RuntimeConfig` has no `compile` field — compile engine selection is by the
`target` argument to `compile.compile(lang, ast, target)` / `handle.compile(ast, target)`,
which selects from the registry's compile map by target name (`"native"`,
`"bytecode"`). This asymmetry is intentional: lex/parse/eval are
one-engine-per-runtime (selected by config), while compile is multi-target
(you may compile to both bytecode and native in the same session, selecting
per call).

```rust
// handl-runtime (abstraction + Runtime impl)
// deps: handl-language, plugin-engine-lex, plugin-engine-parse,
//       plugin-engine-eval (for engine traits + HostFunctions/HostFn)

/// A serializable handle to a compiled program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledHandle {
    pub id: u64,
    pub target: String,
}

/// A compiled artifact — engine-specific, non-serializable, held in the
/// registry behind a trait object. Each compile engine produces its own impl.
pub trait CompiledArtifact {
    fn run(&self, lang: &Language, ast: &Ast, host: Option<&dyn HostFunctions>)
        -> Result<Value, LangError>;
}

/// A compile engine — produces `CompiledArtifact` impls from an `Ast`.
pub trait CompileEngine {
    fn compile(&self, lang: &Language, ast: &Ast, target: &str)
        -> Result<Box<dyn CompiledArtifact>, LangError>;
}

/// A name→engine map, populated by the host. `handl-runtime` looks up by
/// name; it names no engine crate. Multiple engines per stage coexist.
pub struct EngineRegistry {
    lex: HashMap<String, Box<dyn LexerEngine>>,
    parse: HashMap<String, Box<dyn ParseEngineDyn>>,
    eval: HashMap<String, Box<dyn EvalEngine>>,
    compile: HashMap<String, Box<dyn CompileEngine>>,
}

/// Name-based engine selection, resolved against an `EngineRegistry`.
/// Serializable — kit manifests carry this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub lex: String,
    pub parse: String,
    pub eval: String,
}

/// Builds a `RuntimeHandle` for a `Language`, with meta re-attached so the
/// sub-runtime can recurse. The factory carries the registry + config and
/// re-injects them into sub-runtimes. It also injects `self` as a global
/// variable bound to the `Language` value (ADR-0041 Phase 3) — this is not
/// a parameter; the factory does it automatically.
pub trait RuntimeFactory {
    fn build(
        &self,
        lang: Language,
        delegate: Option<Rc<dyn HostFunctions>>,
    ) -> Result<Box<dyn RuntimeHandle>, LangError>;
}

/// A handle to a runtime bound to a `Language`. All stage calls go through
/// this trait; the impl owns the engines and the `COMPILED_REGISTRY`.
pub trait RuntimeHandle {
    fn lex(&self, src: &Source)        -> Result<Tokens, LangError>;
    fn parse(&self, toks: &Tokens)     -> Result<Ast, LangError>;
    fn eval(&self, ast: &Ast)          -> Result<Value, LangError>;
    fn compile(&self, ast: &Ast, target: &str) -> Result<CompiledHandle, LangError>;
    fn run_compiled(&self, handle: &CompiledHandle) -> Result<Value, LangError>;
}
```

- `plugin-lib-lex`/`parse`/`eval`/`compile`/`run-compiled` depend on
  `handl-runtime` + `handl-language` only. Each plugin holds a
  `&dyn RuntimeFactory` injected at registration and calls
  `factory.build(...)` per invocation, then `handle.lex(...)` /
  `handle.parse(...)` / `handle.eval(...)` / `handle.compile(...)` /
  `handle.run_compiled(...)`.
- `Runtime` implements `RuntimeFactory` and `RuntimeHandle`. **The factory
  re-injects the meta functions into the sub-runtime itself** — so the
  recursion (sub-runtime gets `lex`/`parse`/`eval`/`compile` re-attached)
  lives in `handl-runtime`, not in the `plugin-lib-*` crates. That is what
  breaks the cycle: the `plugin-lib-*` crates never reference each other or
  any engine crate; only the host (which populates the registry) does.
- Hosts wire it: populate an `EngineRegistry`, build a `Runtime` via
  `from_registry(lang, &registry, config)`, construct a `RuntimeFactory` from
  it, pass the factory into each `plugin-lib-*` plugin at registration,
  register the plugins.
- `RuntimeConfig` stays as name-based selection (serializable, kit-manifest-
  friendly). The registry is the indirection that resolves names to engines
  without `handl-runtime` hardcoding any match.

`HostFunctions` / `HostFn` are re-exported from `plugin-engine-eval` (already
a low crate); `handl-runtime` re-exports them so `plugin-lib-*` crates depend
on `handl-runtime` only, not `plugin-engine-eval` directly.

### 3. `compile` / `run_compiled` are leaf-able via `CompiledHandle` + `CompiledArtifact`

The compiled artifact (cranelift `CompiledResult` + `JITModule`, or bytecode
`Vec<Instr>`) is not `serde`-serializable and not `Send`. It stays in
`handl-runtime`'s thread-local `COMPILED_REGISTRY` — but now the registry
holds `Box<dyn CompiledArtifact>`, not an engine-specific enum. Each compile
engine (`plugin-engine-eval-cranelift`, `plugin-engine-eval-bytecode`)
produces its own `CompiledArtifact` impl. The `plugin-lib-compile` and
`plugin-lib-run-compiled` crates work entirely with serializable handles:

- `compile.compile(lang, ast, target)` → calls `factory.build(lang, ...)`,
  then `handle.compile(ast, target)` → the `Runtime` asks its `CompileEngine`
  to produce a `Box<dyn CompiledArtifact>`, stores it in the registry, returns
  a `CompiledHandle` wrapped as a handl value `{"__compiled__": id, "target": target}`.
- `run_compiled.run_compiled(handle)` → extracts the `CompiledHandle` from
  the value, calls `factory.build(...)` (needed to re-attach meta for
  `Foreign` dispatch during execution), then `handle.run_compiled(&compiled_handle)`
  → the `Runtime` looks up the `CompiledArtifact` in the registry and calls
  `artifact.run(...)`.

The non-serializable artifact never crosses the trait boundary; only the
serializable handle does. This makes both crates proper leaves. The
`COMPILED_REGISTRY` is generic (holds `Box<dyn CompiledArtifact>`) — no
engine-specific types in `handl-runtime`.

### 4. `run` is dropped — it is composition, not a primitive

`run(lang, text)` was the convenience composite
`eval(lang, parse(lang, lex(lang, text)))`. It is not a primitive — it is
composition of the three staged callables. With `lex.lex`/`parse.parse`/
`eval.eval` now individually available as handl-callable functions, `run` is
expressed at the handl level:

```
eval.eval(lang, parse.parse(lang, lex.lex(lang, text)))
```

The 1-arg `run(text)` form (using `self`) becomes:

```
eval.eval(self, parse.parse(self, lex.lex(self, text)))
```

No `run` host function, no `plugin-lib-run` crate. The Rust-level
`Runtime::run` convenience method may stay (it is just
`self.eval(&self.parse(&self.lex(src)?)?)` sugar for host-side Rust code);
what is dropped is the **handl-callable** `run` — there is no
`run.run(lang, text)` function exposed to handl programs. Dropping it
eliminates a redundant primitive and keeps the surface minimal: five
operations (`lex`, `parse`, `eval`, `compile`, `run_compiled`), each its
own crate.

### 5. `self` stays as an ambient global

`self` is an ambient global variable bound by `RuntimeFactory::build`
(ADR-0041 Phase 3), not a host function. Unchanged. The factory continues to
inject `self` as a global bound to the `Language` value.

### 6. Scope types re-exported from `plugin-lib-eval`

`plugin-ext-eval` already re-exports `Scope`, `EvalContext`, `Function`,
`AsyncFunction`, `Module`, `ModuleLookup` from `handl-platform`.
`plugin-lib-eval` does the same — re-exports them from `handl-platform`
directly. No new home crate; the source is `handl-platform`.

## Consequences

### Positive

- **Composability restored.** `lex`/`parse`/`eval`/`compile` are selectable,
  tree-shakeable, and swappable like every other `plugin-lib-*`. A host can
  take `eval` without `lex`/`parse`; a kit can swap an instrumented `eval`.
- **Engines are composable, not bundled.** `from_config`'s hardcoded
  name→struct match is replaced by an `EngineRegistry` — a name→engine map
  populated by the host. `handl-runtime` names no engine crate. Adding an
  engine = register it; no `handl-runtime` edit. Multiple engines per stage
  coexist (tree-walk + bytecode both registered; pick per runtime).
- **Naming disambiguated.** `eval.eval` (callable) vs `lang.eval[node.kind]`
  (data) no longer share a symbol. Same for `compile.compile` vs any future
  data-namespace `lang.compile`.
- **Dependency direction preserved.** `plugin-lib-*` crates remain leaves
  depending on `handl-runtime` + `handl-language`; the cycle is broken by the
  trait pair, not by a leaf reaching up to engines.
- **`compile` / `run_compiled` are leaf-able.** The `CompiledHandle` +
  `CompiledArtifact` abstraction keeps the non-serializable artifact behind
  a trait object while both leaf crates work with serializable handles only.
- **`run` eliminated.** One fewer redundant primitive; composition at the
  handl level is the idiomatic path.
- **Kit-data decoupled from engine impls.** Today `handl-kit-data`,
  `handl-kit-data-canonical`, and `handl-kit-data-config` (pure data crates)
  depend on `handl-runtime` for `RuntimeConfig` — which transitively pulls
  all 5 engine crates (cranelift, bytecode, …). With the registry, kit-data
  depends on `handl-runtime` for `RuntimeConfig` (names only, serializable)
  and `handl-language` for fragment types — no engine crates at all. The
  registry is populated by the host, not by kit-data.
- **No native crate to maintain.** The "native" selection is just a default
  a host composes — not a crate that bundles engines and must be edited when
  engines change.
- **ADR-0030 superseded.** Its `ac-lib-lang` façade over a trait-based AST
  and Rust emitters is obsolete post-0044; this ADR is the honest
  replacement — granular crates, not a monolithic façade.

### Trade-offs

- **One real refactor.** Add `RuntimeFactory`/`RuntimeHandle`/
  `CompiledHandle`/`CompileEngine`/`CompiledArtifact`/`EngineRegistry` types
  to `handl-runtime`; delete `MetaHostFunctions` (its responsibilities split:
  host fns → `plugin-lib-*`, `COMPILED_REGISTRY` → `Runtime`, `self`
  injection → `RuntimeFactory::build`); delete the hardcoded `from_config`
  match (replaced by `from_registry` + registry lookup); drop all 5 engine
  impl deps from `handl-runtime`'s `Cargo.toml`; move `lex`/`parse`/`eval`/
  `compile`/`run_compiled` host fns into five new `plugin-lib-*` crates;
  generalize `COMPILED_REGISTRY` to `Box<dyn CompiledArtifact>`; update
  `kit-canonical`/`handl-defining-handl` to populate a registry and call
  `from_registry`. Not a file move.
- **Hosts must populate the registry.** Each engine crate still needs an
  explicit `register_*` call from the host (Rust has no self-registration).
  A host-side helper crate (not `handl-runtime`) may provide
  `EngineRegistry::with_defaults()` to reduce boilerplate — but
  `handl-runtime` itself names no engine crate.
- **Indirection cost.** Each meta call now goes through a trait object
  (`&dyn RuntimeFactory` → `Box<dyn RuntimeHandle>`). Negligible for the meta
  layer (it spins up a sub-runtime per call anyway), but real.
- **`COMPILED_REGISTRY` stays thread-local in `handl-runtime`.** The
  `CompiledHandle` is serializable but the artifact it references is not —
  handles don't survive across threads/processes. This is inherent to native
  code and unchanged from today.

## References

- [ADR-0030 — lib-lang](./0030-lib-lang.md) (superseded)
- [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
- [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
- `crates/handl-runtime/src/meta.rs` — current `MetaHostFunctions` (to be split)
- `crates/plugin-lib-json` / `crates/plugin-lib-toml` — the convention this
  ADR extends to the meta functions
