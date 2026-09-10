# handl-plugin-lib-tag

Tagged values — the host primitive for **nominal identity** in handl
(ADR-0055, ADR-0054). A tagged value is a JSON object with a reserved
`__tag__` key, minted only by the host:

```json
{ "__tag__": "ok", "value": 42 }
{ "__tag__": "type.collection", "__registry__": "handl.canonical", "name": "number" }
{ "__tag__": "Number", "__registry__": "handl.canonical", "grades": ["Addable"], "value": 1.0 }
```

Tagging is a **host capability, not a convention**: user object literals
cannot contain `__tag__` (eval engines reject it), so a value carries a
tag only if the host minted it via this crate. That makes tags unforgeable
nominal identities — ideal for grades, types, "classes", `Result`/`Option`,
capabilities, and provenance.

## API

Module `tag`, declared in `handl-packages/tag/tag.hnl` (`op` + `handl`), backed
by Rust handlers in an `OpRegistry`:

| Callable | Role |
|---|---|
| `tag.tag(name, payload)` | **mint** a tagged value (host capability) |
| `tag.untag(v)` | payload of `v` (`__tag__` removed), or `null` |
| `tag.tag_of(v)` | tag name string, or `null` |
| `tag.is_tag(v)` | `true` iff `v` carries a string `__tag__` |
| `tag.construct(name, payload)` | **mint** a registry-scoped *type value* (`handl.canonical`) |
| `tag.match_type(v, cases)` | **dispatch** on a type value's tag (no default fallback; `null` on miss) |

Payload rules for `tag.tag`: object fields are spread into the value,
`null` means empty, any other value is wrapped as `{value: payload}` —
so `tag.tag("ok", 42)` → `{"__tag__":"ok","value":42}`. A payload that
smuggles `__tag__` (or `__registry__` for `construct`) is rejected.

## Basic use

```handl
tag.tag("ok", 42)                        -- {"__tag__":"ok","value":42}
tag.tag("unit", null)                    -- {"__tag__":"unit"}
tag.tag("point", { x: 1, y: 2 })         -- {"__tag__":"point","x":1,"y":2}

tag.untag(tag.tag("unit", { m: 5 }))     -- { m: 5 }
tag.tag_of(tag.tag("ok", 1))             -- "ok"
tag.is_tag("plain")                      -- false
tag.is_tag(tag.tag("ok", 1))             -- true

{ "__tag__": "ok" }                      -- ERROR: reserved key, unforgeable
tag.untag("plain")                       -- null (not a tagged value)
tag.tag_of({ a: 1 })                     -- null
```

Tags compare by structure: same tag + same payload means equal.

## Syntax sugar: `tag` declarations

Writing `tag.tag("ok", …)` everywhere is verbose. `.hnl` sources support a
`tag` declaration that expands to a constructor arrow (ADR-0077):

```handl
tag ok = Value
tag err = Error

{
  ok: ok,
  err: err,
}
```

expands (conceptually) to:

```handl
do {
  ok :: _0 ~> tag.tag("ok", { value: _0 })
  err :: _0 ~> tag.tag("err", { value: _0 })
  { ok: ok, err: err }
}
```

This is exactly how `handl-packages/result/result.hnl` defines the `Result`
type — `result.ok(1)` mints `{"__tag__":"ok","value":1}` and
`result.err(e)` mints `{"__tag__":"err","value":e}`. The `= Type` side is
documentation/contract intent (`ok` wraps a `Value`, `err` wraps an `Error`);
the constructor always tags `{value: payload}`.

## Pattern matching on tags

No dedicated match op is needed — the native `match` dispatches on
`__tag__` identity with call-shaped patterns `TagName(binder)`:

```handl
match r {
  ok(v): v,
  err(e): e,
}

-- tag payload object form also works: ok(v) binds the single `value` field;
-- ok(_) ignores the payload. With several payload fields, binders match
-- fields positionally in alphabetical key order (excluding `__tag__`).
```

In Rust AST terms, `ok(v)` is a `call` node with callee `var("ok")` and a
binder arg. First matching arm wins; no arm matches → match error.

Combined with `error.try`, this gives railway-style handling:

```handl
match error.try(_ ~> might_fail()) {
  ok(v): v,
  err(msg): "failed: " + msg,
}
```

## Applications: grades, types, "classes", nominal identity

Because tags are unforgeable nominal markers sharing one value space
(ADR-0032 "types are values"), every layer can coin identities without
colliding:

- **Grades / capabilities** — values carry `grades: ["Addable"]` next to
  `__tag__: "Number"`; `op` declarations name a required grade and handlers
  are looked up per grade (`grade_dispatch` / `graded_e2e` tests).
- **Types as values** — `tag.construct("type.collection", {name:"number"})`
  mints `{"__tag__":"type.collection","__registry__":"handl.canonical",…}`;
  `tag.match_type(v, { "type.collection": …, "type.pi": … })` dispatches on
  the type tag. The `__registry__` scopes equality/matching to one registry
  (`TypeRegistry`, `src/registry.rs`, id `"handl.canonical"`).
- **"Classes" / ADTs** — each constructor is a distinct tag
  (`ok`/`err`, `Some`/`None`, `Cons`/`Nil`); construction mints, matching
  dispatches. Prefer the `tag` sugar + `match` over manual string checks.
- **Provenance / hosts** — `assert` returns `ok(true)` /
  `err({message, expected, actual, diff})` tagged values using the same
  primitive.

Rule of thumb: if identity must be *checked* (did this come from the real
`ok` constructor / the canonical registry / the `Number` sort?), make it a
tag. If it is just data, keep it a plain object.

## Rust surface

- `Tag`, `Untag`, `TagOf`, `IsTag`, `Construct`, `MatchType` — `Function`
  impls registered in `tag_op_registry()` under `"tag.*"` op names.
- `TagModule` — legacy `Module` (`tag`, `untag`, `tag_of`, `is_tag`,
  `construct`, `match_type`); kept for `kit-canonical`-style wiring.
- `TagLibraryPlugin` / `tag_handl_plugin()` — `HandlLibraryPlugin` wrapper
  loading `tag.hnl` with the op registry (the capability boundary: no plugin
  → no minting).
- `TypeRegistry::new(id)` — `construct` / `is_type_value` / `as_type_tag` /
  `match_type` (pure data, no callable).
- `handl_language::{tag_value, as_tag, is_tag_value, is_reserved_value_key}`
  — mint/read/test primitives and the reserved-key guard
  (`__tag__`, `__registry__`, `__language__`, `__ast__`, `__arrow__`,
  `__home__`).
