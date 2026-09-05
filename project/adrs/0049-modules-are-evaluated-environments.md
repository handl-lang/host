# ADR-0049: Modules are Evaluated Environments

## Status

Proposed

Supersedes [ADR-0048 — Modules as First-Class Values](./0048-modules-as-first-class-values.md),
which over-engineered the problem by re-inventing environment semantics as
a separate subsystem. Builds on [ADR-0013 — Standard Library](./0013-standard-library.md),
[ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md),
and [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md).

## Context

ADR-0048 proposed making `Module` a first-class value type with a
`CLOSURE_REGISTRY`, `ValueModule`/`HandlModuleLookup` adapters, and a
`plugin-lib-module` leaf crate — mirroring ADR-0041's treatment of
`Language`. On review, this re-invents machinery the language already has.

### The reduction

A module is a **named bag of bindings produced by evaluating code**. There
is no separate "module" concept; there is only:

- **evaluation**, which already produces bindings (`let`/`var`/`stmt.let`/
  `stmt.const` are data-driven `Scheme::Bind` in `eval.yaml`), and
- **import resolution**, which maps a module name to a source, evaluates
  it once, and binds the resulting environment under that name.

Concretely:

- `import "math"` = evaluate `math.hnl`, cache its top-level environment,
  bind it to `math`.
- `math.add(x, y)` = field access on the `math` environment value — the
  existing `field-access` construct, not a new one.
- `export foo = ...` = sugar declaring which top-level bindings the
  module's environment exposes to importers.
- The "module system" = the eval engine's environment machinery + an
  import resolver that caches evaluated environments by name.

There is no `Module` value type, no `CLOSURE_REGISTRY`, no
`HandlModuleLookup`, no `plugin-lib-module` leaf. A module is an
environment, and an environment is what evaluation already produces.

### Why this dissolves ADR-0048

ADR-0048's machinery existed to make `Module` a _separate_ first-class
value alongside `Language`. But the substance of a module — a set of
named bindings — is already the substance of an environment, which
evaluation already produces. Building a parallel value system for it was
redundant. The closure-serialization "crux" ADR-0048 raised is not a
module problem at all: it is the general problem of first-class closures,
which is an environment-as-value concern, not a module concern. A
stdlib function like `math.add` is a `Scheme` combinator with parameters
bound at call time — no captured environment, no closure, no registry.

## Decision

Treat modules as evaluated environments. Do not introduce a `Module`
value type or a `modules` fragment set. The work is:

### 1. Make environments first-class values

An environment (the result of evaluating a scope) must be reifiable as a
handl value — an object/record mapping binding names to values — so that
`import "math"` can bind the _whole_ environment to `math` and
`math.add` can field-access into it. This is the one genuine language
addition, and it is an environment-as-value concern, not a module
concern. It likely subsumes or overlaps with first-class closures
(closures are environments + code), so the two should be designed
together.

### 2. Migrate `import`/`export`/`module`/`field-access` off `Foreign`

Today (`handl-kit-data-canonical/data/eval.yaml` lines 136–138, 226–250)
these four constructs dispatch via `Scheme::Foreign` to Rust host
functions (`field_access`, `import`, `export`, `module`). They must
become data-driven `Scheme` combinators (or a small set of well-defined
`Prim` ops) so the data-driven eval engines can run them — the same
unfinished ADR-0044 Phase 4–5 work that blocks the stdlib generally.
`field-access` becomes a `Prim` over an environment value; `import`
becomes a `Prim` or `Foreign` that invokes the import resolver and
caches; `export`/`module` become sugar over binding + environment
exposure.

This is the hard prerequisite. `let`/`var`/`Bind` are already
data-driven, so the environment substrate exists; what is missing is
exposing environments as values and the four constructs above.

### 3. Import resolver as the loader

The import resolver maps a module name to a source, evaluates it under
the current `Language` (via the existing `eval` pipeline), caches the
resulting environment by name, and returns it. This is the _loader_, not
a new language concept. It can be a Rust host function (`Foreign`
dispatch) or, once environments are values, a handl function — the same
bootstrap question as `handl-defining-handl`, with the same answer: a
minimal Rust loader boots, handl takes over once environments are
values.

The resolver is pluggable per [ADR-0028](./0028-pluggable-module-composition.md)
(virtual/HTTP/Git/fs resolvers) and [ADR-0035](./0035-loaders-as-plugins.md)
(`plugin-loader-*` as composable resolver plugins tried in order). An
import like `import "git:github.com/foo/bar@v1/math"` resolves in two
steps: the matching resolver plugin fetches source bytes (fs/http/git/
virtual), then the eval pipeline turns bytes into a cached environment.
Which resolvers are loaded is the import capability boundary
([ADR-0046](./0046-capability-discipline-at-the-language-level.md)) —
fs-only with an allowlist is a sandbox; http+git is not. No new
mechanism; the loaded resolver set _is_ the import capability.

### 4. Stdlib as `.hnl` files

The stdlib (ADR-0013's end state) becomes `.hnl` files evaluated into
environments, imported by name. Rust stays only for `Foreign`-dispatched
primitives the eval engine cannot express (crypto, perf-critical ops) —
the ADR-0013 fallback. No `Module` trait impls for stdlib modules; they
are just source files.

### 5. The module system in handl

Once environments are values and `import`/`field-access` are
data-driven, the module system — name → cached environment, qualified
lookup via field access — is expressible in handl. The Rust
`ModuleRegistry`/`ModuleLookup` (`plugin-module`) becomes the bootloader
for the import resolver, the way `handl-defining-handl` is the
bootloader for the language. This is the user's stated goal, realized
without a parallel value system.

### 6. Per-language module systems

Module semantics are **language data**, not host machinery. The eval
schemes for `import`/`export`/`field-access`/`module` live in the
`Language` value (ADR-0044). A new language defines its own schemes for
those node kinds and gets different module semantics for free:

- a different `import` scheme → lazy import, qualified import,
  renaming, side-effecting import, or a `require`-style construct with
  its own grammar row + eval scheme;
- a different `field-access` scheme → different environment-access
  semantics;
- an entirely new node kind (e.g. `require`) with its own grammar +
  eval scheme → a genuinely different module system.

The **fetch** stays host-level (resolver plugins are shared capability,
§3), but a language can call a different `Foreign` resolver function or
define its own fetch scheme if it wants a different source model.

Because `Language` is first-class (ADR-0041), a language can implement
its own module system **in itself**: the `import` eval scheme calls
`eval(self, parse(self, lex(self, resolver.fetch(name))))` — all
handl-level. The module system becomes self-hosting the way
`handl-defining-handl` made the language self-hosting. One-liner:
module semantics = language data (swappable per language); module fetch
= host capability (swappable per kit); both composable, neither
hardcoded.

## Consequences

### Positive

- **No new value type.** Modules are environments; environments are what
  evaluation already produces. No `Module` value, no `CLOSURE_REGISTRY`,
  no `ValueModule`/`HandlModuleLookup`, no `plugin-lib-module` leaf.
- **ADR-0013 end state reachable.** Stdlib as `.hnl` files, Rust as
  fallback — exactly as ADR-0013 specified, with no extra subsystem.
- **Module system in handl.** The registry/lookup logic is handl over
  environment values, backed by a thin Rust import resolver. The user's
  goal, without ADR-0048's machinery.
- **Closures and environments designed together.** The one genuine
  addition — environments as values — is co-designed with first-class
  closures, since they are the same problem. No separate closure
  registry bolted on.
- **Reuses existing constructs.** `math.add` is field access; `import`
  is evaluate-and-bind. No new syntax for what evaluation already does.
- **Per-language module systems.** Module semantics are language data,
  so a new language defines its own `import`/`field-access` schemes (or
  new node kinds like `require`) and gets different module semantics for
  free. A language can even implement its module system in itself via
  `eval(self, ...)` — self-hosting modules, the way
  `handl-defining-handl` made the language self-hosting.

### Trade-offs

- **Depends on eval-engine completeness.** `import`/`export`/`module`/
  `field-access` must migrate off `Foreign` (ADR-0044 Phase 4–5). This
  ADR does not do that work; it depends on it. A `math` subset whose
  functions are pure `Scheme` combinators can be written today; a
  `collections.map` that needs first-class function values and field
  access cannot.
- **Environments-as-value is a real language addition.** It is smaller
  than ADR-0048's subsystem but not free: it touches the eval engine's
  environment representation and likely the type system (an environment
  value's type is a record of its bindings). This is the actual work;
  everything else is sugar over it.
- **Closures still aren't serializable.** ADR-0048's crux doesn't vanish
  — it relocates. A closure (code + captured environment) is not
  `serde`-serializable in general. But this is now a first-class-closures
  concern, not a module concern: stdlib `Scheme`-tree functions don't
  need it, and a handl-authored closure passed to `collections.map` is
  held by the running eval engine, not stored in a module. If closures
  ever need to be stored in modules as values, the `COMPILED_REGISTRY`-
  style handle pattern from ADR-0047 §3 applies — but that is a future
  decision, not this ADR's foundation.
- **Import resolver bootstrap.** The loader is needed to load the
  loader. Solved as in `handl-defining-handl`: minimal Rust resolver
  boots, handl takes over. Real work, existing pattern.

### Neutral

- **The Rust `Module`/`Function`/`ModuleLookup` traits stay** as the
  seam for `Foreign`-dispatched primitives and the import resolver
  bootloader. They are not deleted; they are demoted from "the only way
  to be a module" to "the Rust-side interface for true primitives and
  bootstrapping." Mirrors how ADR-0044 kept engine _traits_ as Rust while
  making `Scheme` data.

## Implementation phases

### Phase 1 — Environments as values

Reify an evaluated scope's environment as a handl value (object/record of
bindings). Co-design with first-class closures, since closures are
environments + code. This is the one genuine language addition and the
actual work.

### Phase 2 — Migrate `field-access`/`import`/`export`/`module` off `Foreign`

Make the four constructs data-driven `Scheme` combinators or `Prim` ops
(ADR-0044 Phase 4–5). `field-access` operates on environment values;
`import` invokes the resolver and caches; `export`/`module` are sugar
over binding + exposure. Hard prerequisite for any non-trivial stdlib in
handl.

### Phase 3 — Import resolver

Map module name → source → evaluated, cached environment. Rust
implementation boots; handl takes over once environments are values.

### Phase 4 — One stdlib module in handl, as proof

A `math` subset (`add`/`mul`/`sub`/`div`) as a `.hnl` file, imported and
called via field access. Proves the ADR-0013 end-to-end path on a small
slice.

### Phase 5 — The module system in handl

Name → cached environment, qualified lookup via field access, merge
policy — all handl over environment values. Rust `ModuleRegistry`
becomes the bootloader. The user's stated goal, realized.

## References

- [ADR-0013 — Standard Library](./0013-standard-library.md) — the end
  state this ADR makes reachable
- [ADR-0028 — Pluggable Module Composition](./0028-pluggable-module-composition.md)
  — the virtual/HTTP/Git/fs resolver model the import loader reuses
- [ADR-0035 — Loaders as Abstract Plugins](./0035-loaders-as-plugins.md)
  — `plugin-loader-*` as composable resolver plugins tried in order
- [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
  — the first-class-value move this ADR avoids duplicating for modules
- [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
  — the data model; Phase 4–5 migration of `Foreign` constructs is the
  prerequisite
- [ADR-0047 — Meta Functions as `plugin-lib-*` Crates](./0047-meta-functions-as-plugin-lib-crates.md)
  — the `COMPILED_REGISTRY` handle pattern, relevant only if closures
  ever need to be stored as module values (a future decision)
- [ADR-0048 — Modules as First-Class Values](./0048-modules-as-first-class-values.md)
  — superseded; the over-engineered path this ADR replaces
- `crates/handl-kit-data-canonical/data/eval.yaml` lines 136–138, 226–250
  — `field-access`/`import`/`export`/`module` as `Scheme::Foreign` today
- `crates/handl-kit-data-canonical/data/eval.yaml` lines 184–225 —
  `let`/`var`/`stmt.let`/`stmt.const` as data-driven `Scheme::Bind`,
  the environment substrate that already exists
