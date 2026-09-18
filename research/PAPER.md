# An N-Dimensional Arrow Calculus for Programmable Computation and Rewriting

## Abstract

We propose a programming language based on a single arrow calculus applied recursively across dimensions. Morphisms are the primitive computational objects: types are objects, and a morphism `A ~[1]~> B` is a transformation between them. Ordinary programs are 1-morphisms. Rewrite rules, laws, and proofs are 2-morphisms. Transformations between laws are 3-morphisms, and so on.

Two independent indices organize the calculus. The morphism dimension `n` in `~[n]~>` identifies what kind of cell is being constructed; the composition axis `k` in `>>>[k]` identifies along which direction cells compose. Higher-dimensional cells admit several composition directions — vertical, horizontal, and beyond — each with its own identity and interchange structure.

Rather than introducing separate languages for programming, rewriting, proofs, and metaprogramming, the language uses the same morphism mechanism at every dimension. This provides a uniform foundation for generic rewriting, proof-carrying transformations, and optimization of transformation systems themselves. Algebraic effects are expressed using Sanada-style `op` and `handle`; dependent structure is expressed through families with `Π`/`Σ` quantification. Dimension 1 is implicit; higher dimensions are explicitly marked.

## 1. Core Idea: Morphisms, All the Way Up

The fundamental construct is a morphism:

```ts
f : A ~[1]~> B
```

Types are objects; morphisms are transformations between them. Under a Curry–Howard reading, a proposition is an object and a proof is a morphism — the same judgment covers computation and evidence.

Dimension 1 is the default and may be elided:

```ts
f : A ~> B
```

Higher-dimensional morphisms connect parallel lower-dimensional cells:

```ts
f : A ~[1]~> B
g : A ~[1]~> B

α : f ~[2]~> g
```

A transformation is therefore not a special compiler object. It is simply a morphism at the next dimension.

(Earlier drafts wrote the dimension bracket `-[1]->`; we adopt the `~[n]~>` spelling of the object/arrow kernel.)

The same combinators apply at every dimension:

```ts
f >>> g
f &&& g
f ||| g
first(f)
```

This gives a recursive structure:

```text
[1] programs             (morphisms between objects)
 ↓
[2] transformations      (morphisms between 1-morphisms)
 ↓
[3] meta-transformations (morphisms between 2-morphisms)
 ↓
[4] ...
```

## 2. Polymorphic Dimension

The morphism formers are a single dimension-indexed family:

```ts
M[1]<A, B>    -- 1-morphisms between objects
M[2]<f, g>    -- 2-morphisms between parallel 1-morphisms
M[3]<α, β>    -- 3-morphisms between parallel 2-morphisms
```

The sugar `X ~[n]~> Y` denotes `M[n]<X, Y>`; the boundary of an n-morphism is always a pair of parallel (n−1)-morphisms.

Dimension is polymorphic: `n` is a parameter, not a fixed tier. Consequently the structural operations are defined once for all dimensions rather than per level.

Polymorphic identity:

```ts
id : M[n] → M[n]
```

`id` lifts any cell to the identity morphism on it; the dimension is inferred from context. There are no separate `id1`, `id2`, `id3`.

Dimension-polymorphic functoriality:

```ts
map : M<A, B> → M<F A, F B>
```

`map` operates at whatever morphism dimension is appropriate: on 1-morphisms it is the ordinary functor action; on 2-morphisms it is the induced transformation between transformed morphisms; and so on upward.

## 3. Composition Axes

Dimension and composition axis are distinct concepts:

- `~[n]~>` answers _what dimensional morphism is this?_
- `>>>[k]` answers _along which composition direction do we compose?_

At dimension 1 there is essentially one direction:

```ts
f >>>[0] g        -- plain `f >>> g` is shorthand for the default axis
```

At dimension 2 there are already two. Given `α : M[2]<F, F'>` and `β : M[2]<G, G'>`:

- **Vertical** composition stacks transformations along the morphism direction — `F ~[2]~> F' ~[2]~> F''` — written `α >>>[0] α'`.
- **Horizontal** composition pairs transformations across composition of their boundary morphisms — `F ~[2]~> F'` together with `G ~[2]~> G'` yields a 2-morphism between the composites `F >>> G` and `F' >>> G'` — written `α >>>[1] β`.

Spatially: axis 0 composes cells _along_ a structure; axis 1 composes them _across_ it. Higher dimensions add further compositional directions, and the coherence conditions between them.

The general form is `compose[k](a, b)` or `a >>>[k] b`, with axes `Nat`-indexed: `>>>[0]`, `>>>[1]`, `>>>[2]`, .... Unannotated `>>>` is shorthand for the default, inferred axis.

## 4. Laws are Higher Morphisms

The structural laws of the calculus are not declarations in a separate rule language. They are 2-morphisms:

```ts
rightUnit : (f >>> id) ~[2]~> f
assoc     : ((f >>> g) >>> h) ~[2]~> (f >>> (g >>> h))
```

An inhabitant of `M[2]` is simultaneously a rewrite rule, a proof, and a program transformation; the intended reading is supplied by context and by the handler that consumes it. Coherence between laws lives one dimension up, as an `M[3]` cell:

```ts
ω : α ~[3]~> β
```

Consequently the language requires no separate `laws` subsystem: `~[1]~>`, `~[2]~>`, `~[3]~>` are 1-, 2-, and 3-morphisms of a single mechanism. Whether a given law is inhabited by a strict equality or by a genuine higher cell is a coherence choice, not a syntactic distinction (§13).

## 5. Families and Dependent Structure

Dependent structure is expressed through families — type-valued structure over an object:

```ts
P : Family<A>          -- ≈ A ↝ Type
Π (x:A), P(x)
Σ (x:A), P(x)
```

For a morphism `f : A ~[1]~> B`, reindexing (substitution) pulls families on `B` back to families on `A`, and its adjoints give dependent sum and product along `f`:

```ts
Σ_f ⊣ f* ⊣ Π_f
```

Dependent quantification is therefore not a separate type-theoretic subsystem. It is structure transported along the same morphisms that compute.

## 6. Effects

Effects use algebraic operations and handlers:

```ts
Store<K, V> := {
  op read: K -> V
  op write: (K, V) -> Unit
}
```

Implementations are ordinary typed values:

```ts
DatabaseStore: Store<UserId, User>;
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

Thus abstraction and implementation remain separate while the compiler retains the algebraic structure necessary for effect reasoning. Effect annotations are independent of morphism dimension — an effect-tracked computation is still a 1-morphism; whether the grade bracket (`~[R]~>` in the object/arrow kernel) and the dimension bracket share one syntax is an open question (§13).

## 7. Rules as Programs

A rewrite rule is a 2-morphism:

```ts
cache : (S.read >>> S.read) ~[2]~> S.read
```

Its type expresses the transformation it performs: source and target are the parallel 1-morphisms forming its boundary.

Rules can themselves be composed, along whichever axis is appropriate:

```ts
cache >>>[0] otherRule
```

and transformations of rules occupy the next dimension:

```ts
metaRule : cache ~[3]~> optimizedCache
```

Consequently, the language requires no independent rule language.

## 8. Programmable Dimensions

The dimension index may be generalized from a fixed natural number to an expression:

```ts
f: M[G]<A, B>;
```

where `G` is itself represented in the calculus — dimensions are data. This makes dimensional structure programmable rather than merely fixed. Dimension-polymorphic abstractions can therefore operate uniformly over programs, transformations, and higher transformations.

Conceptually:

```ts
transform<N>(x : M[N]<A, B>)
```

can describe transformations independent of the particular dimensional level.

A statically evaluable dimension discipline preserves compile-time reasoning while allowing programs to compute structural information about other programs.

## 9. Generic Rewriting

Because transformations are themselves morphisms, rewriting can be dimension-polymorphic.

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

## 10. Static Reasoning

Higher-dimensional morphisms can serve as typed evidence for transformations:

```ts
Equal (S.read >>> S.read) S.read
```

or directly as cells:

```ts
(S.read >>> S.read) ~[2]~> S.read
```

Normalization and proof-directed rewriting allow the compiler to reason about properties such as:

- effect equivalence
- idempotence
- independence
- parallelizability
- cacheability
- invalidation
- retryability
- optimization correctness

The compiler therefore becomes capable of **effect reasoning and verified transformation**, rather than merely effect tracking.

## 11. Central Principle

The language is built around one principle:

> **Everything is a morphism, all the way up.**

Computation, proofs, laws, transformations, and higher laws share one mechanism — the morphism. The resulting syntax is correspondingly small:

```ts
f : A ~[1]~> B
α : f ~[2]~> g
ω : α ~[3]~> β

f >>>[0] g
α >>>[1] β

P : Family<A>
Π x:A. P(x)
Σ x:A. P(x)
```

Dimensions do not introduce new programming languages. They identify the level at which a morphism operates; axes identify the direction in which morphisms compose. This creates a unified foundation for functional programming, algebraic effects, rewriting, metaprogramming, and higher-dimensional computation.

## 12. Implementation Alignment

The architecture is largely realized in spirit by the existing kernel; what remains is formalization rather than new machinery.

The repository's `Term` calculus (`crates/plugin-engine/src/term.rs`) already embodies _dimension-as-expression_: the arrow formers `Arr` and `Op` carry `dim: Box<Term>` (defaulting to the constant `1`), so dimension is data inside the calculus rather than a static annotation. `Src`/`Tgt` project the boundary faces of a 2-cell, and `Rewrite` records a witnessed transformation — a precursor of `M[2]` inhabitants. The law-inference pass (`crates/handl-language/src/law_inference.rs`) already normalizes `Term` graphs to a fixpoint and decides law equality by comparing normal forms — a prototype of 2-morphism checking for the structural fragment.

Four engineering prerequisites remain before the `Term` kernel is literally the n-dimensional calculus:

1. **Arrows as values** — quoting, `App`, and currying must let morphisms be constructed, inspected, and applied as ordinary data.
2. **Axis slot on `Seq`** — `Term::Seq` currently carries only `fst`/`snd`; it must acquire the axis parameter of `>>>[k]`.
3. **Source/target tracking in types** — the type layer must record morphism boundaries so `M[n]<A, B>` can be checked.
4. **One dimension concept** — the effect-grade `dim` and the morphism dimension `n` must be merged, or explicitly separated.

## 13. Open Questions

The design is settled architecturally; the open work is formalization.

**Cells and composition**

1. What exactly is `M[n]` — a type former, a judgment, or a family of categories?
2. What are the typing rules for `>>>[k]`?
3. Which axes exist for a given `n`?
4. What identity and associativity structures exist at every level?
5. How are horizontal and vertical composition related, and which interchange and coherence laws hold between axes?

**Strictness and shape**

6. Are higher laws strict equalities or genuine higher morphisms? Note that `M[n]<A, B>` with `A, B ∈ M[n-1]` is already the **globular** presentation — the syntax implies globular shape; strict-vs-weak is the remaining choice.
7. What categorical structure results overall — strict, weak, globular, cubical?

**Interaction with the rest of the language**

8. How do `Family`, `Π`, and `Σ` interact with higher morphisms (e.g. families indexed by `M[2]` cells)?
9. Is there a real distinction between computation cells and proof/law cells, or only between how they are consumed?
10. How does Sanada's operational semantics lift through dimensions?

**Metatheory**

11. Which dimension expressions are statically normalizable?
12. Under what conditions can higher-dimensional transformations safely rewrite programs?
13. How can generic rewriting remain terminating and decidable?
14. What computational power and practical benefits arise from programmable dimensionality?

The goal is a language in which **computation and reasoning about computation are expressed by the same calculus**, recursively and statically.
