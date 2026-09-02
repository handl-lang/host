# handl Frontend: Custom `.handl` Syntax

## Context

handl documents are currently written in YAML (or JSON, TOML). The
`Frontend` trait (`handl-runtime::Frontend`) abstracts format parsing —
each frontend parses text into `NonFunctionValue` and the downstream
pipeline (field extraction, type checking, typegen, VM) operates on the
value, unaware of the source format.

YAML works but is verbose for handl's use case. The verbosity comes from
JSON Schema boilerplate stacked with handl special keys:

```yaml
fields:
  user:
    type: object
    expr: 'http.get("https://api.example.com/users/1")'
  is_ok:
    type: boolean
    expr: 'http.get("https://api.example.com/health").ok'
  greet:
    arg: true
    type: string
    expr: 'concat("hello, ", name)'
functions:
  add:
    params:
      a:
        type: number
      b:
        type: number
    return:
      type: number
    body: 'a + b'
auth_headers:
  Authorization: "Bearer ${env.TOKEN}"
  Accept: "application/json"
```

A 15-field document is ~80 lines in YAML. Most of the volume is
repeated `type:`, `expr:`, `arg:`, and `params:` keys that the parser
could infer from a more compact syntax.

### Design goals

1. **Less verbose** — computed fields in one line, not three.
2. **Type inference** — omit `type:` when the type is obvious from
   context (expression result, `{}` body, `[]` suffix).
3. **Familiar syntax** — TypeScript-style type annotations (`: type`),
   arrow functions (`=>`), brace objects (`{}`).
4. **Zero downstream changes** — the frontend produces the same
   `NonFunctionValue` structure the pipeline already consumes.
5. **No `fields:` section** — the syntax form itself distinguishes field
   declarations from data.

## Decision

**Implement a new frontend crate `handl-frontend-handl` that parses a
purpose-built `.handl` syntax and implements the `Frontend` trait.**

The frontend compiles `.handl` syntax into the same `serde_json::Value`
structure the YAML frontend produces. No changes to field extraction,
type checking, typegen, VM, or any downstream crate.

### Syntax


#### Comments

```handl
// Line comment
user => http.get("https://api.example.com/users/1")

/* Block comment
   spanning multiple lines */
is_ok: boolean => http.get("https://api.example.com/health").ok

tags: string[]  // inline comment
```
#### Operators

| Operator | Meaning | Maps to |
|---|---|---|
| `=>` | Computed field (expression) | `expr: "..."` |
| `=` | Literal data assignment | raw value at top level |
| `:` | Type annotation | `type: "..."` |

#### Field declarations

```
// Computed, type inferred
user => http.get("https://api.example.com/users/1")

// Computed, type explicit
is_ok: boolean => http.get("https://api.example.com/health").ok

// Type-only (no expr, no braces)
api_key: string

// Type with array shorthand
tags: string[]

// Schema via braces — {} is always object, no "object" keyword needed
user_full: {
  id: string
  name: string
} => http.get("https://api.example.com/users/1")

// Schema only (no expr)
user_full: {
  id: string
  name: string
}

// Array of objects with schema
users: {
  id: string
  name: string
}[]
```

#### Parameterized fields and functions

```
// Parameterized computed field (untyped arg)
greet(name) => concat("hello, ", name)

// Parameterized computed field (typed arg + return)
greet_typed(name: string): string => concat("hello, ", name)

// Function with typed params and return
add(a: number, b: number): number => a + b


#### Generics (type parameters)

Type parameters on functions and fields, using angle brackets (TS-style).
Bare type parameters only — no constraints in v1.

```handl
// Generic function
map<T, U>(items: T[], transform: (T) => U): U[] => reduce(items, transform, [])

// Generic parameterized field
first<T>(arg: T): T => arg

// Generic type-only field
items<T>: T[]

// Multiple type parameters
pair<T, U>(first: T, second: U): { first: T, second: U } => ...
```

| `.handl` syntax | Output in `NonFunctionValue` |
|---|---|
| `map<T, U>(items: T[], ...): U[] => ...` | `functions: {map: {generic: ["T", "U"], params: {...}, return: {...}, body: "..."}}` |
| `first<T>(arg: T): T => arg` | `fields: {first: {generic: ["T"], arg: {type: "T"}, type: "T", expr: "arg"}}` |
| `items<T>: T[]` | `fields: {items: {generic: ["T"], type: "array", items: {type: "T"}}}` |

The `generic:` key is a new special key in the canonical dialect. The
typechecker treats type parameter names as `Type::Unknown` (accepts
anything). No constraints in v1 — `T: comparable` syntax is reserved for
future use.

#### Enum shorthand

Union of literal values with `|` — type inferred from first value.

```handl
// String enum
role: "admin" | "user" | "guest"

// Number enum
priority: 1 | 2 | 3

// Boolean enum
active: true | false

// With computed
status: "active" | "inactive" | "pending" => http.get("...").status

// With modifiers
inherit role: "admin" | "user" | "guest"
lock priority: 1 | 2 | 3
```

| `.handl` syntax | Output in `NonFunctionValue` |
|---|---|
| `role: "admin" \| "user" \| "guest"` | `fields: {role: {type: "string", enum: ["admin", "user", "guest"]}}` |
| `priority: 1 \| 2 \| 3` | `fields: {priority: {type: "number", enum: [1, 2, 3]}}` |
| `status: "a" \| "b" => http.get("...")` | `fields: {status: {type: "string", enum: ["a", "b"], expr: "http.get(...)"}}` |

The frontend infers `type` from the first enum value. The `|` appears
only in type position (after `:`), so there is no ambiguity with
expression syntax.

#### Pipe and composition operators

Three operators for expression-level flow control:

| Operator | Name | Direction | Returns | Desugars to |
|---|---|---|---|---|
| `\|>` | Pipe | Left-to-right | Value (immediate) | Nested calls: `x \|> f(a)` → `f(a, x)` |
| `>>` | Flow | Left-to-right | Function (deferred) | `fp.flow(f, g)` |
| `<<` | Compose | Right-to-left | Function (deferred) | `fp.compose(f, g)` |

```handl
// |> pipe — immediate, left-to-right
// data flows through functions, value as last arg
names => users
  |> filter((u) => u.active)
  |> map((u) => u.name)
  |> sort(by_priority)
// desugars: sort(by_priority, map(fn, filter(fn2, users)))

// >> flow — deferred, left-to-right (reusable pipeline)
getActiveNames => filter((u) => u.active) >> map((u) => u.name) >> sort(by_priority)
// desugars: fp.flow(filter_fn, map_fn, sort_fn)
active => apply(getActiveNames, users)

// << compose — deferred, right-to-left (math convention)
getActiveNames => sort(by_priority) << map((u) => u.name) << filter((u) => u.active)
// desugars: fp.compose(sort_fn, map_fn, filter_fn)
active => apply(getActiveNames, users)

// All three produce the same result: sort(filter(map(users)))
```

**Desugaring rules:**

- `x |> f(args...)` → `f(args..., x)` — pipe inserts value as last argument
- `x |> f(a) |> g(b)` → `g(b, f(a, x))` — chains nest inside-out
- `f >> g >> h` → `fp.flow(f, g, h)` — returns a function
- `f << g << h` → `fp.compose(f, g, h)` — returns a function

**Runtime impact:**

- `|>` is **zero-cost** — pure frontend desugaring to nested calls, no
  new runtime function needed.
- `>>` and `<<` require `fp.flow` and `fp.compose` to be added to
  `handl-lib-utils`. These functions accept `FunctionValue` arguments and
  return a new `FunctionValue` that chains them.

**No downstream changes to expression parser or typechecker** — the
frontend desugars all three operators into existing expression forms
(nested function calls and `fp.flow`/`fp.compose` calls) before emitting
the `expr:` string.

#### Null coalescing and optional chaining

Two operators for safe null handling, both pure frontend desugaring:

| Operator | Name | Example | Desugars to | Runtime change? |
|---|---|---|---|---|
| `??` | Null coalesce | `a ?? b` | `fp.default(a, b)` / `fp.coalesce(...)` | No |
| `?.` | Optional chain | `user?.address?.city` | `fp.default(fp.default(user, {}).address, {}).city` | No |

```handl
// ?? null coalescing
name => user.name ?? "anonymous"
timeout => config.timeout ?? 3000
value => primary ?? fallback ?? "default"
// chains desugar to fp.coalesce(primary, fallback, "default")

// ?. optional chaining
city => user?.address?.city
port => config?.server?.port ?? 8080
email => user?.contact?.email ?? "no-email"

// Combined with pipe operators
result => users
  |> filter(active)
  |> map((u) => u?.name ?? "unknown")
  |> sort(by_priority)
```

**Desugaring rules:**

- `a ?? b` → `fp.default(a, b)`
- `a ?? b ?? c` → `fp.coalesce(a, b, c)`
- `a?.path` → `fp.default(a, {}).path` (access on empty object returns null)
- `a?.b?.c` → `fp.default(fp.default(a, {}).b, {}).c`

Both operators are **pure frontend desugaring** — no changes to the
expression parser, typechecker, or runtime. The frontend emits the
desugared `fp.default`/`fp.coalesce` calls in the `expr:` string.

#### Data (literal values)

```
// Object data
auth_headers = {
  Authorization: "Bearer ${env.TOKEN}"
  Accept: "application/json"
}

// Scalar data
payload_json = '{"name":"alice","email":"alice@example.com"}'
```

#### Modifiers

Three field-level modifiers, as keyword prefixes:

- `inherit` — value flows down to nested scopes during scoped
  interpolation (`inheritable: true` in YAML).
- `lock` — value cannot be overridden in nested scopes
  (`mutable: false` in YAML).
- `spread` — field's object value is spread into parent, field
  is removed (`merge: true` in YAML).

```handl
// Single modifier
inherit api_key: string
lock version: number
spread defaults => http.get("https://api.example.com/defaults")

// With type + computed
inherit timeout: number => 3000
lock name: string => "handl"

// With schema
inherit config: {
  host: string
  port: number
}
spread base: {
  host: string
  port: number
}

// Combined modifiers
inherit lock api_key: string
spread base: { host: string }
```

### How the parser distinguishes fields from data

No `fields:` section is needed. The syntax form is the discriminator:

| Syntax form | Classification | Goes into |
|---|---|---|
| `key => expr` | Computed field | `fields` |
| `key: type => expr` | Typed computed field | `fields` |
| `key: type` | Type-only field | `fields` |
| `key: { ... }` | Field with schema | `fields` |
| `key(params) => expr` | Parameterized field | `fields` |
| `key = value` | Literal data | top-level data |
| `key: value` (inside `{}`) | Object property | part of parent |

The frontend groups all field declarations under the `fields` key and
all `fn` declarations under the `functions` key in the output
`NonFunctionValue`. Data stays at top level.

### Output mapping

| `.handl` syntax | Output in `NonFunctionValue` |
|---|---|
| `user => http.get(...)` | `fields: {user: {expr: "http.get(...)"}}` |
| `is_ok: boolean => ...` | `fields: {is_ok: {type: "boolean", expr: "..."}}` |
| `api_key: string` | `fields: {api_key: {type: "string"}}` |
| `tags: string[]` | `fields: {tags: {type: "array", items: {type: "string"}}}` |
| `greet(name) => ...` | `fields: {greet: {arg: true, expr: "..."}}` |
| `greet_typed(name: string): string => ...` | `fields: {greet_typed: {arg: {type: "string"}, type: "string", expr: "..."}}` |
| `add(a: number, b: number): number => a + b` | `functions: {add: {params: {a: {type: "number"}, b: {type: "number"}}, return: {type: "number"}, body: "a + b"}}` |
| `user_full: { id: string } => ...` | `fields: {user_full: {properties: {id: {type: "string"}}, expr: "..."}}` |
| `users: { id: string }[]` | `fields: {users: {type: "array", items: {properties: {id: {type: "string"}}}}}` |
| `auth_headers = { ... }` | `auth_headers: { ... }` (top-level data) |
| `map<T, U>(items: T[], ...): U[] => ...` | `functions: {map: {generic: ["T", "U"], params: {...}, return: {...}, body: "..."}}` |
| `first<T>(arg: T): T => arg` | `fields: {first: {generic: ["T"], arg: {type: "T"}, type: "T", expr: "arg"}}` |
| `items<T>: T[]` | `fields: {items: {generic: ["T"], type: "array", items: {type: "T"}}}` |
| `payload_json = "..."` | `payload_json: "..."` (top-level data) |

### Type inference rules

The frontend omits `type:` when the type is obvious:

| Context | Inferred type |
|---|---|
| `{ ... }` body | `object` (with `properties` from body) |
| `[]` suffix | `array` (with `items` from prefix type) |
| `=> expr` only (no type, no braces) | omitted — typechecker infers from expr |
| Empty `{}` | `unknown` (no type emitted) |

When `type:` is omitted, the typechecker's existing `Type::Unknown`
handling applies — it passes silently if the type cannot be inferred
(see `handl-typecheck-keys/src/expr/check.rs:37-39`).

### Implementation plan

1. **New crate `handl-frontend-handl`** — implements `Frontend` trait.
2. **Parser** — hand-written or `pest`/`nom` PEG parser.
3. **Codegen** — walks AST, emits `serde_json::Value` in canonical
   structure (1:1 mapping, no logic).
4. **Register in `frontend_for`** — add `"handl"` match arm in
   `handl-frontend/src/lib.rs`.
5. **Source map** — implement `parse_with_locations` for type error
   reporting (line/column tracking).

### Verbosity comparison

| Construct | `.handl` | YAML | Reduction |
|---|---|---|---|
| Computed field (no type) | 1 line | 3 lines | 67% |
| Typed computed field | 1 line | 3 lines | 67% |
| Type-only field | 1 line | 2 lines | 50% |
| Array shorthand (`string[]`) | 1 line | 4 lines | 75% |
| Parameterized (untyped arg) | 1 line | 4 lines | 75% |
| Parameterized (typed arg) | 1 line | 5 lines | 80% |
| Function | 1 line | 7 lines | 86% |
| Complex field (schema + expr) | 5 lines | 9 lines | 44% |
| Data (object) | 4 lines | 3 lines | -33% |
| **Full 15-field example** | **~25 lines** | **~80 lines** | **69%** |

YAML remains slightly more compact for pure data objects. `.handl` wins
everywhere else, especially for functions and parameterized fields.

## Consequences

**Positive:**

- 69% line reduction for typical documents.
- Type inference eliminates redundant `type:` declarations.
- Brace-based schemas are visually cleaner than nested YAML maps.
- Arrow syntax unifies fields and functions under one mental model.
- Zero downstream changes — same `NonFunctionValue`, same pipeline.
- Existing YAML/JSON/TOML frontends remain fully supported.

**Negative:**

- New syntax to learn — users must understand `=>` vs `=` vs `:`.
- New crate to maintain (parser, codegen, source maps).
- Data objects are slightly more verbose than YAML (`key = { ... }` vs
  `key: ...`).
- String escaping differs from YAML (single quotes for expr strings
  inside `.handl` vs YAML's quoting rules).

**Open questions:**

- **Modifier syntax.** Resolved — keyword prefixes: `inherit`, `lock`, `spread`.
  - `inherit` → `inheritable: true` (value flows down to nested scopes)
  - `lock` → `mutable: false` (value cannot be overridden in nested scopes)
  - `spread` → `merge: true` (object value spread into parent, field removed)
  - Modifiers can combine: `inherit lock api_key: string`
- **Enum shorthand.** Resolved — `role: "admin" | "user" | "guest"`
  syntax. Type inferred from first literal value. The `|` appears only
  in type position (after `:`), no ambiguity with expressions.
- **Generic constraints.** `T: comparable` syntax (like TypeScript's
  `T extends Comparable`) is reserved for future use. V1 supports bare
  type parameters only. Constraints would require a named schema/type
  registry that does not exist yet.
- **Pipe/compose runtime.** `|>` is pure desugar (no runtime change).
  `>>` and `<<` require adding `fp.flow` and `fp.compose` to
  `handl-lib-utils` — these return chained `FunctionValue`s. Implementation
  detail, not a language design question.
- **Multi-line expressions.** Resolved — handl is expression-based, not statement-based. `{ }` in function bodies groups a single expression (e.g. if-expressions), not multiple statements. Long expressions may need line continuation (`\` or automatic inside parens) — deferred to implementation.
- **Comments.** Resolved — `//` for line comments, `/* */` for block comments (TS-consistent).

**Non-goals:**

- This frontend does not replace YAML/JSON/TOML — it is an additional
  option.
- The `.handl` syntax does not change the language semantics — it is a
  pure syntactic transformation producing the same `NonFunctionValue`.
- Custom dialects (different key names) are not addressed here — the
  `.handl` frontend targets the canonical dialect.

## Status

Proposed
