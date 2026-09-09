# ADR-0098: One Language, Multiple Interpreters

## Status

Proposed.

## Context

The platform currently has a separate tree-walk eval engine and an NBE typecheck engine, even though the `Language` data already stores both evaluation and type rules as Arrow `Scheme`s. The runtime and the typechecker are both doing the same `Scheme` walk with different primitive handlers. At the same time, the value and type spaces are not yet fully unified, and generic types are not represented consistently.

This ADR consolidates the design from ADR-0097 and ADR-0098 into a single architecture: one language, one `Scheme` language definition, multiple `Runtime` interpreters, and a shared tagged-value space for both runtime values and types.

A key correction: the platform currently treats `Integer`, `String`, `Number`, etc. as semantic types. They are not. The semantic model is **capabilities** (`Addable`, `Comparable`, `Serializable`, `Iterable`, `Lengthable`, `Indexable`). A "data type" is a target-specific implementation of a capability set. The typechecker checks capabilities; the compiler picks the concrete representation for the chosen runtime (native, WASM, JS).

## Decision

### 1. Everything is an arrow

The unifying design principle is that **every surface construct is an arrow**. Handl has one syntactic category: the arrow. Values, types, capabilities, handlers, and effects are all arrows or tags produced by arrows.

- A **capability** is an arrow to a tag:

  ```handl
  cap Addable
  ```

  is sugar for:

  ```handl
  type Addable = () ~> tag.construct("Addable", {})
  ```

- An **operation** is an arrow signature:

  ```handl
  op add : (Addable, Addable) ~> Addable
  ```

  declares the arrow `add : (Addable, Addable) ~> Addable`.

- A **concrete type** is an arrow to a capability record:

  ```handl
  type Number = () ~> [Addable, Comparable, Serializable]
  ```

  The surface form `type Number impls Addable, Comparable, Serializable` is exactly that.

- A **parameterized type** is an arrow with parameters:

  ```handl
  type Vec = (T, n: Nat) ~> [Lengthable(n), Indexable(n)]
  ```

  The surface form is `type Vec<T, n: Nat> impls Lengthable(n), Indexable(n)`.

- A **handler** is an arrow to a handler record:

  ```handl
  handler MoneyHandler = () ~> Handler {
      add: (a, b) ~> Money { cents: a.cents + b.cents }
  }
  ```

- A `with` expression is an arrow transformer:

  ```handl
  with MoneyHandler { total = price1 + price2 }
  ```

  takes a `Handler` and a body arrow, and returns a body arrow where `Op` dispatch is augmented by the handler.

- A `handle` expression is a row-bracketing arrow:

  ```handl
  handle { with State(0) { ... } }
  ```

  takes an arrow with an effect row and returns an arrow with that row removed.

- `if`/`for`/`while` are arrows:
  - `if` lowers to `Choice`.
  - `for` lowers to `Iter`.
  - `while` lowers to `Loop` + `Choice`.

The arrow syntax `~>` is the only arrow constructor. There is no `->`.

### 2. `plugin-engine` is the base `Scheme` language + `Runtime` trait crate

`plugin-engine` owns the `Scheme` data and the `Runtime` trait. It does not own an interpreter implementation. Each `Runtime` is a separate interpreter for the same `Scheme` language.

```rust
pub trait Runtime {
    type Value;
    fn run(
        &self,
        scheme: &Scheme,
        incoming: Self::Value,
        ctx: &RuntimeCtx,
    ) -> Result<Self::Value, LangError>;
}

/// The evaluation context passed to every `run` call.
pub struct RuntimeCtx<'a> {
    pub ast: &'a Ast,
    pub lang: &'a Language,
    pub current_node: &'a AstNode,
    pub node_ref: AstRef,
}
```

`RuntimeCtx` carries the AST, the `Language`, and the current node. It is needed to resolve `Child` roles, read `Op` payloads, look up eval/type schemes by node kind, and report errors with span information.

- `plugin-engine-exec` is the native tree-walk `Runtime`.
- `plugin-engine-typecheck` is the type `Runtime`.
- WASM, JS, compiler, analyzer, optimizer are more `Runtime` impls.

The `Runtime` is the interpreter. It is not a flat `name -> function` registry; it is a composition of **handlers**, where each handler provides capabilities for a value family. A `Runtime` can support or not support any `Scheme` combinator. If it sees a `Scheme` it does not implement, it returns an error.

### 3. All primitives and combinators are optional, per-runtime

There is no hardcoded primitive set. A `Runtime` provides a set of **handlers**, each implementing capabilities for a value family. When the interpreter sees `Op { name }`, it looks up the `op` declaration to find the required input/output capabilities, then dispatches to the handler that provides those capabilities for the incoming value's `impls` tags.

- `ValueRuntime` (native) may include a `NumberHandler` (for `Addable`, `Comparable`, `Serializable`), a `StringHandler`, a `ListHandler`, etc., plus the structural combinators `Seq`, `First`, `Choice`, `App`, `Loop`, `Iter`.
- `TypeRuntime` may support `synth`, `check`, `unify`, `lookup`, `construct`, `match_type`, `Seq`, `First`, `Choice`, `App`, `Iter`, but **not** `Loop`.
- Termination is structural: the type runtime simply does not implement `Loop`.
- `Loop` (`ArrowLoop`) is available only in runtimes that choose to implement it.

### 4. The data-driven arrow-handler algebra

The `Language` data is the **algebraic signature**:

- `type` declarations define value families (carriers) and their `impls` (sort membership).
- `op` declarations define operation arrows with input/output sorts.
- `Scheme` is the term language of the free arrow category over that signature.
- A `Runtime` is a **handler algebra**: a family of models, one per value family, interpreting the `op` signature.

```text
Language signature Σ = (type tags, op arrows)
Free arrow category F(Σ)
Scheme             = a term in F(Σ)
Runtime algebra A  = { A_T } for each type tag T
                    where A_T interprets the ops whose input sorts are in T.impls
Runtime map        = a functor F(Σ) → A
```

A `Runtime` is not a single interpreter; it is a **composition of handlers**. Each handler is an algebra for one value family. A `NumberHandler` provides `Addable`, `Comparable`, `Serializable` for `Number`. A `ListHandler` provides `Iterable`, `Lengthable` for any `List<T>`, and calls the `T` handler for element operations. An `add` operation works on any value whose `impls` includes `Addable`, because the dispatch is by capability, not by operation name.

### 5. `Scheme` calculus

The canonical `Scheme` language contains:

```text
Arr, Op, Seq, First, Choice, Loop, App, Iter, Child
```

- `Arr` is the pure arrow/constant combinator (Sanada's `arr`).
- `Op` is the named effect operation (Sanada's `op`).
- `Seq`, `First`, `Choice`, `Loop`, `App`, `Iter`, `Child` are the structural arrow combinators.

This extends the six-combinator arrow calculus of ADR-0052 (Sanada's arrow-handler calculus) with `Iter`, a bounded iteration combinator. `Iter` is a principled exception: not all handlers (typechecker, analyzer, theorem prover) can support `Loop`, so bounded iteration is a first-class kernel form. Runtimes may support `Iter` without `Loop`, and vice versa.

Surface `iter` is the core form. Surface `for` (JS-style) is sugar over `Iter`.

Arrow lambdas use `~>`:

```handl
x ~> x + 1
```

There is no `->`; only `~>`.

### 6. `Scheme` shape

The `Scheme` enum is the calculus. New for this ADR are `Op` (named effect), `Iter` (bounded iteration), and `Child` (AST child reference).

```rust
pub enum Scheme {
    Arr { value: Value },                       // pure arrow / constant (Sanada's `arr`)
    Op { name: String, args: Vec<Scheme>, source: ArrSource, lit: Option<Value> },  // named effect (Sanada's `op`)
    Seq { fst: Box<Scheme>, snd: Box<Scheme> },
    First { inner: Box<Scheme> },
    Choice { l: Box<Scheme>, r: Box<Scheme> },
    Loop { body: Box<Scheme> },
    App { callee: Box<Scheme>, args: Vec<Scheme> },
    Iter { fuel: Box<Scheme>, predicate: Box<Scheme>, step: Box<Scheme>, initial: Box<Scheme> },
    Child { role: String },
}
```

`Child { role }` is resolved through `lang.child_role_index(node.kind, &role)` to find the actual child `AstRef`, and then `lang.eval[child.kind]` is the sub-`Scheme` to run. The child runs with the current `RuntimeCtx` updated to the child node.

### 7. Surface arrow combinators are `Scheme` sugar

The surface forms desugar to the `Scheme` calculus. A new `Scheme::Child { role }` (or equivalent) is added so a `Scheme` can reference an AST child as a sub-`Scheme` to be run with a fresh incoming value.

- `arr op` → `Op { name: "$payload" }`.
- `first f` → `First { inner: Child { role: "arrow" } }`.
- `second f` → `Seq(First(Child "arrow"), swap)`.
- `left f` → `Choice { l: Child "arrow", r: Op { name: "id" } }`.
- `right f` → `Choice { l: Op { name: "id" }, r: Child "arrow" }`.
- `f +++ g` → `Choice(Seq(Child "left", Op { name: "either_left" }), Seq(Child "right", Op { name: "either_right" }))`.
- `f app x` → `App { callee: Child "arrow", args: [Child "argument"] }`.

Pure wrapper ops (`either_left`, `either_right`, `swap`, `id`) are `Op` native operations interpreted by the native `Runtime`, not hardcoded `Scheme` combinators.

### 8. Bounded repetition

`Iter` is the only bounded repetition combinator, with shape `Iter { fuel, predicate, step, initial }` where each field is a `Scheme`. Surface `iter` is the core form. `for` is sugar over `Iter`. `while` is sugar over `Loop` + `Choice`.

```handl
iter (fuel, predicate, step, initial)
for (i from 0 to n) body
while (cond) body
```

The `for` lowering is approximately:

```handl
iter (
    n,
    (i) ~> i < n,
    (i) ~> (i + 1, body),
    0
)
```

The exact state representation (tuple, record, or single value) is left to the `Runtime`.

The `while` lowering is approximately:

```handl
Loop(
    Choice(
        (s) ~> if cond(s) then Left(body(s)) else Right(s),
        id
    )
)
```

It is available only to runtimes supporting `Loop`.

### 9. Types are capability tags

Types are not a separate `Type` ADT, not user-forgeable JSON objects, and not concrete data types like `Integer` or `String`. A type is a **capability tag** minted by `tag.construct` and inspected by `tag.match_type`. The `__tag__` key is reserved, so user code cannot forge a type.

```handl
type Number = () ~> tag.construct("Number", { impls: [Addable, Comparable, Serializable] })
```

`Number` vs `Integer` is a representation hint for the compiler, not a semantic boundary. A function like `add` requires `Addable`; the typechecker checks that the value's tag has the `Addable` capability. The compiler then chooses the target-specific representation (native `i64`, WASM `i32`, JS `Number`) from the capabilities and the target.

### 10. `data` is removed; inductive types are ordinary `type` values

There is no `data` keyword. `type` is the canonical type declaration. Inductive types are just ordinary handl values in the type runtime, but constructors are not forged records; they are `plugin-tag` values minted by the `tag.tag` host capability.

```handl
type Nat = Zero || Succ(Nat)

Zero = () ~> tag.tag("Zero", {})
Succ = (pred) ~> tag.tag("Succ", { pred: pred })
Nat  = (x) ~> tag.match_type(x, [
    ("Zero", () ~> True),
    ("Succ", (p) ~> Nat(p))
])
```

The `||` in the right-hand side is parsed as `alt` nodes, the same mechanism used by the removed `data` keyword.

### 11. Generics are `Generic` tagged applications over capabilities

`List` by itself is a first-class type-constructor value. `List<Number>` is a `Generic` value wrapping the constructor and an argument that is a capability-bearing concrete tag. `match_type` dispatches on the `constructor` tag and checks that the `args` satisfy the constructor's required capabilities.

```handl
List<Number>
```

lowers to:

```text
tag.construct("Generic", {
    constructor: tag.construct("List", {}),
    args: [Number],
    impls: [tag.construct("Lengthable", {}), tag.construct("Iterable", {})]
})
```

`List` is a first-class value. `List<Number>` is an application. The `Generic` tag is the only mechanism needed for type application.

### 12. Surface syntax details

#### `type` — type declaration

`type` is the canonical declaration. It comes in three forms:

```handl
type Addable                         -- capability (same as `cap Addable`)
type Number = () ~> [Addable, Comparable, Serializable]
type Vec = (T, n: Nat) ~> [Lengthable(n), Indexable(n)]
type Nat = Zero || Succ(Nat)
```

`cap` is retained as an alias for `type` with no `impls` clause. All `type`/`cap` declarations desugar to `tag.construct` values in the type runtime.

#### `op` — operation declaration

```handl
op add : (Addable, Addable) ~> Addable
```

The `op` keyword declares an arrow in the `Language` signature. It desugars to a `type`/`tag.construct` for the operation and a `type` rule recording its input/output capabilities:

```text
add → tag.construct("op.add", {
    params: [Addable, Addable],
    return: Addable
})
```

#### `handler` — handler value

```handl
handler MoneyHandler = () ~> Handler {
    add: (a, b) ~> Money { cents: a.cents + b.cents },
    compare: (a, b) ~> a.cents < b.cents
}
```

A handler is a `Handler` tagged value, which is a record of `op` names to `Scheme` arrows. It can be passed, returned, and stored.

#### `with` — scope a handler

```handl
with MoneyHandler {
    total = price1 + price2
}
```

`with` is an arrow transformer: it takes a `Handler` and a body arrow, pushes the handler onto the `RuntimeCtx` handler stack, runs the body, and pops the handler. `Op` dispatch checks the handler stack before the value's `impls`.

#### `handle` — bracket an effect row

```handl
handle {
    with State(0) {
        x = get()
        put(x + 1)
        x
    }
}
```

`handle` is a row-bracketing arrow: the inner body may carry an effect row, but the full `handle` expression removes it. The `Runtime` saves the ambient `effect_row`, runs the body, and restores it.

### 13. Design principle: keep effects structural and handlers composable

The `Scheme` calculus is for control-flow and structural operations whose meaning is shared across runtimes: `Seq`, `First`, `Choice`, `Loop`, `App`, `Iter`, `Child`.

`Op` is for named operations, but an `Op` is **not** an opaque string. Its meaning comes from:

1. The `op` declaration in `Language` data (input/output capabilities).
2. The `Runtime`'s handlers, each of which provides those capabilities for a value family.

The test for which side a construct belongs on is: **can a tool transform, analyze, or reason about it without knowing the target runtime?** If yes, it belongs in `Scheme`; if its meaning is determined by a runtime-specific handler, it belongs in `Op`.

This is what keeps Handl from collapsing into an opaque effect-assembly language. `Scheme` is the one shared arrow calculus; `Op` is the interface through which composable handlers supply capabilities.

### 14. Frontiers

These extensions are natural consequences of the data-driven arrow-handler algebra. They are not required for the initial implementation, but the core design already supports them.

#### 14.1 First-class handlers

A handler is a record of `impls`. It becomes a first-class value. The surface syntax is `with H { body }`:

```handl
handler MoneyHandler = () ~> Handler {
    add: (a, b) ~> Money { cents: a.cents + b.cents }
}

with MoneyHandler {
    total = price1 + price2
}
```

A handler value is a `tag.construct` value:

```text
tag.construct("Handler", {
    impls: {
        add: Scheme,
        compare: Scheme,
        ...
    }
})
```

The `impls` map is a record from `op` name to a `Scheme` arrow.

`with H { body }` lowers to a new `Scheme` combinator:

```rust
With {
    handler: Scheme,
    body: Scheme,
}
```

`handle { ... }` brackets a `With` region:

```rust
Handle { body: Scheme }
```

`Handle` is the dynamic boundary that restores the previous handler stack.

`RuntimeCtx` gains a `handler_stack: Vec<HandlerValue>`. `With` pushes; `Handle` pop/restores. `Op` dispatch now checks:

```text
1. Top of handler_stack for an `impls` entry matching the op name.
2. The incoming value's tag `impls` list.
3. The `Runtime`'s primitive handlers.
```

A handler `Scheme` runs with the **same `Runtime`**, but the `Op` dispatch checks the handler stack first. This makes handlers first-class and composable.

#### 14.2 Resource and linear types

A `Linear` capability means a value must be used exactly once. `Disposable` means it has a finalization operation.

```handl
cap Linear
cap Disposable

op open : (Path) ~> File
op close : (File) ~> Unit
op borrow : (File) ~> Borrowed<File>

type File impls Linear, Disposable
```

Linear values are ordinary values, but the `Linear` tag in `impls` tells the `Runtime` to track ownership. A `File` tag value carries `impls: [Linear, Disposable]`.

`Op` gains an optional `mode`:

```rust
Op { name, args, mode: Consume | Borrow, ... }
```

`close(file)` is `Op { name: "close", args: [file], mode: Consume }`. `borrow(file)` is `Op { name: "borrow", args: [file], mode: Borrow }`. The mode is a runtime-facing hint derived from the `op` declaration.

`RuntimeCtx` gains a `linear_env`. On `Op` dispatch with `mode: Consume` on a `Linear` value, the `Runtime` removes the value from `linear_env` and runs the handler. If it is not there, it is a use-after-consume error. `Borrow` does not remove; it creates a non-owning view. The type `Runtime` does the same check statically.

#### 14.3 Dependent capabilities

`type` and `op` may carry value parameters. `impls` may depend on values in the payload.

```handl
cap Lengthable(n: Nat)
cap Indexable(n: Nat)

type Vec<T, n: Nat> impls Lengthable(n), Indexable(n)

op length : (Lengthable(n)) ~> Nat
```

A parameterized `type` is an arrow to its `impls`:

```handl
Vec = (T, n: Nat) ~> [Lengthable(n), Indexable(n)]
```

So `Vec<Number, 10>` is `Vec(Number, 10)`, which evaluates to:

```text
[tag.construct("Lengthable", { n: 10 }), tag.construct("Indexable", { n: 10 })]
```

The type value is then stored as:

```text
tag.construct("Generic", {
    constructor: tag.construct("Vec", {}),
    args: [Number, 10],
    impls: [tag.construct("Lengthable", { n: 10 }), tag.construct("Indexable", { n: 10 })]
})
```

`Op` dispatch with dependent capabilities becomes pattern-matching on the `impls` payload. For `length(vec)` where `vec` is `Vec<Number, 10>`, the `Runtime` matches the `Lengthable(n)` pattern, extracts `n = 10`, and returns it.

The `Runtime` treats `impls` not as a set of strings but as a set of **patterns**. A handler for `Vec` registers patterns like `Lengthable(n)` and `Iterable` (which is generic over `n`). Dispatch matches the most specific pattern.

#### 14.4 Effect rows

Every arrow type carries an effect row. `arr` is pure. `Op` adds an effect. `handle` removes an effect.

```handl
op get : (State<s>) ~> s
op put : (State<s>, s) ~> Unit

counter = handle {
    with State(0) {
        x = get()
        put(x + 1)
        x
    }
}
```

Arrow types with rows:

```text
(A, B) ~> C                 -- pure
(A, B) ~> C with (State, Console)  -- effectful
```

The row is a **set** of capabilities. Row polymorphism means `(A, B) ~> C with (State)` is a subtype of `(A, B) ~> C with (State, Console)`.

The `op` declaration in `Language` data includes an `effects` field:

```handl
op get : (State<s>) ~> s with State
op put : (State<s>, s) ~> Unit with State
```

`arr f` has an empty row. `Seq(f, g)` has the union of `f`'s and `g`'s rows. `First`, `Choice`, `Loop`, `Iter`, `App` propagate the row of their sub-arrows.

`with State(0) { ... }` lowers to `With` that adds `State` to the ambient row for `body`. `handle { ... }` lowers to `Handle` that brackets the row: the inner `body` may have `State`, but the full `handle` expression removes it.

`RuntimeCtx` gains an `effect_row` set. `With H { ... }` adds `H`'s capabilities to the row. `Handle { ... }` saves the row, runs the body, restores. `Op` dispatch checks that the `op`'s required `effects` are in the ambient `effect_row` (or in the value's `impls` for value-indexed effects like `State<s>`). Missing effects are a runtime/compile-time error.

The type `Runtime` does row inference and subtyping:

```handl
pure : (Number, Number) ~> Number
add  : (Number, Number) ~> Number with Addable
```

`pure` can be used anywhere. `add` can only be used where `Addable` is in scope.

### 15. Why this is not theoretically novel

The individual pieces come from published work:

- **Sanada (2024)** — arrow calculus with operations and handlers, where handlers are homomorphisms of A-algebras.
- **Brachthäuser et al. / Effekt (2020)** — effects as capabilities, capability-passing, lightweight effect polymorphism.
- **Cambria** — parametrized/generic handlers with resource abstraction.
- **Graded algebraic theories** — capabilities as sorts, `impls` as sort membership.

The contribution is the **data-driven combination**: one `Scheme` language, one `Language` signature, and a family of `Runtime` handler algebras, all driven by data rather than hardcoded in the engine.

## Consequences

### Positive

- One language, multiple runtimes. Adding a new interpretation (compiler, optimizer, analyzer) means adding a new `Runtime`, not a new engine.
- The `Scheme` language is the single source of truth; each `Runtime` is free to support or not support each combinator.
- Termination is structural: the type runtime simply does not implement `Loop` or other unbounded combinators.
- Types and values share the same `TaggedValue` representation.
- Types are capability tags; concrete data types are implementation hints for the compiler.
- A value's runtime representation (native `i64`, WASM `i32`, JS `Number`) is selected from its capabilities and the target, not from the surface type name.
- Generics are just another tagged value, handled by `construct` and `match_type`.
- New value families and capabilities can be added as `type`/`cap`/`op` declarations without engine changes.

### Negative

- Requires defining `Scheme` and the `Runtime` trait in `plugin-engine`.
- Requires porting `plugin-engine-exec-tree-walk` and `plugin-engine-typecheck-nbe` to standalone `Runtime` impls.
- Requires adding `Iter` and `Scheme::Child` to the calculus.
- Requires updating `handl-kit-data-canonical` grammar/ast/eval to remove `data` and support `type`, `cap`, `op`, `iter`, `for`.
- Requires migrating `types.yaml` to produce capability tags and `Generic` applications.

## Migration order

1. Define `Scheme` and the `Runtime` trait in `crates/plugin-engine`.
2. Port `plugin-engine-exec-tree-walk` to a `Runtime` impl over `TaggedValue`.
3. Port `plugin-engine-typecheck-nbe` to a `Runtime` impl for the type interpreter.
4. Add `Iter` to the `Scheme` calculus and surface `iter`/`for` syntax.
5. Add `Scheme::Child` (or equivalent) and desugar surface `arr`/`first`/`second`/`left`/`right`/`+++`/`app` into `Scheme` sugar.
6. Remove the `data` keyword from canonical language data; make `type` the canonical type declaration.
7. Add `cap` and `op` declarations and the `impls` clause to `type`.
8. Define canonical capabilities in `handl-packages/type` (`Addable`, `Comparable`, `Serializable`, `Iterable`, `Lengthable`, `Indexable`) and migrate `types.yaml` to use capability tags.
9. Migrate `types.yaml` to use `Generic` for type application.
10. Remove `plugin-engine-exec-tree-walk` and `plugin-engine-typecheck-nbe` once the runtimes are stable.
