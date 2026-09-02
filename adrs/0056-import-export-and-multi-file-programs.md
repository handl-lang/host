# ADR-0056: Import / Export and Multi-File Programs as Evaluated Environments

## Status

Proposed

Builds on [ADR-0049 — Modules are Evaluated Environments](./0049-modules-are-evaluated-environments.md),
[ADR-0028 — Pluggable Module Composition](./0028-pluggable-module-composition.md),
[ADR-0035 — Loaders as Plugins](./0035-loaders-as-plugins.md),
[ADR-0046 — Capability Discipline at the Language Level](./0046-capability-discipline-at-the-language-level.md),
and [ADR-0013 — Standard Library](./0013-standard-library.md).

## Context

A handl program should be able to span multiple source files and import
other modules without inventing a second runtime concept for modules.

The current state is still shaped by host-only, ad hoc operations:

- `import`
- `export`
- `module`
- `field-access`

These are partly implemented via `Foreign` host functions and partly as
special forms. That creates three problems:

1. **The semantics are not data-driven.** The language cannot express the
   import system as ordinary evaluation rules, which blocks the eval engine
   from being fully data-driven.
2. **The module concept is split across runtime and language.** We are
   treating modules as a thing distinct from the environment produced by
   evaluation, instead of as the environment itself.
3. **Multi-file programs are impossible or brittle.** A program cannot
   cleanly be split into `.hnl` files and imported by name, and the stdlib
   cannot become source-based modules in the intended long-term architecture.

The design already points to the correct direction. ADR-0049 reduces the
problem to a single crucial insight: a module is an evaluated environment,
not a distinct value type. The remaining work is to express that in the
language semantics, the import resolver, and the runtime loader model.

## Decision

Implement multi-file programs as evaluated environments, with import/export
as ordinary language-level semantics rather than as a bespoke module system.

### 1. A module is an environment value

A module is the result of evaluating a source file or a source bundle under
some current `Language`.

That result is an environment: a binding table mapping names to values.
The module namespace is therefore the ordinary binding namespace of the
environment, not a separate runtime object graph.

This means:

- `import "math"` resolves a source file, evaluates it, and binds the
  resulting environment to `math`
- `math.add` is ordinary field access on an environment value
- `math` is just a value whose fields are the exported bindings
- `module` is a convenience form or sugar, not a distinct runtime concept

No `Module` value type is introduced as part of the language runtime.
No separate module registry sits above evaluation. The environment already
contains the data we need.

### 2. `import` resolves source and caches the resulting environment

Import is a two-step operation:

1. resolve a module name or module locator to source bytes
2. evaluate that source under an effective module `Language` and cache the
   resulting environment

Import syntax must support the following core forms:

```handl
import "math"
import "math" with { kits: ["std"], lang: "handl" }
import math as m
import "math" as m
import "math" lazy
from "math" import *
from "math" import add, mul as multiply
```

The `with { ... }` form overrides the module-local language configuration for
that import only. This is not a new module concept; it is a request to the
resolver/evaluator to instantiate the imported source under a specific
`Language` configuration. A module may thus run with a different `Language`,
with different kits, or with a sandbox-specific policy while still being
represented as an environment value.

The resolver is pluggable and conforms to the loader/plugin architecture:

- file system resolver
- HTTP resolver
- git resolver
- virtual resolver
- custom host-provided resolvers

The import operation may look like this conceptually:

```handl
import "math"
math.add(2, 3)
```

Where the runtime does roughly:

```text
resolve("math") -> source path / bytes
source -> eval(lang, source) -> Environment
cache["math"] = environment
bind "math" = environment
```

A second import of the same module returns the cached environment instead of
re-evaluating the source, unless invalidation rules say otherwise.

The lazy form is useful but is not a module-system requirement in itself. It is
best treated as a general runtime/lookup optimization or a language-level
feature for delayed binding, not as the core semantics of `import`.

### 3. `export` defines the public surface of a module

`export` is the declaration of which top-level bindings are made visible to
importers.

The syntax must support both declaration forms:

```handl
let add = ...
let sub = ...
export add
export sub
```

and assignment-style export:

```handl
export two = 2
export add = fn (x, y) => x + y
```

and wildcard export:

```handl
export *
```

and re-export from another module:

```handl
from "math" export *
from "math" export add
from "math" export add, mul as multiply

export * from "math"
export add from "math"

export "math"
```

Re-export is sugar for: import the target module, then expose the selected
names or the entire module into the current module's public environment.

The module's exported environment is the set of visible bindings.
Importers see only those names.

This preserves a clean separation between:

- implementation details internal to the module
- the public surface used by other files

The semantics of `export` are therefore not an extra module system, but an
environment visibility mechanism layered on top of ordinary bindings.

### 4. `field-access` is the module access mechanism

Modules are accessed by ordinary field lookup on the bound environment value.
This keeps the mechanism consistent with the rest of the language:

```handl
import "math"
math.add(2, 3)
```

The runtime performs an environment lookup as an object/record access, not a
special-case module dispatch path.

This is the crucial design simplification: modules do not need a bespoke
lookup layer if they are already represented as environments.

### 5. Import capability is a policy boundary, not a language concept

The set of available import resolvers is part of the host environment and is
subject to capability discipline.

This matches ADR-0046 and ADR-0035:

- a host may allow fs-only resolution
- a host may deny HTTP or git loaders
- a sandboxed runtime may permit only a known set of module roots

The import capability boundary is therefore the allowed resolver set, not a
new language feature required to understand module semantics.

### 6. Stdlib becomes `.hnl` files evaluated into environments

ADR-0013's end state remains valid: the standard library is a collection of
`.hnl` source files imported by name.

The implementation is straightforward under this model:

- each stdlib file is a source module
- evaluating it creates an environment
- the resolver loads the source by module name
- exported bindings become available to importers

No Rust `Module` trait implementations are required for stdlib modules.
They are just source files whose resulting environments are cached and bound.

### 7. The language runtime must expose environments as values

The one genuine language change is not “module support”; it is the ability to
reify environments as handl values.

Without that, imports can evaluate source but cannot deliver a meaningful,
usable environment object. Once environments are first-class values, the rest
of the design follows naturally:

- imported modules are values that can be named, stored, passed, and inspected
- module-local `Language` configuration is a runtime policy choice, not a
  special module metaphysics
- a sandboxed sub-runtime can import and evaluate modules under a different
  allowed loader and kit set without granting access to the parent runtime

- module names bind to environment values
- field access works on them
- export is a visibility policy on the environment
- import is cache + loader + evaluation

This is the architectural difference between a malformed module system and a
working environment-based module model.

## Behavior examples

The following examples illustrate the intended runtime behavior and are the
main review target for the design.

### Example 1: simple module import

Filesystem layout:

```text
project/
  main.hnl
  math.hnl
```

`math.hnl`:

```handl
let add = fn (x, y) => x + y
let mul = fn (x, y) => x * y

export add
export mul
```

`main.hnl`:

```handl
import "math"

let result = math.add(2, 3)
print(result)
```

Behavior:

1. resolver resolves `"math"` to `project/math.hnl`
2. evaluator runs `math.hnl` and produces an environment
3. the environment is cached under the module name `math`
4. `math.add` resolves as field access into that environment
5. only `add` and `mul` are visible to importers because they were exported

The end state is equivalent to:

```handl
let math = {
  add: fn (x, y) => x + y,
  mul: fn (x, y) => x * y,
}

let result = math.add(2, 3)
print(result)
```

The difference is that the module came from a source file and was imported,
not constructed manually in memory.

### Example 2: private bindings stay private

```handl
let secret = 42
let public_value = secret * 2

export public_value
```

Behavior:

- `secret` is bound in the module environment
- `secret` is not exposed to importers
- `public_value` is visible through the imported module namespace

So:

```handl
import "settings"
print(settings.secret)  // error: not exported
print(settings.public_value) // OK
```

### Example 3: import is cached

```handl
import "math"
import "math"
```

Behavior:

- the first import resolves source and evaluates it
- the second import reuses the cached environment
- the module is not re-evaluated unless explicitly invalidated or reloaded

This makes repeated imports idempotent and keeps module evaluation single-
shot for a given module identity.

### Example 4: nested imports

```text
project/
  app.hnl
  util/math.hnl
```

`app.hnl`:

```handl
import "util/math"

let x = util.math.add(10, 20)
print(x)
```

Behavior:

1. `util/math` resolves via the configured loader strategy
2. `util/math.hnl` evaluates to an environment
3. the environment is cached under `util/math`
4. `util.math.add` resolves by field lookup into that environment

This makes nested programs or library folders behave as ordinary importable
module namespaces, not as a specially-encoded runtime graph.

### Example 5: capability boundary

A host may register only a filesystem resolver rooted at `./stdlib`.

Behavior:

- `import "math"` is allowed if `math.hnl` exists under the allowed root
- `import "https://example.com/math.hnl"` is rejected because the HTTP
  resolver is not available
- the language semantics do not change; the host’s allowed loader set does

This makes import policy a host capability decision, not a new language-level
feature.

## Consequences

### Positive

- Multi-file `.hnl` programs work naturally.
- The module story is simpler and easier to reason about.
- There is no duplicated module runtime model beside the environment.
- Import resolution is extensible via loaders/plugins.
- Capability discipline applies cleanly to resolver availability.
- The stdlib can be split into `.hnl` files without special-case Rust
  module wiring.
- The evaluation engine remains closer to ordinary language semantics rather
  than a bespoke module subsystem.

### Negative / trade-offs

- The language must support environment values as first-class data.
- `import`/`export`/`field-access` must be moved off ad hoc `Foreign`
  plumbing and onto data-driven evaluation rules or a small set of clear
  `Prim` operations.
- The import resolver is a language/runtime boundary concern and must be
  carefully bootstrapped.
- The earliest stages of bootstrap may still require a minimal Rust loader
  before the handl-defined import system is available.

### Compatibility

This direction supersedes the earlier proposal in ADR-0048, which treated
`Module` as a first-class type and introduced extra runtime machinery that
duplicated the existing environment model. The current design keeps the
runtime surface smaller and aligns module semantics with evaluation.

## Rationale

The problem is not that the language lacks a `Module` abstraction. The problem
is that multi-file programs require:

- source resolution,
- evaluation,
- environment caching,
- binding visibility,
- ordinary access semantics.

The language already has most of these primitives in some form. The mistake in
ADR-0048 was adding a parallel runtime concept for modules instead of using the
underlying environment model that evaluation already provides.

The correct abstraction is therefore:

- environment = the value produced by evaluation
- import = evaluate source and bind environment by name
- export = choose which bindings are visible
- field access = object semantics on the bound environment
- resolver = plugin-based loader policy

This gives us a module system without inventing a second module universe.

## Implementation notes

### Phase 1 — environment values

Make environments reifiable as handl values and ensure they can be stored,
bound, and field-accessed.

### Phase 2 — import resolution

Add a loader API and a cache keyed by resolved module name. Evaluate the
source once and bind the environment to the imported name.

### Phase 3 — export visibility

Define the exported binding set and ensure importers only see the public
surface of a module.

### Phase 4 — plugin resolvers and capability checks

Wire loader plugins into the runtime so the host can grant or deny access to
filesystem, HTTP, git, or virtual stores.

### Phase 5 — stdlib and multi-file programs

Move the stdlib and example programs to `.hnl` files and import them by
module name to validate the design end-to-end.

## Related ADRs

- [ADR-0048 — Modules as First-Class Values](./0048-modules-as-first-class-values.md)
- [ADR-0049 — Modules are Evaluated Environments](./0049-modules-are-evaluated-environments.md)
- [ADR-0028 — Pluggable Module Composition](./0028-pluggable-module-composition.md)
- [ADR-0035 — Loaders as Plugins](./0035-loaders-as-plugins.md)
- [ADR-0046 — Capability Discipline at the Language Level](./0046-capability-discipline-at-the-language-level.md)
- [ADR-0013 — Standard Library](./0013-standard-library.md)

## Summary

We do not need a new module runtime object. We need a module system built on
ordinary evaluation, ordinary environments, and ordinary field access. The
language should support import/export as environment semantics, the runtime
should resolve modules through loader plugins, and the stdlib should be
organized as importable `.hnl` modules.
