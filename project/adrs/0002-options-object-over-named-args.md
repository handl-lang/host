# Options Object Over Named Arguments

## Context

handl function calls use positional arguments exclusively:

```
fn(arg1, arg2, arg3)
```

This is sufficient for most module functions (`math.add(1, 2)`,
`path.join("a", "b")`). However, some functions — notably `http.request` —
need optional parameters (headers, timeout, body) that don't map cleanly
to fixed positions. The question is how to handle optional/named
parameters without complicating the language.

Two options were considered:

### Option A: Language-level named arguments

```handl
http.request(method: "GET", url: "https://example.com", headers: {...})
```

This would require changes across multiple crates:

- **Parser** — new `name: expr` syntactic form in argument position
- **Type checker** — signatures must declare param names; inference must
  validate them
- **Dialect** — the canonical dialect spec gains a new argument form
- **Typegen** — function signatures must carry param name metadata
- **Runtime/VM** — `Function::call` receives a flat `&[NonFunctionValue]`;
  would need a new calling convention or a name-to-index map
- **Frontend** — JSON deserialization must handle named args

The grammar is currently minimal: arguments are comma-separated
expressions. Named args introduce a new syntactic category, a new
type-checking dimension, and dialect complexity — all for the benefit of
skipping `null` in a handful of functions.

### Option B: Options object at the module adapter level

```handl
http.request("GET", "https://example.com", {body: "...", headers: {...}})
```

The third argument is a JSON object. The module adapter inspects it and
extracts known keys (`body`, `headers`, `timeout`). No parser change, no
type system change, no dialect change. `NonFunctionValue` already wraps JSON
objects — this is existing infrastructure.

This is the same pattern the Fetch API uses: `fetch(url, { method,
headers, body })`.

## Decision

**Use Option B: options objects at the module adapter level.**

handl will not add named arguments to the language grammar. Module
functions that need optional parameters will accept a JSON object
argument whose keys are interpreted by the module adapter.

Conventions for options objects:

- The options argument is always the **last positional argument**.
- Missing keys are treated as absent (not errors).
- `null` or an empty object `{}` is equivalent to omitting the argument.
- Verb shorthands (`http.get`, `http.post`, etc.) remain positional for
  the common case. The options object is only needed on the escape-hatch
  function (`http.request`) or when passing headers to a verb shorthand.

## Consequences

**Positive:**

- The language grammar stays minimal — no new syntactic forms.
- No changes to parser, type checker, dialect, typegen, or VM calling
  convention.
- Module adapters have full control over how options are interpreted.
- Pattern is reusable: any module can adopt it without language changes.
- Self-documenting in practice: `{headers: {...}, timeout: 5000}` is
  readable without knowing parameter order.

**Negative:**

- No compile-time validation that option keys are valid (a typo like
  `{heder: "..."}` silently ignores the key). This is acceptable — module
  adapters can warn on unknown keys at runtime if desired.
- No positional shorthand for "skip this optional" — you must pass `null`
  or `{}`. The verb shorthands mitigate this for common cases.
- Options objects are not type-inferred — the type checker sees them as
  generic objects, not typed records. This is consistent with how handl
  handles all JSON values.

**Non-goals:**

- This decision does not preclude adding named args to the language in
  the future if a compelling need arises beyond module functions.
- This decision applies to the canonical handl dialect. Custom dialects are
  free to experiment with named args at their own level.

## Status

Proposed
