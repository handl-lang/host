# An N-Dimensional Arrow Calculus for Programmable Computation and Rewriting

## Abstract

We propose a programming language based on a single arrow calculus that is recursively applied across dimensions. Ordinary programs are arrows at dimension 1. Transformations between programs are arrows at dimension 2. Transformations between transformations occupy dimension 3, and so on.

Rather than introducing separate languages for programming, rewriting, proofs, and metaprogramming, the language uses the same arrow calculus at every dimension. This provides a uniform foundation for generic rewriting, proof-carrying transformations, and optimization of transformation systems themselves.

Algebraic effects are expressed using Sanada-style `op` and `handle`, while dimensionality is represented by grades. Dimension 1 is implicit; higher dimensions are explicitly marked.

## 1. Core Idea

The fundamental construct is an arrow:

```ts
f: A -> B
```

Dimension 1 is the default, equivalent to:

```ts
f: A -[1]-> B
```

Higher-dimensional arrows connect parallel cells:

```ts
f: A -[1]-> B
g: A -[1]-> B

r: f -[2]-> g
```

A transformation is therefore not a special compiler object. It is simply another program at a higher dimension.

The same composition operators apply at every dimension:

```ts
f >>> g
f &&& g
f ||| g
first(f)
```

This gives a recursive structure:

```text
[1] programs
 ↓
[2] transformations
 ↓
[3] transformations of transformations
 ↓
[4] ...
```

## 2. Effects

Effects use algebraic operations and handlers:

```ts
Store<K, V> := {
  op read: K -> V
  op write: (K, V) -> Unit
}
```

Implementations are ordinary typed values:

```ts
DatabaseStore: Store<UserId, User>
```

Generic computations depend on the abstract operation algebra:

```ts
getUser<S: Store<UserId, User>> :=
  id => S.read(id)
```

An implementation is selected through a handler:

```ts
handle Store with DatabaseStore
```

Thus abstraction and implementation remain separate while the compiler retains the algebraic structure necessary for effect reasoning.

## 3. Rules as Programs

A rewrite rule is a higher-dimensional arrow:

```ts
cache:
  (S.read >>> S.read) -[2]-> S.read
```

Its type expresses the transformation it performs.

Rules can themselves be composed:

```ts
cache >>> otherRule
```

and transformations of rules occupy the next dimension:

```ts
metaRule:
  cache -[3]-> optimizedCache
```

Consequently, the language requires no independent rule language.

## 4. Programmable Dimensions

The dimension marker may be generalized from a fixed natural number to an expression:

```ts
f: A -[G]-> B
```

where `G` is itself represented in the arrow calculus.

This makes dimensional structure programmable rather than merely fixed. Dimension-polymorphic abstractions can therefore operate uniformly over programs, transformations, and higher transformations.

Conceptually:

```ts
transform<N>(x: A -[N]-> B)
```

can describe transformations independent of the particular dimensional level.

A statically evaluable grade system preserves compile-time reasoning while allowing programs to compute structural information about other programs.

## 5. Generic Rewriting

Because transformations are themselves programs, rewriting can be dimension-polymorphic.

The same machinery can operate on:

```text
programs
rules
meta-rules
proofs
transformations of proofs
...
```

This enables **meta-optimization**.

An optimizer can optimize programs:

```text
Program -> Program
```

while another higher-dimensional transformation can optimize the optimizer itself:

```text
Optimizer -> Optimizer
```

No separate metaprogramming mechanism is fundamentally required.

## 6. Static Reasoning

Higher-dimensional arrows can serve as typed evidence for transformations:

```ts
Equal (S.read >>> S.read) S.read
```

or directly as transformation cells:

```ts
(S.read >>> S.read) -[2]-> S.read
```

Normalization and proof-directed rewriting allow the compiler to reason about properties such as:

* effect equivalence
* idempotence
* independence
* parallelizability
* cacheability
* invalidation
* retryability
* optimization correctness

The compiler therefore becomes capable of **effect reasoning and verified transformation**, rather than merely effect tracking.

## 7. Central Principle

The language is built around one principle:

> **Everything is an arrow, all the way up.**

Programs, transformations, proofs, optimization strategies, and transformations of those strategies share one compositional language.

Dimensions do not introduce new programming languages. They identify the level at which an arrow operates.

This creates a unified foundation for functional programming, algebraic effects, rewriting, metaprogramming, and higher-dimensional computation.

## 8. Research Questions

The primary formal questions are:

1. What are the precise typing and boundary rules for n-cells?
2. Which grade expressions are statically normalizable?
3. How are equality and rewriting represented and checked?
4. How does Sanada's operational semantics lift through dimensions?
5. Under what conditions can higher-dimensional transformations safely rewrite programs?
6. How can generic rewriting remain terminating and decidable?
7. What computational power and practical benefits arise from programmable dimensionality?

The goal is a language in which **computation and reasoning about computation are expressed by the same calculus**, recursively and statically.
