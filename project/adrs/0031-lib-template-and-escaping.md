# lib-template

## Status

Proposed

## Context

String templating in the Handl platform needs to move from a monolithic runtime feature to a composable, language-agnostic engine. The current `lib-utils` `TemplateModule` bundles tokenization, expression evaluation, and HTML escaping into a single `template.render` implementation. This makes it hard to support different syntaxes, output contexts, or evaluation strategies without changing the whole module.

We also already have a capable expression evaluation layer in `lib-lang` and the VM. The template engine should not duplicate or depend deeply on that layer; instead, it should be a coordinator that knows how to find code inside text and delegate the actual evaluation.

Escaping is similarly a cross-cutting concern. The rules for HTML, URL, JS, CSS, SQL, etc. do not belong to the template engine. They belong in a separate escaping library that the engine can call.

## Decision

`lib-template` is a **pluggable template engine**.

It owns the pipeline for turning a template string into output text:

1. **Tokenizer** — turns the source string into a sequence of tokens (`Text`, `OpenDelimiter`, `CloseDelimiter`, etc.).
2. **Parser** — builds an AST from tokens (`TextNode`, `InterpolateNode`, `EvaluateNode`, `BlockNode`, `FilteredNode`, etc.).
3. **Transformer passes** — optional AST-to-AST passes for optimization or user-defined rewriting.
4. **Renderer** — walks the AST, evaluates embedded expressions through a `RenderContext`, applies escapers, and concatenates the final string.

All language-specific, escaping, and syntax concerns are injected via traits and registries. The engine is not tied to Handl delimiters, the Handl evaluator, or any particular escaping context.

### Pluggable pieces

| Component             | Injected via                       | Examples                              |
| --------------------- | ---------------------------------- | ------------------------------------- |
| Delimiter syntax      | `Tokenizer` + `Vec<DelimiterRule>` | `<%= ... %>`, `{{ ... }}`, `${ ... }` |
| Expression evaluation | `RenderContext` trait              | `lib-lang`, VM, no-op, sandboxed      |
| Output escaping       | `Escaper` registry                 | HTML, URL, JS, CSS, SQL, none         |
| Value transforms      | `Filter` registry                  | `upper`, `slugify`, `json`            |
| Control flow          | `Tag` registry                     | `if`, `for`, `with`                   |

### The `RenderContext` boundary

The renderer delegates every embedded expression to a `RenderContext`:

```rust
pub trait RenderContext {
    fn eval_expr(&self, expr: &str, scope: &[(String, Value)]) -> Result<Value, Error>;
    fn eval_block(&self, tag: &str, args: &[Value], body: &str) -> Result<Option<String>, Error>;
}
```

For the Handl runtime, a `HandlRenderContext` is provided. It parses and evaluates expressions through `lib-lang`. Other hosts can provide a different context without changing `lib-template`.

### Why the engine does not evaluate expressions itself

Embedded expressions are Handl expressions (or whatever language the host uses). The engine calls `RenderContext::eval_expr` for every `<%= ... %>` or `{{ ... }}` region. This keeps the engine language-agnostic and lets it use the same parser, evaluator, and sandbox as any other Handl code.

### Why the engine does not escape values itself

Escaping rules depend on the output context (HTML, URL, JS, CSS, SQL, plain text). The engine applies an escaper selected by:

- the delimiter (`<%- ... %>` selects the configured escape context),
- an explicit `escape` option passed to `render`,
- a filter that requests a specific escaper.

The rules themselves live in `lib-escaping`, which is described in a separate ADR.

### Public API

At the Handl language surface:

```ac
template.render("Hello, <%= name %>!", {name: "handl"})
template.render("Safe: <%- html %>", {html: "<script>"})
template.render("Hi {{ name }}!", {name: "handl"}, {delimiters: {interpolate: ["{{", "}}"]}})
let greet = template.compile("Hello, <%= name %>!")
```

At the Rust host surface:

```rust
let engine = Engine::builder()
    .tokenizer(HandlTokenizer::default())
    .evaluator(HandlRenderContext::new(&vm))
    .escaper(EscaperRegistry::with(html, url))
    .build();
```

### Crate boundaries

- `lib-template-core` — tokenizer, parser, AST, renderer, and trait definitions.
- `lib-template-stdlib` (optional) — built-in filters and tags such as `upper`, `if`, `for`.
- `lib-template` — the pre-configured Handl-facing crate that `lib-utils` or the VM can use.
- `lib-escaping` — escaping rules; addressed in a separate ADR.

## Consequences

### Positive

- `lib-template` is no longer a monolithic `render` function.
- The same core can power Handl templates, email generators, code scaffolding, and other text-assembly use cases.
- New delimiters, filters, tags, and escapers can be added without touching the engine internals.
- The engine can be used with or without the Handl VM.
- `lib-lang` remains the single source of truth for Handl expression evaluation.

### Trade-offs

- The engine is larger than the current `TemplateModule` because it is now a pipeline of traits and registries.
- Callers must choose an evaluator and escapers explicitly.
- Control flow inside templates (`if`, `for`) is an extension, not a built-in language feature, which may require more design work.

## References

- `adrs/0030-lib-lang.md`
- `crates/lib-utils/src/template/core.rs` (existing implementation)
- `project/notes/format-template.md`
