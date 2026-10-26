# Tutorials

Tutorials are hands-on lessons for learning the platform by doing. If
you are new here, start with the first one and work through them in
order. Each tutorial is self-contained, and every step produces a
working result.

## Start here

- [Set up the platform](./setup) — clone the public repos and build the
  workspace.

## Build with the platform

- [Build a host](./build-a-host) — create a Rust binary that composes
  plugins and runs a capability.
- [Write a plugin](./write-a-plugin) — implement `Plugin` and contribute
  a rule to an operation.
- [Dependent types in handl](./dependent-types) — indexed family
  declarations, `match` refinement, `Path`/transport/`J`, `IO<...>`
  handlers, and implicit `<...>` binders.

## The research track

The end-to-end path for the `graphopt` stack (renamed from
`research-mlopt`) — a model in, a cheaper certified form out, and the
law-discovery loop underneath:

- [Research track](/graphopt/tutorials/) — install → first `search()` → first
  minted law → the scorecard.

## When to use another section

- If you know the basics and need a recipe, see the
  [How-to guides](../how-to/).
- If you want to understand why the platform is built this way, see the
  [Explanations](../explanation/).
- If you need function signatures and examples, see the
  [Reference](../reference/).
