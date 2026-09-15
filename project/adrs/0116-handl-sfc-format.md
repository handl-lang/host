# ADR-0116: Handl Single-File Component (`.hfc`) Format

## Status

Proposed.

## Context

[ADR-0094 — Generic AST Transform Engine](./0094-generic-ast-transform-engine.md)
makes transforms first-class `Ast → Ast` passes. [ADR-0114 — Handl Compile
Plugin](./0114-handl-compile-plugin-architecture.md) is planned to lower Handl to
JavaScript. [ADR-0115 — `.d.ts` SDKgen](./0115-handl-sdkgen-dts-js.md) will
generate Handl port/adapter pairs from NPM package declarations.
[ADR-0117 — Handl View / JSX-like Component Syntax](./0117-handl-view-component-syntax.md)
defines the inline `view` syntax itself. The missing piece is a file format that
bundles the view, the script, and the style into one source file, the way Vue
SFCs do.

## Decision

Add an `.hfc` (Handl File Component) file format and loader.

### File layout

An `.hfc` file has three optional regions:

```hfc
<script>
export props := #{ name: String, greet?: Bool }
state counter := 0
export bump := () ~> counter := counter + 1
</script>

<template>
  <div class="hello">
    <h1 v-if="greet">Hello, {name}</h1>
    <h1 v-else>Hey, {name}</h1>
    <button @click="bump">{counter}</button>
    <slot name="footer">Default footer</slot>
  </div>
</template>

<style scoped>
.hello { color: blue; }
</style>
```

### Loader behavior

1. Splits the file at region markers (`<script>`, `<template>`, `<style>`).
2. Parses `<script>` and `<script setup>` as a Handl module.
3. Parses `<template>` as a `view` block and runs the `component` transform from
   ADR-0117.
4. Carries `<style>` as data, rewriting selectors if `scoped` is present and
   adding the generated scope id to every host element in the `<template>`.
5. Emits a single Handl module plus a CSS string/artifact for the compile-to-JS
   backend.

### `<script>` variants

- `<script>`: ordinary module. `export`ed names are the public API and are
  available in the template. `state` declarations become reactive state. `props`
  (when exported as a record) becomes the component's props type.
- `<script setup>` (Vue-style): the entire region is a setup function. Top-level
  values and arrows are automatically exposed to the template and returned by the
  setup. This is the default for one-component-per-file files.

### Scoped styles

When `<style scoped>` is present, the loader:

1. Generates a unique scope id for the file.
2. Rewrites CSS selectors to include the scope attribute
   (e.g. `.hello[data-v-1a2b3c]`).
3. Adds the scope id to every host element in the `<template>`.
4. Emits the processed CSS as a string that the compile-to-JS backend writes to a
   `<style>` block or a `.css` asset.

### Slots

Slots are explicit `<slot>` elements. A default slot has no `name`; named slots
use `name="..."`. Scoped slots pass data back to the parent through a `let:`
attribute on the slot and an `each` attribute for iteration. The lowering is the
same Handl callback arrow mechanism described in ADR-0117.

## Consequences

- One file per component, matching Vue/Astro/Svelte conventions.
- The loader is a small pre-processor; the real syntax work is in ADR-0117.
- The style step is not typechecked by Handl; it is emitted as data for the JS
  target.
- Astro/Svelte/Vue/React can share the same `<template>` syntax and just need a
  different `target` config in the compile step.

## References

- [ADR-0094 — Generic AST Transform Engine](./0094-generic-ast-transform-engine.md)
- [ADR-0114 — Handl Compile Plugin](./0114-handl-compile-plugin-architecture.md)
- [ADR-0115 — Handl SDK Generator: TypeScript Declaration Backend](./0115-handl-sdkgen-dts-js.md)
- [ADR-0117 — Handl View / JSX-like Component Syntax](./0117-handl-view-component-syntax.md)
