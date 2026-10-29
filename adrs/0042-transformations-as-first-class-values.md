---
title: Transformations as First-Class Values
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Builds on [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md).
---

# ADR-0042: Transformations as First-Class Values

## Context

ADR-0041 made `Language` a first-class value and `eval` a handl-level
function. The operator that worked on level N became a value at level N+1:
`apply` operated on functions, so functions became values; `eval` operated
on languages, so languages became values.

The next operator up is the one that operates on languages: **a
transformation `Language -> Language` (or `Language -> Target`).**
Compilers, translators, optimizers, desugarers, partial evaluators,
capability strippers. Today these are fixed host processes — `cargo
build`, `tsc`, `esbuild`. Once `Language` is a value, a transformation is
just a function from values to values, and functions are already
first-class.

The move is the same move, one level up again:

| level | what's first-class | the operator                                 |
| ----- | ------------------ | -------------------------------------------- |
| 1     | functions          | `apply : (A -> B) -> A -> B`                 |
| 2     | languages          | `eval : Language -> Source -> Value`         |
| 3     | transformations    | `compose : (L -> L) -> (L -> L) -> (L -> L)` |

A compiler stops being a build step and becomes a value you bind, pass,
compose, and dispatch on. `compose(optimize, strip-io)` is a compiler
pipeline built at runtime from two values — the same shape as
`compose(f, g)` for functions.

### Why this follows from ADR-0041, not requires new infrastructure

ADR-0041 makes `Language` a value and `eval` a handl function. A
transformation `Language -> Language` is a function over a value — and
functions are already first-class (level 1). So level 3 is not new
machinery; it's level 1 applied to level 2. The only new thing is naming
the common transformations and making them available as handl values.

## Decision

Make transformations first-class handl values. A transformation is a
function `Language -> Language` (semantic-preserving) or `Language ->
Target` (compilation). Both are ordinary function values, composable via
ordinary function composition.

### 1. Transformations are handl function values

```handl
# a compiler is a value: Language -> Target
to-wasm:    type.any = (l: type.any) -> compile(l, "wasm");
to-native:  type.any = (l: type.any) -> compile(l, "native");

# an optimizer is a value: Language -> Language
optimize:   type.any = (l: type.any) -> language {
  ...l,
  eval: optimize-behaviors(l.eval),
};

# a capability stripper is a value: Language -> Language
strip-io:   type.any = (l: type.any) -> restrict(l, pure-caps);
```

### 2. Compiler pipelines are function composition

```handl
# compose transformations — pipeline as function composition
pipeline: type.any = compose(to-wasm, compose(optimize, strip-io));
safe-wasm-handl: type.any = pipeline(handl);
```

`compose(optimize, strip-io)` is a compiler pipeline built at runtime
from two values. No build system, no plugin chain, no fixed order. The
pipeline is data, inspectable and reorderable.

### 3. The category of languages is first-class

Languages are objects; transformations are morphisms. The category is
first-class — you can inspect it, compose in it, build functors over it.

```handl
# a functor: map a transformation across a family of languages
lang-map: type.any = (t: type.any, langs: type.any) -> {
  map(t, langs)
};

# apply one optimizer to every language in a kit family
optimized-family: type.any = lang-map(optimize, [handl, json-lang, toml-lang]);
```

This is not imposed abstraction. It is what the values already are once
languages and transformations are both first-class.

### 4. Optimizable optimizations

An optimizer is a `Language -> Language` transformation, which is data.
You can optimize the optimizer — specialize it for a target language,
fuse two passes, dead-code-eliminate a pass that is a no-op. The
meta-level is the same level, because transformations are values.

```handl
# specialize an optimizer for a specific language
specialized-opt: type.any = specialize(optimize, handl);

# fuse two passes into one
fused: type.any = fuse-passes(optimize, strip-io);
```

### 5. Semantic interop as a value

A translation between two languages is a `Language -> Language` function.
Polyglot programming stops being "two runtimes with a bridge" and becomes
"one value that translates."

```handl
# json-as-handl: a translator value
json-to-handl: type.any = (l: type.any) -> language {
  ...handl,
  grammar: json-grammar,
  eval: json-eval-bridged-to-handl-eval,
};

# compose translations: A -> B -> C
a-to-c: type.any = compose(b-to-c, a-to-b);
```

You can check, compose, and verify translations at runtime. Interop is a
value, not a bridge.

## Consequences

### Positive

- **Composable compilers.** A compiler pipeline is function composition
  over values, built at runtime. No build system, no fixed plugin order.
- **Category of languages is first-class.** Languages are objects,
  transformations are morphisms, composition is ordinary function
  composition. Functors over the category are `lang-map`.
- **Optimizable optimizations.** An optimizer is data; you can
  specialize, fuse, and dead-code-eliminate passes. The meta-level is
  the same level.
- **Semantic interop as a value.** A translator is a `Language ->
Language` function, composable and inspectable. Polyglot programming
  is one value, not two runtimes with a bridge.
- **Runtime specialization.** A compiler can be specialized to a
  specific language at runtime via ordinary partial application — the
  Futamura projections, now applied to transformations themselves.
- **No new infrastructure.** Level 3 is level 1 (first-class functions)
  applied to level 2 (first-class languages). The machinery already
  exists once ADR-0041 lands.

### Negative

- **Correctness is not guaranteed by composition.** Composing two
  semantic-preserving transformations preserves semantics only if both
  actually preserve semantics. The type system can track that a value is
  a transformation, but not that it is a _correct_ one. That is level 4
  (proofs) — see [ADR-0043](./0043-proofs-as-first-class-values.md).
- **Performance of composed transformations.** Naive composition may
  produce intermediate `Language` values that are immediately
  re-transformed. Fusion and specialization mitigate, but require the
  optimizer-on-optimizers machinery to be built.
- **Naming and curation.** The common transformations (`compile`,
  `optimize`, `restrict`, `extend`, `merge`) need to be exposed as
  handl-callable values, not just Rust functions. This is a surface-area
  task, not a deep change, but it is work.

## Implementation phases

### Phase 1 — Expose `compile` as a handl function

The minimal step: `compile : Language -> Target` is callable from handl.
This alone gives the self-hosting bootstrap (from ADR-0041) and runtime
specialization.

### Phase 2 — Expose the standard transformations

`optimize`, `restrict`, `extend`, `merge` (from ADR-0041) are already
`Language -> Language` functions internally; expose them as handl values.

### Phase 3 — Composition and `lang-map`

`compose` already works for functions; ensure it works uniformly for
`Language -> Language` and `Language -> Target`. Add `lang-map` for
families of languages.

### Phase 4 — Optimizable optimizations

Build the pass-fusion and specialization machinery. An optimizer is
data; specialize and fuse passes at runtime.

### Phase 5 — Semantic interop as values

Ship translator values for common pairs (json-as-handl, toml-as-handl).
Interop is a value, not a bridge.

## Related work

- [ADR-0041](./0041-languages-as-first-class-values.md) — Languages as
  First-Class Values (this ADR applies level 1 to level 2)
- [ADR-0043](./0043-proofs-as-first-class-values.md) — Proofs as
  First-Class Values (the next level: correctness of transformations)
- Category theory — objects and morphisms, composition, functors
- Futamura projections — specialization of interpreters into compilers,
  now applied to transformations themselves
- LLVM pass managers — composable compiler passes, but fixed at build
  time, not runtime values
