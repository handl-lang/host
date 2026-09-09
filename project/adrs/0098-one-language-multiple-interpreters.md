# ADR-0098: One Language, Multiple Interpreters

## Status

Proposed.

## Context

The platform currently has a separate tree-walk eval engine and an NBE typecheck engine, even though the `Language` data already stores both evaluation and type rules as Arrow `Term`s. The runtime and the typechecker are both doing the same `Term` walk with different primitive handlers. At the same time, the value and type spaces are not yet fully unified, and generic types are not represented consistently.

This ADR consolidates the design from ADR-0097 and ADR-0098 into a single architecture: one language, one `Scheme` language definition, multiple `Model` interpreters, and a shared tagged-value space for both runtime values and types.

A key correction: the platform currently treats `Integer`, `String`, `Number`, etc. as semantic types. They are not. The semantic model is **grades** (`Addable`, `Comparable`, `Serializable`, `Iterable`, `Lengthable`, `Indexable`). A "data type" is a target-specific implementation of a grade set. The typechecker checks grades; the compiler picks the concrete representation for the chosen runtime (native, WASM, JS).

## Decision

### 1. Everything is an arrow, and the arrow language is graded

The unifying design principle is that **every surface construct is an arrow** in a **category-graded arrow theory**. Handl has one syntactic category: the arrow. Values, types, grades, handlers, and effects are all arrows or tags produced by arrows.

- A **grade** is an arrow to a grade tag:

  ```handl
  grade Addable
  ```

  is sugar for:

  ```handl
  sort Addable = () ~> tag.construct("Addable", {})
  ```

- A **sort** is an arrow to a grade record:

  ```handl
  sort Number = () ~> [Addable, Comparable, Serializable]
  ```

  The surface form `sort Number grades Addable, Comparable, Serializable` is exactly that.

- A **parameterized sort** is an arrow with parameters:

  ```handl
  sort Vec = (T, n: Nat) ~> [Lengthable(n), Indexable(n)]
  ```

  The surface form is `sort Vec<T, n: Nat> grades Lengthable(n), Indexable(n)`.

- An **operation** is a graded arrow signature:

  ```handl
  op add<A: Addable> : (A, A) ~> A
  ```

  declares the arrow `add` taking any sort `A` that carries the grade `Addable`, and returning `A`.

- A **handler** is an arrow to a handler record:

  ```handl
  handler NumberHandler = () ~> Handler {
      add: (a, b) ~> Number { native: a.native + b.native }
  }
  ```

- A `with` expression is an arrow transformer:

  ```handl
  with NumberHandler { total = price1 + price2 }
  ```

  takes a `Handler` and a body arrow, and returns a body arrow where `Op` dispatch is augmented by the handler.

- A `handle` expression is a row-bracketing arrow:

  ```handl
  handle { with State(0) { ... } }
  ```

  takes an arrow with a grade row and returns an arrow with that row removed.

- `if`/`for`/`while` are arrows:
  - `if` lowers to `Choice`.
  - `for` lowers to `Iter`.
  - `while` lowers to `Loop` + `Choice`.

The arrow syntax `~>` is the only arrow constructor. There is no `->`.

### 2. The graded-theory map

Handl is a **category-graded arrow theory** in the sense of Kura et al. (CatEff):

| Graded theory | Handl | Surface keyword |
|---|---|---|
| Sort | value family | `sort` |
| Grade | capability / effect | `grade` |
| Carrier | `Model::Value` for a sort | concept |
| Operation | `op` declaration | `op` |
| Term | `Scheme`/`Term` arrow | concept |
| Model | `Model` + handlers | concept |
| GradeOfATerm | effect / grade row | `gradeof` (future) |
| Handler | model for a sort | `handler` |

The `Model` is a **model** of the graded theory. A handler is a **model** for one sort. The `Term` is a **term** in the free model. The `Scheme` is the language of such terms.

### 3. `plugin-engine` is the base `Term` language + `Model` trait crate

`plugin-engine` owns the `Term` data and the `Model` trait. It does not own an interpreter implementation. Each `Model` is a separate model for the same `Scheme` language.

```rust
pub trait Model {
    type Value;
    fn run(
        &self,
        term: &Term,
        incoming: Self::Value,
        ctx: &ModelCtx,
    ) -> Result<Self::Value, LangError>;
}

/// The evaluation context passed to every `run` call.
pub struct ModelCtx<'a> {
    pub ast: &'a Ast,
    pub lang: &'a Language,
    pub current_node: &'a AstNode,
    pub node_ref: AstRef,
}
```

`ModelCtx` carries the AST, the `Language`, and the current node. It is needed to resolve `Child` roles, read `Op` payloads, look up eval/type `Term`s by node kind, and report errors with span information.

- `plugin-engine-exec` is the native tree-walk `Model`.
- `plugin-engine-typecheck` is the type `Model`.
- WASM, JS, compiler, analyzer, optimizer are more `Model` impls.

The `Model` is a model of the graded theory. It is not a flat `name -> function` registry; it is a composition of **handlers**, where each handler provides grades for a sort. A `Model` can support or not support any `Term` combinator. If it sees a `Term` it does not implement, it returns an error.

### 4. All primitives and combinators are optional, per-runtime

There is no hardcoded primitive set. A `Model` provides a set of **handlers**, each implementing grades for a sort. When the interpreter sees `Op { name }`, it looks up the `op` declaration to find the required input/output grades, then dispatches to the handler that provides those grades for the incoming value's `grades` tags.

- `ValueModel` (native) may include a `NumberHandler` (for `Addable`, `Comparable`, `Serializable`), a `StringHandler`, a `ListHandler`, etc., plus the structural combinators `Seq`, `First`, `Choice`, `App`, `Loop`, `Iter`.
- `TypeModel` may support `synth`, `check`, `unify`, `lookup`, `construct`, `match_type`, `Seq`, `First`, `Choice`, `App`, `Iter`, but **not** `Loop`.
- Termination is structural: the type runtime simply does not implement `Loop`.
- `Loop` (`ArrowLoop`) is available only in runtimes that choose to implement it.

### 5. The data-driven arrow-handler algebra

The `Language` data is the **graded algebraic signature**:

- `sort` declarations define value families (carriers) and their `grades` (sort membership).
- `op` declarations define graded operation arrows with input/output sorts.
- `Term` is the term language of the free arrow category over that signature.
- A `Model` is a **graded model**: a family of handlers, one per sort, interpreting the `op` signature.

```text
Language signature Σ = (sorts, grades, graded op arrows)
Free arrow category F(Σ)
Term               = a term in F(Σ)
Model model A    = { A_T } for each sort T
                     where A_T interprets the ops whose input grades are in T.grades
Model map        = a functor F(Σ) → A
```

A `Model` is not a single interpreter; it is a **composition of handlers**. Each handler is a model for one sort. A `NumberHandler` provides `Addable`, `Comparable`, `Serializable` for `Number`. A `ListHandler` provides `Iterable`, `Lengthable` for any `List<T>`, and calls the `T` handler for element operations. An `add` operation works on any value whose `grades` includes `Addable`, because the dispatch is by grade, not by operation name.

### 6. `Scheme` / `Term` calculus

The canonical `Term` language contains:

```text
Arr, Op, Seq, First, Choice, Loop, App, Iter, Child
```

- `Arr` is the pure arrow/constant combinator (Sanada's `arr`).
- `Op` is the named graded effect operation (Sanada's `op`).
- `Seq`, `First`, `Choice`, `Loop`, `App`, `Iter`, `Child` are the structural arrow combinators.

This extends the six-combinator arrow calculus of ADR-0052 (Sanada's arrow-handler calculus) with `Iter`, a bounded iteration combinator. `Iter` is a principled exception: not all handlers (typechecker, analyzer, theorem prover) can support `Loop`, so bounded iteration is a first-class kernel form. Models may support `Iter` without `Loop`, and vice versa.

Surface `iter` is the core form. Surface `for` (JS-style) is sugar over `Iter`.

Arrow lambdas use `~>`:

```handl
x ~> x + 1
```

There is no `->`; only `~>`.

### 7. `Term` shape

The `Term` enum is the calculus. New for this ADR are `Op` (named graded effect), `Iter` (bounded iteration), and `Child` (AST child reference).

```rust
pub enum Term {
    Arr { value: Value },                       // pure arrow / constant (Sanada's `arr`)
    Op { name: String, args: Vec<Term>, source: ArrSource, lit: Option<Value> },  // named graded effect (Sanada's `op`)
    Seq { fst: Box<Term>, snd: Box<Term> },
    First { inner: Box<Term> },
    Choice { l: Box<Term>, r: Box<Term> },
    Loop { body: Box<Term> },
    App { callee: Box<Term>, args: Vec<Term> },
    Iter { fuel: Box<Term>, predicate: Box<Term>, step: Box<Term>, initial: Box<Term> },
    Child { role: String },
}
```

`Child { role }` is resolved through `lang.child_role_index(node.kind, &role)` to find the actual child `AstRef`, and then `lang.eval[child.kind]` is the sub-`Term` to run. The child runs with the current `ModelCtx` updated to the child node.

### 8. Surface arrow combinators are `Term` sugar

The surface forms desugar to the `Term` calculus. A new `Term::Child { role }` (or equivalent) is added so a `Term` can reference an AST child as a sub-`Term` to be run with a fresh incoming value.

- `arr op` → `Op { name: "$payload" }`.
- `first f` → `First { inner: Child { role: "arrow" } }`.
- `second f` → `Seq(First(Child "arrow"), swap)`.
- `left f` → `Choice { l: Child "arrow", r: Op { name: "id" } }`.
- `right f` → `Choice { l: Op { name: "id" }, r: Child "arrow" }`.
- `f +++ g` → `Choice(Seq(Child "left", Op { name: "either_left" }), Seq(Child "right", Op { name: "either_right" }))`.
- `f app x` → `App { callee: Child "arrow", args: [Child "argument"] }`.

Pure wrapper ops (`either_left`, `either_right`, `swap`, `id`) are `Op` native operations interpreted by the native `Model`, not hardcoded `Term` combinators.

### 9. Bounded repetition

`Iter` is the only bounded repetition combinator, with shape `Iter { fuel, predicate, step, initial }` where each field is a `Term`. Surface `iter` is the core form. `for` is sugar over `Iter`. `while` is sugar over `Loop` + `Choice`.

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

The exact state representation (tuple, record, or single value) is left to the `Model`.

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

### 10. Sorts are grade records

Sorts are not a separate `Type` ADT, not user-forgeable JSON objects, and not concrete data types like `Integer` or `String`. A sort is a **grade record** minted by `tag.construct` and inspected by `tag.match_type`. The `__tag__` key is reserved, so user code cannot forge a sort.

```handl
sort Number = () ~> tag.construct("Number", { grades: [Addable, Comparable, Serializable] })
```

`Number` vs `Integer` is a representation hint for the compiler, not a semantic boundary. A function like `add` requires `Addable`; the typechecker checks that the value's tag has the `Addable` grade. The compiler then chooses the target-specific representation (native `i64`, WASM `i32`, JS `Number`) from the grades and the target.

### 11. The `data` keyword is removed; inductive sorts are ordinary `sort` values

There is no `data` keyword. `sort` is the canonical sort declaration. Inductive sorts are just ordinary handl values in the type runtime, but constructors are not forged records; they are `plugin-tag` values minted by the `tag.tag` host capability.

```handl
sort Nat = Zero || Succ(Nat)

Zero = () ~> tag.tag("Zero", {})
Succ = (pred) ~> tag.tag("Succ", { pred: pred })
Nat  = (x) ~> tag.match_type(x, [
    ("Zero", () ~> True),
    ("Succ", (p) ~> Nat(p))
])
```

The `||` in the right-hand side is parsed as `alt` nodes, the same mechanism used by the removed `data` keyword.

### 12. Generics are `Generic` tagged applications over grades

`List` by itself is a first-class sort-constructor value. `List<Number>` is a `Generic` value wrapping the constructor and an argument that is a grade-bearing concrete tag. `match_type` dispatches on the `constructor` tag and checks that the `args` satisfy the constructor's required grades.

```handl
List<Number>
```

lowers to:

```text
tag.construct("Generic", {
    constructor: tag.construct("List", {}),
    args: [Number],
    grades: [tag.construct("Lengthable", {}), tag.construct("Iterable", {})]
})
```

`List` is a first-class value. `List<Number>` is an application. The `Generic` tag is the only mechanism needed for sort application.

### 13. Surface syntax details

#### `sort` — sort declaration

`sort` is the canonical declaration. It comes in three forms:

```handl
sort Addable                                  -- grade (same as `grade Addable`)
sort Number = () ~> [Addable, Comparable, Serializable]
sort Vec = (T, n: Nat) ~> [Lengthable(n), Indexable(n)]
sort Nat = Zero || Succ(Nat)
```

`grade` is retained as an alias for `sort` with no `grades` clause. All `sort`/`grade` declarations desugar to `tag.construct` values in the type runtime.

#### `grade` — grade declaration

```handl
grade Addable
```

is sugar for:

```handl
sort Addable = () ~> tag.construct("Addable", {})
```

A grade is a sort with no carrier; it is a tag used in `op` grade constraints and sort `grades` clauses.

#### `op` — graded operation declaration

```handl
op add<A: Addable> : (A, A) ~> A
```

The `op` keyword declares a graded arrow in the `Language` signature. It desugars to a `sort`/`tag.construct` for the operation and a `sort` rule recording its input/output grades:

```text
add → tag.construct("op.add", {
    params: ["A"],
    constraints: [Addable],
    return: "A"
})
```

For a concrete sort:

```handl
op add : (Number, Number) ~> Number grade Addable
```

The `grade` clause on the arrow is the grade of the operation. The operation can only be used where the ambient grade row contains `Addable`.

#### `handler` — handler value

```handl
handler NumberHandler = () ~> Handler {
    add: (a, b) ~> Number { native: a.native + b.native }
}
```

A handler is a `Handler` tagged value, which is a record of `op` names to `Term` arrows. It can be passed, returned, and stored.

#### `with` — scope a handler

```handl
with NumberHandler {
    total = price1 + price2
}
```

`with` is an arrow transformer: it takes a `Handler` and a body arrow, pushes the handler onto the `ModelCtx` handler stack, runs the body, and pops the handler. `Op` dispatch checks the handler stack before the value's `grades`.

#### `handle` — bracket a grade row

```handl
handle {
    with State(0) {
        x = get()
        put(x + 1)
        x
    }
}
```

`handle` is a row-bracketing arrow: the inner body may carry a grade row, but the full `handle` expression removes it. The `Model` saves the ambient `grade_row`, runs the body, and restores it.

### 14. Design principle: keep effects structural and handlers composable

The `Term` calculus is for control-flow and structural operations whose meaning is shared across runtimes: `Seq`, `First`, `Choice`, `Loop`, `App`, `Iter`, `Child`.

`Op` is for named graded operations, but an `Op` is **not** an opaque string. Its meaning comes from:

1. The `op` declaration in `Language` data (input/output sorts and grade).
2. The `Model`'s handlers, each of which provides those grades for a sort.

The test for which side a construct belongs on is: **can a tool transform, analyze, or reason about it without knowing the target runtime?** If yes, it belongs in `Term`; if its meaning is determined by a runtime-specific handler, it belongs in `Op`.

This is what keeps Handl from collapsing into an opaque effect-assembly language. `Term` is the one shared arrow calculus; `Op` is the interface through which composable handlers supply grades.

### 15. Theoretical grounding

The individual pieces come from published work:

- **Hughes / Lindley, Wadler, Yallop** — the arrow calculus.
- **Sanada (2024)** — arrow calculus with operations and handlers, where handlers are homomorphisms of A-algebras.
- **Brachthäuser et al. / Effekt (2020)** — effects as capabilities, capability-passing, lightweight effect polymorphism.
- **Kura et al. / CatEff (2022)** — category-graded algebraic theories and effect handlers.
- **Graded algebraic theories (Kura, 2020)** — grades as a monoidal/category index on algebraic theories.

The contribution is the **data-driven combination**: one `Term` language, one `Language` signature, and a family of `Model` handler models, all driven by data rather than hardcoded in the engine.

### 16. Frontiers

These extensions are natural consequences of the data-driven arrow-handler algebra. They are not required for the initial implementation, but the core design already supports them.

#### 16.1 First-class handlers

A handler is a record of `op` implementations. It becomes a first-class value. The surface syntax is `with H { body }`:

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
        add: Term,
        compare: Term,
        ...
    }
})
```

The `impls` map is a record from `op` name to a `Term` arrow.

`with H { body }` lowers to a new `Term` combinator:

```rust
With {
    handler: Term,
    body: Term,
}
```

`handle { ... }` brackets a `With` region:

```rust
Handle { body: Term }
```

`Handle` is the dynamic boundary that restores the previous handler stack.

`ModelCtx` gains a `handler_stack: Vec<HandlerValue>`. `With` pushes; `Handle` pop/restores. `Op` dispatch now checks:

```text
1. Top of handler_stack for an `impls` entry matching the op name.
2. The incoming value's tag `grades` list.
3. The `Model`'s primitive handlers.
```

A handler `Term` runs with the **same `Model`**, but the `Op` dispatch checks the handler stack first. This makes handlers first-class and composable.

#### 16.2 Resource and linear types

A `Linear` grade means a value must be used exactly once. `Disposable` means it has a finalization operation.

```handl
grade Linear
grade Disposable

op open : (Path) ~> File
op close : (File) ~> Unit
op borrow : (File) ~> Borrowed<File>

sort File grades Linear, Disposable
```

Linear values are ordinary values, but the `Linear` tag in `grades` tells the `Model` to track ownership. A `File` tag value carries `grades: [Linear, Disposable]`.

`Op` gains an optional `mode`:

```rust
Op { name, args, mode: Consume | Borrow, ... }
```

`close(file)` is `Op { name: "close", args: [file], mode: Consume }`. `borrow(file)` is `Op { name: "borrow", args: [file], mode: Borrow }`. The mode is a runtime-facing hint derived from the `op` declaration.

`ModelCtx` gains a `linear_env`. On `Op` dispatch with `mode: Consume` on a `Linear` value, the `Model` removes the value from `linear_env` and runs the handler. If it is not there, it is a use-after-consume error. `Borrow` does not remove; it creates a non-owning view. The type `Model` does the same check statically.

#### 16.3 Dependent grades

`sort` and `op` may carry value parameters. `grades` may depend on values in the payload.

```handl
grade Lengthable(n: Nat)
grade Indexable(n: Nat)

sort Vec<T, n: Nat> grades Lengthable(n), Indexable(n)

op length<A: Lengthable(n)> : (A) ~> Nat
```

A parameterized `sort` is an arrow to its `grades`:

```handl
Vec = (T, n: Nat) ~> [Lengthable(n), Indexable(n)]
```

So `Vec<Number, 10>` is `Vec(Number, 10)`, which evaluates to:

```text
[tag.construct("Lengthable", { n: 10 }), tag.construct("Indexable", { n: 10 })]
```

The sort value is then stored as:

```text
tag.construct("Generic", {
    constructor: tag.construct("Vec", {}),
    args: [Number, 10],
    grades: [tag.construct("Lengthable", { n: 10 }), tag.construct("Indexable", { n: 10 })]
})
```

`Op` dispatch with dependent grades becomes pattern-matching on the `grades` payload. For `length(vec)` where `vec` is `Vec<Number, 10>`, the `Model` matches the `Lengthable(n)` pattern, extracts `n = 10`, and returns it.

The `Model` treats `grades` not as a set of strings but as a set of **patterns**. A handler for `Vec` registers patterns like `Lengthable(n)` and `Iterable` (which is generic over `n`). Dispatch matches the most specific pattern.

#### 16.4 Effect rows

Every arrow type carries a grade row. `arr` is pure. `Op` adds a grade. `handle` removes a grade.

```handl
op get : () ~> s grade State<s>
op put : (s) ~> Unit grade State<s>

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

The row is a **set** of grades. Row polymorphism means `(A, B) ~> C with (State)` is a subtype of `(A, B) ~> C with (State, Console)`.

The `op` declaration in `Language` data includes a `grades` field:

```handl
op get : () ~> s with State<s>
op put : (s) ~> Unit with State<s>
```

`arr f` has an empty row. `Seq(f, g)` has the union of `f`'s and `g`'s rows. `First`, `Choice`, `Loop`, `Iter`, `App` propagate the row of their sub-arrows.

`with State(0) { ... }` lowers to `With` that adds `State` to the ambient row for `body`. `handle { ... }` lowers to `Handle` that brackets the row: the inner `body` may have `State`, but the full `handle` expression removes it.

`ModelCtx` gains a `grade_row` set. `With H { ... }` adds `H`'s grades to the row. `Handle { ... }` saves the row, runs the body, restores. `Op` dispatch checks that the `op`'s required `grades` are in the ambient `grade_row` (or in the value's `grades` for value-indexed grades like `State<s>`). Missing grades are a runtime/compile-time error.

The type `Model` does row inference and subtyping:

```handl
pure : (Number, Number) ~> Number
add  : (Number, Number) ~> Number with Addable
```

`pure` can be used anywhere. `add` can only be used where `Addable` is in scope.

### 17. Theoretical frontiers beyond Koka/Frank/Effekt

The following are open theoretical questions, not just engineering:

1. **Category-graded arrow calculus.** Sanada has arrows and handlers; Kura has category-graded theories and handlers. The combination is not yet formalized. The main difficulty is how grade morphisms interact with `Choice` and `Loop`.
2. **Dependent grades.** `Lengthable(n: Nat)` is a grade indexed by a value. Current category-graded theories have grades as objects/morphisms, not term-dependent families. A full dependent-grade system is open.
3. **Multiple models for one signature.** The same `Term` has a value model, a type model, a compiler model, etc. The categorical relationship between these models (homomorphisms, logical predicates, realizability) is not worked out.
4. **Termination by grade omission.** Proving that a `Model` that omits `Loop` always terminates, and that `Iter` provides a normalizing bounded-iteration grade.
5. **Linear grades with handlers.** Combining graded linear types with effect handlers and arrows is an active area, but no canonical system exists.
6. **Grade reflection.** Making `gradeof(term)` a first-class value inside the language, and keeping it consistent with the static row inference.
7. **Session types as grade morphisms.** Using grade morphisms for protocols (sending/receiving typed data) as in CatEff, but over arrows.

### 18. Frontier theory integrations

These are not just open problems; they are existing theories that can be layered onto the graded arrow theory.

#### 18.1 Coeffects and graded comonads

Graded monads track what a computation *does* to the context (effects). Graded comonads track what a computation *requires* from the context (coeffects). In Handl, grades are coeffects: `add` requires `Addable`.

```handl
op add : (A, A) ~> A  needs  Addable
```

A full coeffect system (Orchard & Chadvadze) would add a `needs` row alongside the `with` row. The `needs` row tracks context requirements; the `with` row tracks context changes. A term has both:

```text
add : (Number, Number) ~> Number  needs  Addable  with  Addable
```

#### 18.2 Substructural grades

Grades can form a **semiring** of usage counts, not just a set of capabilities:

```text
0  = not available
1  = use exactly once
ω  = use unlimited
k  = use at most k times
```

```handl
grade File usages { open: 1, read: ω, close: 1 }
```

This subsumes `Linear`. It is the foundation of Granule and bounded linear logic. `Linear` becomes `usage 1`, `Borrow` becomes a lower usage grade.

#### 18.3 Session types as grade morphisms

A session type is a protocol of send/receive/branch actions. It can be encoded as a category-graded effect (Orchard & Yoshida):

```handl
grade FileSession = Open -> (Read* -> Close) | (Write* -> Close)

sort File grades FileSession
```

`op open` has grade `1 -> Open`. `op read` has grade `Open -> Open`. `op close` has grade `Open -> End`. Composing operations with mismatched session grades is a type error. This is much more precise than `Linear` for resources with protocols.

#### 18.4 Refinement grades

A grade can be a logical predicate:

```handl
grade Positive = { n: Number | n > 0 }
grade NonZero  = { n: Number | n != 0 }
grade Sorted   = { xs: List | sorted(xs) }

sort PositiveNumber grades Number, Positive

op divide<A: NonZero> : (Number, A) ~> Number
```

The grade is a refinement of the sort. The type `Model` can use an SMT solver to check grades. This is the LiquidHaskell / March approach, but over graded arrows.

#### 18.5 Guarded recursion and modal grades

The `Later` modality (Nakano) allows coinductive types without `Loop`:

```handl
grade Later

sort Stream<T> grades Later
sort CoNat     grades Later

op head : (Stream<T>) ~> T          grade Later
op tail : (Stream<T>) ~> Stream<T>  grade Later
```

A `Stream<T>` is a guarded coinductive value. You cannot access the tail without paying a `Later` grade. `Iter` is finite unfolding. `Loop` is replaced by `Lob` induction over `Later`. This gives a total language with infinite data and no `Loop`.

### 19. More frontier theories

These are additional theories that layer onto the graded arrow substrate.

#### 19.1 Algebraic subtyping

Dolan's algebraic subtyping gives a foundation for all subtyping: sorts, grades, and arrows. In Handl, a sort with more grades is a subtype:

```handl
sort Animal grades HasLegs, HasName
sort Dog    grades Animal, CanBark
```

`Dog` is a subtype of `Animal` because its grade set includes `Animal`. `walk : (A: HasLegs) ~> Unit` accepts `Dog` because `Dog grades HasLegs`. No nominal inheritance is needed; subtyping is set inclusion on grades.

#### 19.2 Modal types and staged computation

A modality is a grade on the stage of computation:

```handl
grade Staged
grade Now

op quote : (A) ~> Code<A>  grade Staged
op splice : (Code<A>) ~> A  grade Now
```

`quote` lifts a term into a `Code<A>` value that can be inspected or compiled. `splice` injects code back into the current stage. The `Compiler` runtime provides the model for `Staged`; the `Exec` runtime provides `Now`.

#### 19.3 Probabilistic programming

A `Sample` grade for randomness:

```handl
grade Sample

op sample  : () ~> A            grade Sample
op observe : (A, A) ~> Unit     grade Sample

infer = handle {
    with Prior {
        x = sample()
        observe(x, data)
        x
    }
}
```

The `Model` model for `Sample` is an inference strategy: exact enumeration, rejection sampling, or MCMC. The same `Term` can be interpreted by different inference runtimes.

#### 19.4 Information flow and security labels

Grades as confidentiality or integrity labels:

```handl
grade Public
grade Secret
grade Declassify

sort Password grades Secret

op hash       : (Secret) ~> Public
op declassify : (A: Secret) ~> Public  grade Declassify
```

A `TypeModel` ensures no `Secret` flows into `Public` except through an explicit `Declassify` operation. The security policy is just a grade discipline.

#### 19.5 Cost and resource grades

Track time, space, or energy:

```handl
grade Cost(n: Nat)

op loop : (A) ~> A  grade Cost(1)
```

A `WorstCase` runtime can compute the grade of a `Term` to give an upper bound. A `Profile` runtime can measure actual cost. `Cost` is a dependent grade over natural numbers.

#### 19.6 Taint tracking

```handl
grade Tainted
grade Sanitized

op user_input : () ~> String  grade Tainted
op escape_check : (Tainted) ~> Sanitized

render = (html) ~> html  needs  Sanitized
```

`Tainted` strings cannot flow to `render` unless they pass `escape_check`. The grade tracks the taint; the handler removes it.

#### 19.7 Full dependent sorts

The current `Vec<T, n: Nat>` is a dependent sort. The frontier is full Π and Σ sorts:

```handl
sort Pair = (A, B: A -> Sort) ~> (a: A) × B(a)
sort Pi   = (A, B: A -> Sort) ~> (a: A) -> B(a)
```

This turns Handl into a dependently-typed arrow language. The `TypeModel` becomes a proof assistant; the `ValueModel` just evaluates.

#### 19.8 Proof-carrying grades

A grade can be a proof or a predicate:

```handl
grade Even(n: Nat) = { n | n % 2 == 0 }
sort EvenNat grades Nat, Even

op half : (n: EvenNat) ~> Nat
```

The `TypeModel` can generate proof terms. The `Compiler` can erase them. The `TheoremProver` runtime can verify them. The same grade is used by all three runtimes for different purposes.

#### 19.9 Concurrent separation logic

For shared mutable state and locks:

```handl
grade Lock(l)
grade HeapPiece(l)

op acquire : (Lock(l)) ~> HeapPiece(l)
op release : (HeapPiece(l)) ~> Unit  grade Lock(l)
```

The grade is a resource invariant. The `Lock` handler enforces CSL-style ownership. This complements session types, which cover linear channels.

#### 19.10 Domain theory and pointed orders

A denotational `Model` assigns domains to sorts and continuous functions to operations:

```text
Number^⊥ = lifted natural numbers
add      = strict continuous function
```

This is the foundation for proving that `Loop` is the least fixed point, that `Iter` converges, and that `Model` models are continuous. A `DenotationalModel` can coexist with a concrete `ValueModel` as different models of the same `Term`.

### 20. Meta-frontiers: 2-categorical arrows and homotopy type theory

The graded arrow theory is one-dimensional: sorts are 0-cells, terms are 1-cells. The next enrichment adds 2-cells (rewrites, optimizations, proofs), and the ultimate enrichment is a directed homotopy type theory.

#### 20.1 2-categorical arrows

A 2-category has three levels:

```text
0-cells: sorts
1-cells: arrows (Term combinators)
2-cells: rewrites / optimizations / proofs between arrows
```

A `Term` is no longer just `f : A ~> B`; it can also include `α : f => g`, a rewrite from arrow `f` to arrow `g`. Examples of 2-cells:

```text
First(f >>> g) => First(f) >>> First(g)          -- functoriality
arr(f) >>> arr(g) => arr(f . g)                   -- arrow law
Loop(First(f) >>> (g × id)) => Loop(...)          -- trace law
inline(add, NumberHandler) => native_add          -- optimization
for (i from 0 to n) body  =>  iter(n, pred, step, init)  -- desugaring
```

The `Language` data can declare not only `sort`, `grade`, and `op`, but also **laws** (2-cells) and **rewrite rules**. A `Model` becomes a **2-functor**: it sends 0-cells to carriers, 1-cells to functions, and 2-cells to equalities or transformations.

The existing `plugin-engine-transform` already produces rewrites. A 2-categorical view makes those rewrites part of the `Term` language itself:

- `ValueModel` maps a 2-cell to an equality proof: `f` and `g` are observationally equivalent.
- `CompilerModel` maps a 2-cell to a target-level transformation: replace `f` with `g`.
- `OptimizerModel` is a 2-functor from `Term` to itself: rewrite the `Term` before running.
- `TheoremProverModel` maps a 2-cell to a proof step in the type model.

This means the optimizer is not a separate engine; it is a `Model` into the `Term` 2-category. The `desugar` and `transform` passes are 2-cells.

#### 20.2 The `Language` as a presented 2-category

A `Language` is a presentation of a 2-category:

- `sort` declarations = 0-cells
- `op` declarations = generating 1-cells
- `laws` and `rewrite` declarations = generating 2-cells
- `Model` = 2-functor from that presented 2-category to a target 2-category

For example, a law could be:

```handl
law add_assoc : (a, b, c) ~> add(add(a, b), c) => add(a, add(b, c))
```

The `ValueModel` verifies this at runtime by checking `(a + b) + c == a + (b + c)`. The `TheoremProverModel` proves it from the handler algebra. The `CompilerModel` uses it to reorder instructions.

#### 20.3 Homotopy type theory

HoTT makes the next step: equality itself is structure. A `sort` is a space. A value of `sort Number` is a point. An equality `a =_Number b` is a path between points. A proof that `a == b` is a path, and a proof that two proofs are equal is a higher path.

The key axiom is **univalence**: equivalent sorts are equal.

```handl
sort Number
sort Integer

equiv Number Integer   -- there is an equivalence between the carriers

by univalence:  Number = Integer
```

Univalence means an operation `add : Number ~> Number` can be transported to `add : Integer ~> Integer` along the equivalence. The compiler does not need a special case; the transport is structure.

For Handl, this has several consequences:

1. **Grades are propositions.** A grade `Addable` is a predicate over a sort. In HoTT, predicates are h-level 1 types (subsingletons). The `grades` list is a vector of proofs.

2. **`op` is a fibrant map.** An operation `add : (A, A) ~> A with Addable` is a function in a fibration. The input and output sorts are fibers; the grade is the base.

3. **`Generic` is a dependent sum.** `List<Number>` is a dependent pair `(constructor, args)` of the `List` constructor and the `Number` argument. `impls` / `grades` are fibers over that pair.

4. **Models are univalent functors.** A `Model` respects all paths. If two `Term`s are equal (there is a 2-cell), their interpretations are equal. This makes compiler correctness a consequence of functoriality.

5. **2-cells are paths.** The 2-categorical arrow theory is a truncation of a directed homotopy type theory. A rewrite `f => g` is a directed path. A `Model` is a functor that maps paths in the `Term` category to paths in the target category.

#### 20.4 Directed homotopy type theory as the limit

Directed HoTT (Riehl & Shulman) is the theory where types have not only undirected paths (equalities) but also directed paths (arrows). In Handl, `~>` is the directed path. The `Term` 2-category is a truncation of a directed (∞,1)-category:

- A `sort` is a type.
- An arrow `A ~> B` is a directed path.
- A rewrite `f => g` is a 2-path.
- A `Model` is a directed (∞,1)-functor to a target directed (∞,1)-category.

This is the theoretical limit of the design. The graded arrow theory is the 1-categorical fragment. Adding 2-cells makes it a 2-category. Adding univalence makes it HoTT. Adding directed paths makes it directed HoTT.

The practical consequence is that the `TypeModel` is not just a typechecker; it is a proof assistant. The `Compiler` is not just a translator; it is a functor. The `ValueModel` is not just an interpreter; it is a set-theoretic model. All of them are instances of one concept: a `Model` of the `Language` directed type theory.

## Consequences

### Positive

- One language, multiple runtimes. Adding a new interpretation (compiler, optimizer, analyzer) means adding a new `Model`, not a new engine.
- The `Term` language is the single source of truth; each `Model` is free to support or not support each combinator.
- Termination is structural: the type runtime simply does not implement `Loop` or other unbounded combinators.
- Types and values share the same `TaggedValue` representation.
- Sorts are grade records; concrete data types are implementation hints for the compiler.
- A value's runtime representation (native `i64`, WASM `i32`, JS `Number`) is selected from its grades and the target, not from the surface sort name.
- Generics are just another tagged value, handled by `construct` and `match_type`.
- New value families and grades can be added as `sort`/`grade`/`op` declarations without engine changes.

### Negative

- Requires defining `Term` and the `Model` trait in `plugin-engine`.
- Requires porting `plugin-engine-exec-tree-walk` and `plugin-engine-typecheck-nbe` to standalone `Model` impls.
- Requires adding `Iter` and `Term::Child` to the calculus.
- Requires updating `handl-kit-data-canonical` grammar/ast/eval to remove `data` and support `sort`, `grade`, `op`, `iter`, `for`.
- Requires migrating `types.yaml` to produce grade tags and `Generic` applications.

## Migration order

1. Define `Term` and the `Model` trait in `crates/plugin-engine`.
2. Port `plugin-engine-exec-tree-walk` to a `Model` impl over `TaggedValue`.
3. Port `plugin-engine-typecheck-nbe` to a `Model` impl for the type interpreter.
4. Add `Iter` to the `Term` calculus and surface `iter`/`for` syntax.
5. Add `Term::Child` (or equivalent) and desugar surface `arr`/`first`/`second`/`left`/`right`/`+++`/`app` into `Term` sugar.
6. Remove the `data` keyword from canonical language data; make `sort` the canonical sort declaration.
7. Add `sort`, `grade`, and `op` declarations and the `grades` clause to `sort`.
8. Define canonical grades in `handl-packages/type` (`Addable`, `Comparable`, `Serializable`, `Iterable`, `Lengthable`, `Indexable`) and migrate `types.yaml` to use grade tags.
9. Migrate `types.yaml` to use `Generic` for sort application.
10. Remove `plugin-engine-exec-tree-walk` and `plugin-engine-typecheck-nbe` once the runtimes are stable.
