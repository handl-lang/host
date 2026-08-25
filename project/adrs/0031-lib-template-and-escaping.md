# lib-template and lib-escaping

## Status

Proposed

## Context

ADR 0030 introduces `lib-lang` as the public façade for parsing and evaluating
Handl code. With that in place, template rendering and output escaping can be
pulled out of the language runtime and treated as ordinary consumers of the
language API.

Currently `lib-utils` contains `str.escape_html` and `str.escape_url`. These are
context-sensitive operations (the correct escape rules depend on whether the
output is HTML, a URL, JavaScript, CSS, etc.) and do not belong in a generic
string utility module. They also cannot be safely extended in `lib-utils`
because `lib-utils` is meant for pure string transforms with no knowledge of
output context.

Similarly, a template engine needs to:

1. Parse a template string into literal and expression tokens.
2. Evaluate embedded Handl expressions.
3. Encode the results for the target output context.

Only step 1 is the template engine's own concern. Steps 2 and 3 belong in
`lib-lang` and `lib-escaping`, respectively. Putting all three in one crate would
make `lib-template` depend on parser, evaluator, and HTML/URL details at once,
coupling it to the full compiler and to every escaping context.

## Decision

`lib-template` and `lib-escaping` are separate crates. `lib-template` consumes
`lib-lang` for expression evaluation and `lib-escaping` for output encoding. It
does not own parsing, evaluation, or escaping.

### `lib-escaping`

`lib-escaping` is a context-aware escaping library. It owns the escape rules for
every output context the platform supports, starting with HTML and URL and
later adding JS, CSS, SQL, etc. It exposes a small, uniform API:

```rust
// Concrete context functions
escaping::html::escape("<script>") // → "&lt;script&gt;"
escaping::url::escape("a=b&c=d")   // → "a%3Db%26c%3Dd"

// Generic interface for callers that choose a context at runtime
escaping::escape(value, Context::Html)
escaping::escape(value, Context::Url)
```

The concrete module layout is `escaping::{html, url, js, css, ...}`. Each
context is an independent module so callers can depend on only the contexts they
need, while the top-level `escaping::Context` enum and `escaping::escape`
dispatcher provide a uniform runtime interface.

`lib-escaping` operates on strings. Callers that have `NonFunctionValue` values
must coerce them to strings first; the `lib-template` renderer does this after
evaluating an expression through `lib-lang`.

### `lib-template`

`lib-template` is a template renderer. It parses a template string and produces
output by evaluating Handl expressions through `lib-lang` and escaping the
results through `lib-escaping`.

```rust
template::render(
    "Hello <%= name %>, safe: <%- html %>",
    json!({"name": "Handl", "html": "<b>"}),
    template::Options {
        escape: escaping::Context::Html,
        ..Default::default()
    },
)
// → "Hello Handl, safe: &lt;b&gt;"
```

`lib-template` recognizes three delimiter kinds:

- `<%= expr %>` — evaluate `expr` and insert the result.
- `<%- expr %>` — evaluate `expr` and insert the HTML-escaped result (or the
  result escaped by the configured `Options.escape` context).
- `<% expr %>` — evaluate `expr` for side effects only, producing no output.

The default delimiters are `<%=`, `<%-`, and `<%` with `%>` as the close token.
They can be customized through `Options.delimiters`.

### Why `lib-template` does not include `template.escape`

A standalone `template.escape` function is not needed. Escaping is a property of
the output context, not the template engine. Callers use the concrete escaping
modules (`escaping::html`, `escaping::url`) directly when they need to escape a
value outside of a template. Inside a template, the renderer applies the
configured `escaping::Context` automatically.

### Why `lib-template` does not evaluate expressions itself

Embedded expressions in a template are Handl expressions. `lib-template` calls
`lib-lang` to parse and evaluate them. This keeps `lib-template` from
reimplementing the parser or duplicating the canonical expression evaluator, and
it ensures that the sandboxed VM (see ADR 0030) can be used for template
evaluation just like any other Handl-in-Handl use case.

## Consequences

### Positive

- `lib-template` is a thin, focused crate: tokenization, renderer loop, and
  delimiter configuration only.
- `lib-escaping` can grow new contexts without touching `lib-template` or
  `lib-utils`.
- Escaping rules are centralized and cannot diverge between the template engine,
  HTTP output, and other emitters.
- `lib-lang` remains the single source of truth for parsing and evaluation.
- The existing `str.escape_html` and `str.escape_url` functions can be
  deprecated and moved to `escaping::html` and `escaping::url`.

### Trade-offs

- `lib-template` must take a dependency on both `lib-lang` and `lib-escaping`.
  This is the correct coupling: it needs both to do its job, but neither belongs
  inside it.
- Every escaping context is a separate module. This is more files than a single
  `str.escape_*` function, but it prevents one context from leaking into another.
- Callers must choose an escaping context explicitly. There is no default
  "escape everything" behavior; the default for `template.render` is `Html` only
  because that is the most common template use case, but it can be changed or
  disabled.

## References

- `adrs/0030-lib-lang.md`
- `crates/lib-utils/src/str/transform.rs` (existing `escape_html` and
  `escape_url`)
