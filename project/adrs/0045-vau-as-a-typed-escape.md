# Vau as a Typed Escape (ADR-0041 Phase 4)

## Context

ADR-0041 introduced `Language` as a first-class value and `eval`/`self`/
`compile` as handl-callable functions. The structured `Language` value —
grammar as data, eval schemes as data — is the default. Every construct
describes its evaluation as a `Scheme` combinator (`App`, `Bind`, `Branch`,
`Prim`, `Foreign`, etc.) that the tree-walk engine interprets
compositionally. The compute graph is statically recoverable: you can
inspect a `Language` value and know what every node does without running it.

This is the arrow substrate. Arrows give you static structure — you can
inspect, optimize, and type-check the computation without executing it. But
arrows lose expressiveness compared to monads: you cannot intercept the
continuation. Some behaviors need to receive the **unevaluated** expression
and decide what evaluation means in place. These are **fexprs** (first-class
operative expressions), or in Kernel terminology, `vau` forms.

The canonical example is a short-circuiting `and`: a structured `and`
evaluates both operands then applies a primitive. A `vau` `and` receives
both operands unevaluated, evaluates the first, and only evaluates the
second if the first is truthy. The structured form is inspectable but
strict; the `vau` form is flexible but opaque to static analysis.

ADR-0041 §6 specifies the design: "Structured by default, vau as an
explicitly-typed opt-in escape, tracked by the type system. Pure/structured
code compiles; vau code interprets. You opt in, the way you opt into
`ArrowApply`."

This ADR refines that design into a concrete implementation plan.

## Decision

### 1. `Scheme::Vau` — a new scheme variant

Add a new variant to `handl_language::Scheme`:

```rust
/// A vau (fexpr) scheme: receives unevaluated child expressions and the
/// current environment, and decides how to evaluate them. The escape hatch
/// from structured compositional evaluation.
Vau {
    /// The parameter names for the unevaluated expression nodes.
    /// These are bound in the evaluation environment to `AstRef`s
    /// (unevaluated), not to `Value`s (evaluated).
    params: Vec<String>,
    /// The body: a scheme to evaluate in the extended environment.
    /// The body can call `eval` on the bound `AstRef`s to evaluate
    /// them, or inspect/transform them without evaluating.
    body: Box<Scheme>,
}
```

When the tree-walk engine encounters `Scheme::Vau`, it does **not** evaluate
the children. Instead, it binds the child `AstRef`s to the param names in a
new scope and evaluates the body. The body can:

- Call `eval(self, ast)` to evaluate a bound expression (recovering
  structured behavior).
- Inspect the AST node (kind, payload, children) to make compile-time
  decisions.
- Transform the AST and eval the result (metaprogramming).

### 2. `vau` syntax in the canonical language

Add a `vau` keyword token and grammar rule:

```yaml
- id: grammar.vau.prefix
  token: token.vau
  position: Prefix
  binding_power: [0, 0]
  child_shape:
    node_kind: vau
    arity: 2 # params list + body
```

The eval scheme for `vau` nodes is `Scheme::Vau` itself — the node
**is** the fexpr definition. When a `vau` node is evaluated, it produces a
closure-like value that, when applied, intercepts the argument expressions
before evaluation.

### 3. Type system tracking

The typechecker marks `vau`-derived values with a distinct type:
`type.vau<a, b>` (a vau from unevaluated `a` to `b`). This is the
`ArrowApply` marker — it tells the type system that this value escapes
structured composition.

- Pure/structured code (no `vau`) compiles to native via cranelift.
- `vau` code falls back to tree-walk interpretation.
- The type system prevents `vau` values from flowing into positions that
  require structured code (e.g. `compile` targets) without an explicit
  `escape` annotation.

### 4. Capability interaction

A `vau` behavior can call `eval` on arbitrary ASTs, which means it can
trigger any capability available to the current `Language` value. This
interacts with ADR-0046 (capability discipline): a `vau` behavior is
automatically flagged as requiring `eval` capability. A sandboxed
`Language` without `eval` cannot contain `vau` behaviors.

### 5. Implementation phases

1. **`Scheme::Vau` in the tree-walk engine** — the core semantics. Bind
   unevaluated `AstRef`s, evaluate body. No type system changes yet.
2. **`vau` syntax** — token, grammar rule, eval scheme. Test
   short-circuiting `and` as the canonical example.
3. **Type system tracking** — `type.vau<a, b>`, compile/interpret split.
4. **Optimization** — detect when a `vau` always evaluates its arguments
   (structured-compatible) and optimize it back to `App`.

## Consequences

### Positive

- **Full expressiveness.** Any evaluation strategy can be implemented as a
  `vau` — short-circuiting, lazy evaluation, pattern matching, macros.
- **Opt-in.** The default is structured; `vau` is explicit. Code that
  doesn't need it stays inspectable and optimizable.
- **Composable with `eval`.** A `vau` body can call `eval(self, ast)` to
  evaluate sub-expressions, making the escape local and recoverable.

### Negative

- **Static analysis breaks at `vau`.** The compute graph is opaque past a
  `vau` boundary — you can't know what it does without running it.
- **Type system complexity.** Tracking `vau` types and the
  compile/interpret split adds typechecker complexity.
- **Performance.** `vau` code falls back to tree-walk; it cannot be
  compiled to native unless proven structured-compatible.

### Neutral

- **Kernel lineage.** This is the Kernel (Shutt) `vau`/fexpr design, lifted
  into a typed, arrow-structured language. The key difference from Kernel
  is that `vau` is typed and opt-in, not the default evaluation strategy.

## Status

Proposed

## References

- [ADR-0041](./0041-languages-as-first-class-values.md) — Languages as
  First-Class Values (Phase 4 specification)
- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as
  Data and Runtime Split (the `Scheme` combinator system)
- [ADR-0046](./0046-capability-discipline-at-the-language-level.md) —
  Capability Discipline at the Language Level (Phase 5)
- Kernel (Shutt) — `vau` and first-class operatives
- Arrow calculus (Lindley, Wadler, Yallop) — `ArrowApply` as the escape
