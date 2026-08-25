# Standard Library Functions

## Context

handl expressions support function calls (`fn(arg1, arg2)`). The standard
library (ADR-0013) provides pure modules like `math`, `text`, `collections`.
But some utility functions are needed so frequently that requiring a module
prefix is ergonomic overhead:

- `uuid()` — a v4 UUID.
- `random(min, max)` — a random number.
- `now()` — the current timestamp.
- `env(name, default)` — an environment variable with a default.

These are not language primitives — they are library functions. But they
are common enough that the language toolchain should define a standard set
and a standard way to expose them.

The key constraint: the language must not become an arbitrary code
execution channel. Any function support must be explicit, whitelisted, and
auditable. Functions that access the filesystem, run shell commands, or
read the environment require host capability grants.

## Decision

handl defines a curated set of standard library functions available as
unprefixed calls when the host provides them.

### Function categories

**Pure (available on all hosts):**

- `random(min, max)` — random number in range. Non-deterministic.
- `uuid()` — v4 UUID. Non-deterministic.
- `len(x)` — length of array or string.
- `keys(obj)` — keys of an object.
- `values(obj)` — values of an object.

**Host-dependent (require capability):**

- `now()` / `now.iso()` / `now.unix()` — current time. Non-deterministic.
  Available on hosts that provide a time module.
- `env(name, default)` — environment variable. Available on hosts that
  provide an environment module.
- `file(path)` — read a file. Available on hosts that provide a filesystem
  module.
- `shell(command)` — run a shell command, capture stdout. Available on hosts
  that provide a process module.

### Resolution

- The host registers functions in its function table.
- Unprefixed calls resolve against the host's function table first, then
  against module-qualified names (`math.add`, `text.concat`).
- A function is only resolved if its name is in the host's registered set.
- Hosts that don't provide a capability (e.g. SandboxHost) simply don't
  register those functions — calls to them fail with "unknown function".

### Determinism tracking

The resolver tracks whether a value is deterministic. If any
non-deterministic function is used, the document is marked as
non-replayable. This is a host concern — the language itself does not
enforce replayability, but provides the metadata for hosts to use.

### Promotion path

Functions start as library functions (implemented in handl or as host
adapters). When a function is:

- Used in a performance-critical path.
- Adopted widely across projects.
- Proven safe and stable over time.

…it can be promoted to a built-in that the host implements natively (in
Rust). This is the same promotion path as custom fields (ADR-0016).

## Consequences

- Expressions can use common utilities without module prefix overhead.
- The host controls which functions are available — capability is emergent.
- Pure functions work everywhere; host-dependent functions work only where
  the host provides the capability.
- Non-deterministic functions are tracked for replayability.
- The standard function set can grow without language kernel changes.

## Status

Proposed
