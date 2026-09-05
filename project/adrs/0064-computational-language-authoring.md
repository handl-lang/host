# ADR-0064: Computational Language Authoring

## Status

Proposed

Depends on [ADR-0063 — Scheme-Core Completeness](./0063-scheme-core-completeness.md)
(the core must be able to describe handl before handl is used to author
languages). Refines [ADR-0044 — Language as Data and Runtime
Split](./0044-language-as-data-and-runtime-split.md) Phase 8 and
[ADR-0041 — Languages as First-Class Values](./0041-languages-as-first-class-values.md)
§4 (language combinators as handl functions).

## Context

ADR-0044 made `Language` pure data: 7 YAML files, serde-serializable, embedded
at compile time. This is declarative — but YAML is _literally_ declarative.
Transcribing the same literals into handl syntax gains nothing; the data is
identical, the expressiveness is identical.

The actual win over YAML is **computational authoring**: handl functions that
_generate_, _derive_, _parameterize_, and _abstract over_ `Language` fragments.
YAML cannot express "a grammar row whose binding power is computed from the
operator's arity" or "an eval scheme shared across all binary operators,
parameterized by the operator name." handl can. This is the feature that makes
authoring a language in handl worth doing, and it is what ADR-0041 §4
anticipated with `extend`/`merge`/`restrict` combinators — but those were
specified as host functions, not as handl-level computation over fragment data.

### The gap

Today, language authoring is:

1. Hand-edit YAML files (declarative, static, no computation).
2. Embed at compile time via `include_str!` (ADR-0044 §6).

What is missing:

3. A handl program that _computes_ a `Language` value — functions that take
   parameters and produce fragments, combinators that build fragment sets from
   smaller pieces, abstractions over repetitive structure.

This is not "declare the language in handl" (that is just YAML with different
syntax). It is "write handl functions that generate the language." The
distinction matters: the first is a format change with no new power; the second
is a new authoring layer with real abstraction.

### Why this depends on ADR-0063

If handl is used to author a `Language` value, and that `Language` value is
then `eval`'d, the eval must not require `Foreign` escapes for meta-operations
(ADR-0063). Otherwise the authored language is not self-contained — it depends
on host machinery that the authoring handl program cannot itself describe.
Computational authoring on top of an incomplete core produces languages that
look self-describing but secretly delegate to the host.

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

## Consequences

### Positive

- **Abstraction over repetitive language structure.** The `binary-op`
  pattern, the statement-form pattern, the desugar-rule pattern — all
  abstractable. Language definitions shrink and become maintainable.
- **Parameterized language families.** Related languages share a definition;
  differences are parameters, not copy-pasted YAML.
- **The self-hosting bootstrap is a program, not a data file.** handl defines
  itself via computation, which is both more compact and more honest than a
  YAML file that happens to describe handl.
- **The output is still data.** No runtime dependency on the authoring
  program. Caching, chunking, and certificates apply to the output.

### Negative

- **Two authoring paths.** YAML and handl programs coexist. The framework
  must accept both as fragment sources. This is the same migration-shape
  concern as ADR-0044 §6 (emitters vs data files), and the resolution is the
  same: both produce `Language` data; the path is invisible to the runtime.
- **The authoring handl program must itself be evaluable.** If handl cannot
  yet describe itself (ADR-0063 incomplete), the authoring program runs on
  the host-built handl, not the self-described one. This is fine for a
  build step but means the bootstrap is staged: host-built handl authors
  handl → self-described handl authors handl.
- **Fragment constructors are a surface area.** Every fragment type needs a
  handl-callable constructor with documented fields. This is a stable
  API that must be maintained as fragment types evolve.

### Neutral

- **YAML is not going away.** This ADR adds a path; it does not replace one.
  The choice is driven by whether the language definition benefits from
  computation.

## Implementation phases

### Phase 1 — Fragment constructors

Expose `token-kind`, `grammar-row`, `eval-scheme`, `ast-shape`,
`capability-decl`, `option-schema` as handl functions. Each takes named fields
and returns a fragment value. This is a thin wrapper over the existing Rust
constructors, exposed via the host-function mechanism (ADR-0040, ADR-0046).

### Phase 2 — Fragment-set and Language combinators

Expose `fragment-set`, `merge`, `extend`, `restrict` as handl functions over
fragment values. `merge` is `Language::merge` (ADR-0044 §5); `extend` is merge
with override; `restrict` is fragment-set filtering.

### Phase 3 — Authoring examples

Convert one or two sections of the canonical YAML to handl authoring programs
(`binary-op` for arithmetic operators, `statement-form` for statement
constructs). Verify the output `Language` matches the YAML-derived one via
the conformance suite (ADR-0044 testing strategy).

### Phase 4 — `handl-defining-handl` as a computational authoring program

Rewrite the self-hosting data file (from ADR-0063 Phase 3) as a handl
authoring program using the constructors and combinators. The output must
pass the same no-meta-`Foreign` and conformance assertions.

## References

- [ADR-0063](./0063-scheme-core-completeness.md) — Scheme-Core Completeness
  (the gate; computational authoring on an incomplete core produces
  non-self-contained languages)
- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as Data
  and Runtime Split (the data model; Phase 8 is the target)
- [ADR-0041](./0041-languages-as-first-class-values.md) — Languages as
  First-Class Values (§4 anticipated language combinators as handl functions)
- [ADR-0052](./0052-arrow-calculus-kernel.md) — Arrow Calculus Kernel (the
  core that the authoring program targets)
- [ADR-0046](./0046-capability-discipline-at-the-language-level.md) —
  Capability Discipline (fragment constructors are host functions; they are
  capability-tracked)
