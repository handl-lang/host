+++
id = "ADR-0042"
title = "ADR-0042: Transformations as First-Class Values"
status = "open"
priority = "medium"
+++

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


## Why

ADR-0041 made `Language` a first-class value and `eval` a handl-level function. The operator that worked on level N became a value at level N+1: `apply` operated on functions, so functions became values; `eval` operated on languages, so languages became values.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Composable compilers.** A compiler pipeline is function composition
- **Category of languages is first-class.** Languages are objects,
- **Optimizable optimizations.** An optimizer is data; you can
- **Semantic interop as a value.** A translator is a `Language ->
- **Runtime specialization.** A compiler can be specialized to a

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0042-transformations-as-first-class-values.md`](../design/0042-transformations-as-first-class-values.md).
