# handl Standard Library

## Context

handl hosts provide modules — sets of functions that programs can call.
The reference runtime provides a set of pure, cross-target modules that
work on every host: math, text, regex, collections, datetime, encoding.
These form the handl standard library.

The standard library is distinct from host-specific libraries. A host like
Peregrin may provide additional modules (filesystem, process, HTTP) that
only work under that host. The standard library is the common denominator
that makes handl programs portable across hosts.

## Decision

The handl standard library is a set of pure, side-effect-free modules
implemented as handl library crates. Every host must provide them (or
equivalent implementations) so that programs using only stdlib modules are
portable.

### Module taxonomy

| Module        | Purpose                 | Examples                                                              |
| ------------- | ----------------------- | --------------------------------------------------------------------- |
| `math`        | Numeric operations      | `add`, `mul`, `sub`, `div`, `mod`, `floor`, `ceil`, `round`           |
| `text`        | String operations       | `concat`, `len`, `split`, `join`, `replace`, `trim`, `upper`, `lower` |
| `regex`       | Pattern matching        | `match`, `replace`, `capture`                                         |
| `collections` | Array/object operations | `map`, `filter`, `reduce`, `sort`, `keys`, `values`, `entries`        |
| `datetime`    | Date/time operations    | `now`, `format`, `parse`, `add`, `diff`                               |
| `encoding`    | Format conversion       | `base64_encode`, `base64_decode`, `json_parse`, `json_stringify`      |
| `fp`          | Functional combinators  | `pipe`, `compose`, `default`, `coalesce`                              |

### Design principles

1. **Pure.** No I/O, no side effects, no non-determinism. `datetime.now`
   is the one exception — it is non-deterministic and marked as such.
2. **Cross-target.** Every module works on every host. No filesystem,
   network, or process access.
3. **Composable.** Functions are small and composable. Complex operations
   are built from primitives, not monolithic APIs.
4. **Host-agnostic.** The standard library does not know which host it runs
   under. It uses only the `Host` trait's function dispatch.
5. **Extensible.** New modules can be added without changing the language
   kernel. A host can override a stdlib module with an optimized
   implementation.

### Namespace convention

Standard library modules are accessed via their module name as a namespace:

```handl
math.add(1, 2)
text.concat("hello", " world")
collections.map(items, fn(x) => x * 2)
```

The default namespace (for shorthand resolution) is a runtime host decision.
The reference runtime provides `math`, `text`, `collections`, etc. as
built-in namespaces. Hosts may add or replace namespaces.

### Relationship to hosts

- **DefaultHost** provides stdlib built-ins only (`add`, `mul`, `concat`,
  `len`, etc.) — a subset sufficient for the VM's internal operations.
- **SandboxHost** provides the full stdlib — all pure modules.
- **NativeHost** provides the full stdlib + platform modules (filesystem,
  network, process).
- **PeregrinHost** provides the full stdlib + orchestration modules
  (workflow control, event dispatch, state management).

Hosts that omit a stdlib module cause import errors for programs that use
it. This is by design — capability is emergent from what the host provides.

### File layout

Standard library implementations live in `libraries/std/` as handl
library crates:

```
libraries/std/
  math/
  text/
  regex/
  collections/
  datetime/
  encoding/
  fp/
```

Each module is a directory containing `.handl` or `.yaml` library files with
function declarations. Hosts may provide native (Rust) implementations of
these modules for performance; the handl implementations serve as the
specification and fallback.

## Consequences

- Programs using only stdlib modules are portable across all hosts that
  provide the full stdlib.
- The standard library is the baseline for cross-target compatibility.
- Host-specific libraries (e.g. Peregrin's orchestration modules) are clearly
  separated from the portable stdlib.
- New stdlib modules can be added without kernel changes.
- Hosts can optimize by providing native implementations of stdlib modules.

## Status

Proposed
