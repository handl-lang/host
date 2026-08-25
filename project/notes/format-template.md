# format / template — string interpolation

## Status

Implemented as `TemplateModule` in `crates/lib-utils/src/template`.

## Summary

A `format` / `template` function for string interpolation in handl
expressions. Two distinct designs exist; the choice determines where it lives.

Takes a template string and a values object as arguments. Substitutes
placeholders from the passed-in object only. No scope access.

```
str.format("Hello {name}", {name: "world"})  →  "Hello world"
str.format("{a} + {b} = {c}", {a: 1, b: 2, c: 3})  →  "1 + 2 = 3"
```

- **Where:** `StrModule` in `lib-utils`, next to `str.concat` / `str.replace`.
- **Shape:** Same as existing `str` functions — pure function over arguments,
  returns a string, no scope or context needed.
- **Pros:** Simple, testable, fits the existing `lib-utils` pattern perfectly.
  All current modules (`math`, `str`, `array`, `bool`, `fp`, `type`) are pure
  functions over their arguments. None use `Context` for anything meaningful
  (`_ctx` everywhere).
- **Cons:** Limited — caller must explicitly pass all values. No nested
  expression evaluation.

### Design B: True interpolation from scope — runtime feature (NOT lib-utils)

Resolves `{name}` from the current variable bindings, or evaluates embedded
expressions like `{a + b}`.

```
let name = "world"
format("Hello {name}")           →  "Hello world"
format("Result: {a + b}")        →  "Result: 3"
```

- **Where:** Runtime layer (e.g. `handl-vm` or a dedicated interpolation crate).
  Needs deep access to the evaluation context, scope chain, or evaluator.
- **Pros:** Powerful, ergonomic, natural for template-heavy workflows.
- **Cons:** Breaks the `lib-utils` pattern (all current modules are pure
  functions over arguments with no scope access). Requires evaluator
  integration. More complex error handling (what if a variable doesn't exist?).

## Recommendation

**Design A** for `lib-utils` — it's a natural fit and can ship independently.
If true scope-based interpolation is needed later, it should be a separate
runtime-level feature, not a `lib-utils` module.

## Open questions

- Placeholder syntax: `{name}` vs `${name}` vs `{{name}}`?
- Missing key behavior: error, leave as-is, or empty string?
- Should Design A support nested paths like `{user.name}`?
- Should Design B support expression evaluation (`{a + b}`) or only variable
  lookup (`{a}`)?
- Naming: `str.format` vs `str.template` vs separate `fmt` module?

## handl Language Usage

The implemented `TemplateModule` is exposed under the `template` namespace and
uses lodash-style `<%` delimiters. All embedded expressions are full handl
expressions, so any function available in the host is callable.

### `template.render(template, data, options?)`

Returns a rendered string.

```
template.render("Hello <%= name %>!", {name: "handl"})
// → "Hello handl!"
```

### HTML-escaped output

```
template.render("<%- html %>", {html: "<script>"})
// → "&lt;script&gt;"
```

### Silent evaluation

```
template.render("A<% add(1, 2) %>B", {})
// → "AB"
```

### Custom delimiters

```
template.render(
  "Hi {{ name }}!",
  {name: "handl"},
  {delimiters: {interpolate: ["{{", "}}"]}}
)
// → "Hi handl!"
```

### `template.compile(template, options?)`

Returns a first-class handl function. Apply it later with a data object.

```
let greet = template.compile("Hello <%= name %>!")
fn.apply(greet, [{name: "handl"}])
// → "Hello handl!"
```

### `template.escape(value)`

Escapes HTML entities for use in templates or standalone output.

```
template.escape("< > ' \" / &")
// → "&lt; &gt; &#x27; &quot; &#x2F; &amp;"
```

### Options reference

- `delimiters.interpolate` — open/close tokens for `<%= %>` (default `["<%=", "%>"]`)
- `delimiters.escape` — open/close tokens for `<%- %>` (default `["<%-", "%>"]`)
- `delimiters.evaluate` — open/close tokens for `<% %>` (default `["<%", "%>"]`)
- `imports` — object of names to make available in embedded expressions
- `variable` — name for the root data object inside embedded expressions
