# Custom Functions via YAML

## Context

The standard library (ADR-0013) and built-in functions (ADR-0018) provide a
curated set of utilities. But users often need domain-specific helpers that
are too project-specific to deserve a built-in: an internal URL shortener,
a company-specific ID generator, a wrapper around a proprietary API.

Adding a built-in requires a code change to a host implementation. The
community should be able to define domain-specific helpers without
publishing a new package or getting a change merged into a host.

## Decision

Support user-defined functions declared in `*.fn.yml` files.

### Function file format

```yaml
# helpers/short-id.fn.yml
name: short-id
params:
  - name: prefix
    default: "id"
body: concat(prefix, "-", random(1000, 9999))
```

- The file extension is `.fn.yml` or `.fn.yaml`.
- Function files may be placed in any directory; the extension is what
  matters, not a dedicated `functions/` folder.
- `params` declares the parameters the function accepts.
- `body` is an handl expression that can reference params, call other
  functions, and use standard library modules.

### Resolution

- The function resolver discovers `*.fn.yml` files in the document
  directory, then `libraries/functions/`, then configured paths.
- Functions are registered by name. Built-in functions take precedence; a
  custom function with the same name as a built-in is ignored and a warning
  is logged.
- Custom functions run under the same capability and resource limits as
  any other function call — the host enforces these.

### Example usage

```handl
short-id("task-")
```

Resolves to the `short-id.fn.yml` function, binds `prefix` to `"task-"`,
and evaluates `concat("task-", "-", random(1000, 9999))`.

### Promotion path

A custom function can be promoted to a built-in when it is:

- Used in a performance-critical path.
- Adopted widely across projects.
- Proven safe and stable over time.

Built-ins emerge from the community; they start as declarative functions
and are rewritten as native host implementations when the cost of
interpretation matters.

## Consequences

- Users can add functions without a package or a merge to a host
  implementation.
- A healthy ecosystem of small, shareable functions can develop in any
  project directory and later be upstreamed.
- Custom functions are slower than built-ins because they go through the
  expression evaluator.
- The resolver needs a function registry, argument binding, and
  capability enforcement.
- Without governance, duplicate or unsafe functions can proliferate. The
  promotion path and a clear deprecation policy are required.

## Status

Proposed
