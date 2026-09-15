# ADR-0117: Handl View / JSX-like Component Syntax for JS Targets

## Status

Proposed.

## Context

[ADR-0115 — `.d.ts` SDKgen](./0115-handl-sdkgen-dts-js.md) will make JS
frameworks reachable as low-level Handl arrows (`createElement`, `h`, `useState`,
`ref`). That is verbose. [ADR-0116 — `.hfc` SFC Format](./0116-handl-sfc-format.md)
packages the view into a file with `<template>`, `<script>`, and `<style>`. This
ADR defines the inline view syntax itself.

## Decision

Add a `view` block and XML-like tag syntax to Handl. It is parsed as an optional
language surface and lowered to ordinary Handl arrows by the `component`
transform pass.

### Syntax

A `view` is a named arrow that returns a JS value:

```handl
view Hello(name: String) {
  <div>
    <h1>Hello, {name}</h1>
  </div>
}
```

The `component` transform lowers this to plain Handl:

```handl
Hello := (name) ~[JS]~> Value
Hello = (name) ~> do {
  h1 = createElement("h1", #{}, ["Hello, " ++ name])
  createElement("div", #{}, [h1])
}
```

### Lowering rules

- `<tag>` is sugar for `createElement("tag", attrs, children)` (or `h` for the
  Vue target).
- `<Tag ...>` is a component call; the tag must be a capitalized identifier that
  resolves to a Handl arrow.
- `attr="val"` becomes `#{ attr: "val" }`.
- `attr={expr}` becomes `#{ attr: expr }`.
- `{expr}` inside a tag is text interpolation or a child expression.
- `v-if="cond"` / `v-else` become ordinary `if`/`else`.
- `@event="handler"` becomes an event attribute such as `#{ onClick: handler }`.
- Capitalized tags are components; lowercase tags are host elements.
- A `view` is just a named arrow, so it composes with ordinary Handl code.

### Slots

Slots are explicit `<slot>` elements:

```handl
view List(items) {
  <ul>
    <slot name="item" each={items} let:item>
      <li>{item}</li>
    </slot>
  </ul>
}
```

- A default slot has no `name`.
- Named slots use `name="..."`.
- Scoped slots use `let:` to name the data passed back and `each:` to iterate a
  list. The lowering is a Handl callback arrow.

### Reactive state

A `state` block declares component-local reactive state:

```handl
view Counter() {
  state count := 0

  <button onClick={() ~> count := count + 1}>
    {count}
  </button>
}
```

The `component` transform turns `state` into the target's reactivity call
(`useState` for React, `ref`/`reactive` for Vue, stores for Svelte).

### Target binding

The transform is target-agnostic. A compile-time `target` config picks the
runtime calls:

- `target = "react"` → `createElement`, `useState`, etc.
- `target = "vue"` → `h`, `ref`, `reactive`, `render_slot`, etc.
- `target = "svelte"` → Svelte runtime calls or `.svelte` source output.
- `target = "dom"` → `document.createElement`, `document.body.append`.

This is equivalent to the JSX factory setting in TypeScript, but extended to
multiple target frameworks.

## Consequences

- The `component` pass is a pure `Ast → Ast` transform and can be previewed,
  traced, or disabled via `hnl transform component`.
- SFC packaging is handled separately in ADR-0116.
- The syntax is small and not framework-specific; adding Astro, Svelte, or Solid
  later is a new `target` adapter, not new grammar.
- Out of scope for the first version: framework-specific lifecycle hooks
  (`onMounted`, `onUnmounted`, `useEffect`).

## References

- [ADR-0094 — Generic AST Transform Engine](./0094-generic-ast-transform-engine.md)
- [ADR-0114 — Handl Compile Plugin](./0114-handl-compile-plugin-architecture.md)
- [ADR-0115 — Handl SDK Generator: TypeScript Declaration Backend](./0115-handl-sdkgen-dts-js.md)
- [ADR-0116 — Handl Single-File Component Format](./0116-handl-sfc-format.md)
