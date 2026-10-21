# Graded algebraic theory in Handl

This document explains the theoretical frame used by the Handl platform. It is written for anyone who wants to understand _why_ the machinery is shaped the way it is, not how to use it. For the architecture decision, see ADR-0098.

## The one-sentence idea

Handl is a **category-graded arrow theory**. Every construct is an arrow; every arrow is graded by capabilities; and a `Model` is just a model of that graded theory.

## Algebraic theories

An algebraic theory has three things:

1. **Sorts** — sets of values.
2. **Operations** — functions on those sets.
3. **Equations** — laws those functions must obey.

For example, the theory of monoids has one sort `M` and two operations:

```text
e : M
mul : M × M → M
```

plus associativity and unit laws. A **model** of the theory is a concrete choice of a set for `M` and functions for `e` and `mul`. A concrete monoid is a model.

Plotkin and Power showed that **computational effects** can be treated as algebraic operations, and **effect handlers** are models of the corresponding theory.

## Graded algebraic theories

A _graded_ algebraic theory adds a **grade** to every operation and every term. The grade is an object in some indexing structure — usually a monoidal category or a category. The grade tracks extra structure: effects, resources, permissions, or capabilities.

The classic example is graded state. The grade of `lookup_l` is the set `{l}`: this operation may access location `l`. The grade of `lookup_l` followed by `update_l` is still `{l}`. The grade of a compound computation is the union (or tensor) of the grades of its parts.

```text
grade(lookup_l)   = {l}
grade(update_l)   = {l}
grade(t1; t2)     = grade(t1) ∪ grade(t2)
```

This corresponds to a **graded monad**: a computation `T_m A` returns an `A` and may use effects in grade `m`.

## Category-graded theories

In a **category-graded** theory, the grades are not just a monoid; they are morphisms in a category. This lets grades express _dependencies_ or _state transitions_.

For example, a state can change type:

```text
lookup  : 1 → int
update  : int → 1
```

The grade of `lookup` is the morphism that takes a unit state and produces an `int` state. The grade of `update` does the opposite. This is the foundation of Kura et al.'s CatEff system.

## How Handl maps onto this

| Graded-theory concept | Handl construct                      |
| --------------------- | ------------------------------------ |
| Sort                  | value family                         |
| Grade                 | capability / effect                  |
| Carrier               | `Model::Value` for a sort            |
| Operation             | a graded arrow declaration           |
| Term                  | `Term` data in the `Scheme` calculus |
| Model                 | `Model` + handlers                   |
| Handler               | model for one sort                   |

### Sorts

A sort is a value family. Declaring that `Number` is a sort carrying the
grades `Addable`, `Comparable`, and `Serializable` is, semantically, an
arrow from unit to the grade row:

```text
Number : () ~> [Addable, Comparable, Serializable]
```

### Grades

A grade is a tag used in grade constraints — semantically a sort with no
carrier. Declaring `Addable` as a grade is the same value as declaring it
as a sort; the grade spelling is a stylistic marker that the sort is meant
to appear in constraints.

### Operations

An operation is a graded arrow. Its inputs and outputs are sorts; its
grade is the capability it requires:

```text
add : (A, A) ~[Addable]~> A
```

This says: `add` takes two values of any sort `A` that carries the grade
`Addable`, and returns an `A`. The operation itself is graded by `Addable`.

The grade is _not_ the input sort. `Addable` is not a value you can pass
to `add`. It is a constraint on the sort of the inputs.

### Terms

A `Term` is a term in the free arrow category over the `Language` signature. The `Term` enum in `hnl-engine` is the calculus:

```rust
pub enum Term {
    Arr { value: Value },       // pure constant arrow
    Op { name: String, args: Vec<Term>, ... },  // graded operation
    Seq { ... },                // composition
    First { ... },              // product routing
    Choice { ... },             // sum routing
    Loop { ... },               // feedback
    App { ... },                // dynamic application
    Iter { ... },               // bounded iteration
    Child { ... },              // AST child reference
}
```

These are the only things the engine knows. Surface constructs
desugar to these terms.

### Models

A `Model` is a model of the graded theory. It assigns to each sort a carrier (e.g. `TaggedValue`) and to each grade a handler.

For `Number`, the `Model` has a `NumberHandler` that provides the operations for grades `Addable`, `Comparable`, and `Serializable`. For `String`, it has a `StringHandler`. For `List<T>`, it has a `ListHandler` that dispatches element operations to the handler for `T`.

When the interpreter sees `Op { name: "add" }` on a `Number` value, it:

1. Looks up the `add` signature: `(A, A) ~> A` with grade `Addable`.
2. Checks that `Number` carries `Addable` in its `grades`.
3. Dispatches to the `NumberHandler` for grade `Addable`.
4. Runs the handler and returns the result.

The same `add` operation works on `Integer`, `Float`, `Money`, or any other sort with `Addable`, because dispatch is by grade, not by operation name.

## First-class handlers

A handler is itself a value — a record mapping operation names to `Term`
arrows. Installing one pushes it onto the `ModelCtx` handler stack; `Op`
dispatch checks the stack before the value's `grades`. This lets a local
handler shadow the default one without changing the engine. Bracketing a
region with a handler removes the corresponding effect from the grade row
and restores the previous stack on exit.

## Effect rows

Every arrow has a **grade row**. Pure arrows have an empty row. An `Op`
adds a grade; installing a handler removes one:

```text
pure : (Number, Number) ~> Number
add  : (Number, Number) ~[Addable]~> Number
```

`pure` can be used anywhere. `add` can only be used where `Addable` is in
the ambient grade row. The row is a set, so `(A) ~[Addable]~> B` is a
subtype of `(A) ~[Addable, Comparable]~> B`.

## Dependent grades

Grades can depend on values. A `Lengthable(n)` grade indexed by a `Nat`,
carried by a `Vec<T, n>` sort, lets an operation extract the index from
the grade:

```text
length : (A) ~[Lengthable(n)]~> Nat
```

`Vec<Number, 10>` carries `Lengthable(10)`; `length(vec)` matches the
`Lengthable(n)` pattern, extracts `n = 10`, and returns it. The type
`Model` checks `Vec<A, 3>` does not unify with `Vec<B, 4>`. No separate
dependent-type language is needed; it falls out of value-indexed grades.

## Frontier theory integrations

The graded arrow theory is not the end. Several other theories layer on top of it.

### Coeffects

Graded monads track what a computation _does_ to the context. Graded comonads track what it _needs_ from the context. Handl grades are already coeffects: `add` _needs_ `Addable`. A full coeffect system would add a `needs` row alongside the effect row.

### Substructural grades

Grades can be a semiring of usage counts:

```text
0 = not available
1 = use once
ω = use unlimited
k = use at most k
```

`Linear` is just `1`. `Borrow` is a low-usage grade.

### Session types

A session type is a protocol encoded as a grade morphism. A `File` has a session `Open -> (Read* -> Close) | (Write* -> Close)`. Each operation is a step in the protocol.

### Refinement grades

A grade can be a logical predicate: `Positive = { n: Number | n > 0 }`. The type `Model` can check these with an SMT solver.

### Guarded recursion

The `Later` modality makes coinductive types total. `Stream<T>` is guarded by `Later`. You pay a `Later` grade to inspect the tail. `Loop` is replaced by `Lob` induction.

## Why this matters

The graded-theory frame explains several design choices:

1. **One language, multiple runtimes.** The `Language` data is the signature; each `Model` is a model. The value and type runtimes are different models of the same theory.
2. **No hardcoded primitives.** The engine only knows the arrow calculus. The `Model` provides handlers for whatever grades are declared.
3. **Termination is structural.** The type `Model` omits the `Loop` grade. It simply does not provide a model for unbounded recursion.
4. **Capabilities, not concrete types.** `Number` vs `Integer` is a choice of carrier, not a semantic boundary. The semantics are the grades.
5. **First-class handlers.** Handlers are models for one sort, and handler scoping is a model operation.

## References

- S. Kura, _Graded Algebraic Theories_ (2020).
- S. Kura et al., _Category-Graded Algebraic Theories and Effect Handlers_ (CatEff, 2022).
- T. Sanada, _Algebraic Effects and Handlers for Arrows_ (2024).
- J. Brachthäuser et al., _Effects as Capabilities: Effect Handlers and Lightweight Effect Polymorphism_ (Effekt, 2020).
- J. Hughes, _Generalising Monads to Arrows_ (2000).

See also ADR-0098 for the architecture decision and ADR-0052 for the arrow-calculus kernel. [ADR-0106](https://github.com/handl-lang/host/blob/project/adrs/0106-object-arrow-kernel.md) grounds the grades of this theory as ordinary objects in the one object/arrow world.
