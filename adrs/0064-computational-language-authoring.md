---
title: Computational Language Authoring
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Depends on [ADR-0063 — Scheme-Core Completeness](./0063-scheme-core-completeness.md)
  (the core must be able to describe handl before handl is used to author
  languages). Refines [ADR-0044 — Language as Data and Runtime
  Split](./0044-language-as-data-and-runtime-split.md) Phase 8 and
  [ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
  §4 (language combinators as handl functions).
---


# ADR-0064: Computational Language Authoring

## Decision


### 1. Fragment constructors as handl functions

Expose the fragment types (`TokenKindDef`, `GrammarRow`, `EvalSchemeDef`,
`AstShapeDef`, `CapabilityDecl`, `OptionSchema`) as handl-callable constructors.
A handl program builds fragments by calling functions, not by writing YAML:

```handl
# Instead of YAML:
#   - id: plus
#     position: infix
#     lbp: 20
#     rbp: 20
#
# Computational:
binary-op = fn(name, lbp, rbp) {
  grammar-row {
    id: name,
    position: "infix",
    lbp: lbp,
    rbp: rbp,
    shape: ["expr", "expr"],
  }
};
plus  = binary-op("plus",  20, 20);
minus = binary-op("minus", 20, 20);
star  = binary-op("star",  40, 40);
```

The `binary-op` function abstracts over a repetitive pattern. YAML cannot do
this. This is the value.

### 2. Fragment-set combinators as handl functions

`Language::merge` (ADR-0044 §1) is exposed as a handl function. Fragment sets
are built by composition:

```handl
arith-ops = fragment-set([plus, minus, star, slash]);
compare-ops = fragment-set([eq, neq, lt, gt]);
all-ops = merge(arith-ops, compare-ops);
```

`extend`/`merge`/`restrict` (ADR-0041 §4) are handl functions over fragment
sets, not host-side registry mutations. The result is a `Language` value —
data, not a side effect.

### 3. Parameterized language templates

A handl function can produce a `Language` parameterized by inputs:

```handl
make-arith-lang = fn(ops, builtins) {
  language {
    tokens:  arith-tokens(ops),
    grammar: arith-grammar(ops),
    eval:    arith-eval(ops, builtins),
  }
};

calc-lang = make-arith-lang([plus, minus, star], [sin, cos, sqrt]);
```

This is the abstraction YAML cannot provide. A family of related languages
(arith calculator, scientific calculator, matrix calculator) shares one
parameterized definition.

### 4. The authored `Language` is still data

The output of a handl authoring program is a `Language` value — the same
serde-serializable data structure ADR-0044 defines. It can be serialized,
cached (ADR-0065), chunked (ADR-0066), and certified (ADR-0067). The
authoring path is a _build step_ that produces data; the runtime consumes
the data, not the authoring program.

This preserves ADR-0044's core property: `Language` is data, no Rust, no
closures, no host-specific state. The authoring handl program runs at build
time (or load time, with caching); the resulting `Language` is pure data at
eval time.

### 5. YAML remains a valid authoring path

YAML is not deprecated. It remains the simplest path for static, hand-written
language definitions. Computational authoring is an _additional_ path for
languages that benefit from abstraction and parameterization. The two paths
produce the same `Language` data structure and can be mixed: a YAML base with
computationally generated extensions.

### 6. The bootstrap is computational authoring of handl itself

ADR-0044 Phase 8's `handl-defining-handl` is the canonical use case: a handl
program that computes the `Language` value describing handl. With
computational authoring, this is not a 5,000-line YAML file — it is a handl
program that uses `binary-op`, `statement-form`, `desugar-rule` constructors
to build the language with shared abstractions. The self-hosting bootstrap
becomes a build step that runs the authoring program and serializes the
result.


## Why

ADR-0044 made `Language` pure data: 7 YAML files, serde-serializable, embedded at compile time. This is declarative — but YAML is _literally_ declarative. Transcribing the same literals into handl syntax gains nothing; the data is identical, the expressiveness is identical.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Abstraction over repetitive language structure.** The `binary-op`
- **Parameterized language families.** Related languages share a definition;
- **The self-hosting bootstrap is a program, not a data file.** handl defines
- **The output is still data.** No runtime dependency on the authoring
- **Two authoring paths.** YAML and handl programs coexist. The framework

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0064-computational-language-authoring.md`](../design/0064-computational-language-authoring.md).
