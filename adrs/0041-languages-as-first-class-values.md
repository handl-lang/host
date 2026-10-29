---
title: Languages as First-Class Values
type: adr
date: 2026-09-02
status: proposed
---

# ADR-0041: Languages as First-Class Values

## Status

Proposed

Builds on [ADR-0039 — Kits Are Curated Plugin Lists](./0039-kits-are-curated-plugin-lists.md)
and [ADR-0040 — VM is an Operation, Not a Subsystem](./0040-vm-is-an-operation-not-a-subsystem.md).

## Context

ADR-0039 made kits thin curations of plugins. ADR-0040 made evaluation an
`Operation` served by composable `EvalBehavior` rules, dissolving the VM
subsystem. Together they completed the platform's stated vision: every
front-end stage — lex, parse, typecheck, lint, format, eval — is a
`RuleAcceptor` plugin built from `Behavior` rules contributed by other
plugins. A kit is a curated list of those plugins; a host loads a kit and
runs it.

That architecture made language **components** first-class: a lexer rule, a
grammar rule, an eval rule are all values you can contribute, swap, and
compose. It did **not** make language **itself** first-class. Three things
are still not values:

1. **A whole language is not a handl value.** `Kit` is a Rust struct
   (`platform/src/kit.rs`), `KitManifest` is a flat `Vec<String>` of plugin
   ids (`kit-canonical/src/kit_manifest.rs`). Neither is reified as a
   `Term`/`Value` in the platform's value space (`platform/src/value.rs`).
   A handl program cannot bind a language, pass it to a function, store it
   in a list, or build one at runtime.

2. **`eval` is a host primitive, not a handl function.** The `kit` capability
   only mutates a `KitRegistry` (`add`/`remove`/`info`,
   `plugin-ext-kit/src/lib.rs`). There is no `eval : Language -> Source ->
Value` a handl program can call. The interpreter is a fixed wall above
   the program, invisible and untouchable.

3. **Rule instantiation is hardcoded Rust, not data.** `kit-canonical` turns
   plugin ids into actual rules through a `const` table and a `match`
   statement (`kit-canonical/src/lib.rs`, `rule_for_plugin_id`). Adding a
   genuinely new grammar rule that isn't already in
   `GRAMMAR_RULE_PLUGIN_IDS` requires touching Rust, not handl.

### The insight

When functions went first-class, the act of calling stopped being a special
form and became a value-level operation: `apply : (A -> B) -> A -> B`. The
thing that operates and the thing operated on became the same kind of
thing.

The parallel move for languages: the act of interpreting stops being a
fixed host and becomes a value-level operation: `eval : Language -> Source
-> Value`. The language you write in and the language you describe become
the same kind of thing.

| first-class functions                     | first-class languages                        |
| ----------------------------------------- | -------------------------------------------- |
| `apply : (A -> B) -> A -> B`              | `eval : Language -> Source -> Value`         |
| authority lives in the function's closure | authority lives in the language's behaviors  |
| gate the function, not `apply`            | gate the language, not `eval`                |
| `this`/`self` = the receiver              | `self` = the running language                |
| recursion via self-reference              | self-hosting bootstrap via `eval(self, ...)` |
| Y combinator unlocks Turing completeness  | Y-for-languages unlocks a universal host     |
| pure function = safe to apply anywhere    | pure language = safe to eval anywhere        |

### Why `eval` is not a security problem

`apply` is neutral plumbing. The authority lives in the function value
(its closure captures what it can do). You protect by controlling which
functions you hand out, not by gating `apply`. A pure function is safe to
apply to anything.

`eval` is, by the exact same move, neutral plumbing. The authority lives
in the language value (its behaviors capture what it can do). You protect
by controlling which languages you hand out, not by gating `eval`. A pure
language (no I/O behaviors) is safe to eval on any source.

The protection toolkit is identical, one level up: capability discipline at
the value level, effect types on the operator, pure-by-default, gate the
value not the plumbing. If the host never offers an `fs-write` behavior,
no `Language` value can contain it, and no `eval` of any source under any
language can touch the filesystem. The capability is absent at the source,
so it is absent everywhere downstream.

### The arrow calculus solves the unpredictable-compute-graph problem

The naive objection to first-class `eval` is that the compute graph becomes
opaque — you can't know what a source does without running it. This is the
same objection monads face vs arrows.

The resolution is the same. Arrows expose the computation as a structured,
inspectable value; monads hide the continuation behind an opaque function.
Lifted: a **structured** `Language` value (grammar as data, behaviors as
data) gives a statically recoverable compute graph — compilable,
typecheckable, optimizable, inspectable. A **vau** behavior (one that
receives the unevaluated expression and decides what evaluation means
in place) is the `ArrowApply` escape: it recovers expressiveness but costs
static analysis.

The design is typed coexistence, lifted from the arrow playbook:
structured by default, vau as an explicitly-typed opt-in escape, tracked
by the type system. Pure/structured code compiles; vau code interprets.
The platform already has the arrow substrate — `Operation`/`Behavior`/
`RuleAcceptor` is exactly "a language is a bag of composable behaviors as
data."

## Decision

Make `Language` a first-class value in handl's value space, and make
`eval` a handl-level function over it. The host shrinks to a kernel that
ships arrow primitives, `eval`, `self`, and `compile`. Everything else —
including handl itself — is a `Language` value built from those primitives.

### 1. `Language` is a handl-level type

A `Language` is a structured value with three components, all data:

```handl
handl: type.any = language {
  lexer:   lexer.from-tokens([number, string, ident, operator, keyword]),
  grammar: grammar.from-rules([
    grammar.rule("value",  primary-value),
    grammar.rule("call",   call-rule),
    grammar.rule("let",    let-rule),
    grammar.rule("match",  match-rule),
    grammar.rule("handle", handle-rule),
  ]),
  eval:    eval.from-behaviors([arith, boolean, array, object, call, let, match, handle]),
};
```

This replaces the flat `KitManifest { plugins: Vec<String> }` with a
structured description. The manifest names _what_ the language is; the
`Language` value _is_ what the language is.

### 2. `eval` is a handl function

```handl
val: type.any = eval(handl, "1 + 2");     # 3
```

`eval : Language -> Source -> Value` is the primary primitive. It is
always available — it is plumbing, not a gated capability. The authority
lives in the `Language` value, which is constructed with limited
behaviors.

### 3. `self` is the running language, handed to the program

```handl
lang: type.any = runtime.self();          # the language currently running me
val:  type.any = eval(lang, "1 + 2");     # self-application
```

`self` is to languages what a function's name is to a recursive function:
the value captures itself in its own behavior set. This enables
metacircularity (a language implements itself) and the self-hosting
bootstrap.

### 4. Language combinators are handl functions

```handl
handl-c:    type.any = extend(handl, line-comments);    # a dialect, as a value
json-lang:  type.any = language { ... };
merged:     type.any = merge(base, extras);             # value-level composition
restricted: type.any = restrict(lang, ["value", "object"]);
```

`extend`/`merge`/`restrict` are the higher-order combinators, the way
`compose` and `partial` are for functions. `lang-map : Language -> [Source]
-> [Value]` has the same shape as `map : (A -> B) -> [A] -> [B]` — the
tell that languages are actually first-class.

### 5. The bootstrap is a compilation step, not interpretation

```handl
# handl-defining-handl.hnl
handl: type.any = language { lexer: ..., grammar: ..., eval: ... };
binary: type.any = compile(handl, "native");    # the self-hosting compiler
```

If `Language` is data and `compile : Language -> Target` is a function on
that data, then the self-bootstrap is one function application. The
Futamura projections — the foundational result of partial evaluation —
become ordinary currying: fix `lang` and leave `src` dynamic, and you
have a compiled program; leave both dynamic, and you have the interpreter
itself.

### 6. Structured by default, vau as typed escape

The structured `Language` value is the default — grammar as data, behaviors
as data, compute graph statically recoverable. This is the arrow substrate
the platform already has.

`vau` is an explicitly-typed escape hatch for behaviors that need to
intercept evaluation in place (receive the unevaluated expression and
decide what evaluation means). The type system marks vau behaviors;
pure/structured code compiles, vau code interprets. You opt in, the way
you opt into `ArrowApply`.

### 7. Capability discipline at the language level

A `Language` value is constructed with limited authority — only the
behaviors it was built with. Sandboxing is static: inspect the `Language`
value's behaviors at load time and reject if they touch disallowed
capabilities. No runtime sandbox required.

```handl
mod-lang: type.any = kit.build("combat-mod.hnl");
safe: type.bool = capabilities(mod-lang).subset-of(allowed-mod-caps);
if not safe then reject else load(mod-lang);
```

## Consequences

### Positive

- **One compiler, any language.** `compile : Language -> Target` compiles
  any language because every language is the same kind of data.
- **Self-hosting bootstrap as a compilation step.** The language is data;
  the compiler is a function on that data; self-hosting is one function
  application.
- **Futamura projections become currying.** Specializing an interpreter
  into a compiler is ordinary partial application of `eval`.
- **Cross-language optimization.** Embedded languages share one IR; the
  optimizer fuses across language boundaries, no FFI, no marshalling.
- **Types flow across language boundaries.** If behaviors carry their
  types as data, `eval(lang, src)` has a computable result type derived
  from `lang`'s behavior signatures.
- **Every language gets every tool, for free.** One formatter, one linter,
  one debugger, one profiler — all parameterized by `Language`.
- **The host shrinks to a kernel.** The binary ships arrow primitives,
  `eval`, `self`, `compile`. Everything else, including handl, is a
  `Language` value built at startup.
- **Language-level hot reload.** Replace a `Language` value in a running
  system; new evaluations use the new language; old evaluations finish
  under the old one.
- **Multiple language versions, simultaneously.** Two `Language` values
  in the same process, same value space, no compatibility layer.
- **The language is its own specification.** The `Language` value is the
  spec; conformance is `eval(thisLanguage, src) == expected`. No
  separate document that can drift.
- **Mods without serialization or runtime sandboxing.** A mod and host
  share a value space (no marshalling); the mod's `Language` value is
  statically verified at load time (no runtime sandbox).

### Negative

- **Types for `eval` are hard.** If `Language` is a value, the result
  type of `eval(lang, src)` depends on the _value_ of `lang`, not its
  type. Typing this requires the type system to reason about which
  language is being evaluated — dependent-type territory, or accept
  `type.any` escape hatches (a cop-out). This is the deepest open
  problem.
- **Performance for the vau case.** When behaviors are opaque functions
  (vau), the compiler cannot specialize to a language at compile time.
  The structured case compiles; the vau case interprets. Partial
  evaluation / staging is the escape but restricts when `self` can be a
  runtime value.
- **The meta-circular trap.** If every behavior can be redefined,
  including the ones that do the redefining, the meaning of any
  expression depends on the entire stack of active redefinitions.
  Powerful but austere; the typed-coexistence design mitigates but does
  not eliminate this.
- **Nobody has shipped this fully in a typed, performant production
  language.** Racket is the closest (languages as modules, `#lang`) but
  not runtime first-class. MetaOCaml does first-class staging but not
  arbitrary languages. Kernel does first-class fexprs but not as a
  production language. The composable part (which handl's platform
  already has) is well-trodden; the fully-first-class part is a research
  frontier with partial ascents and no complete summit.
- **Wide blast radius.** Reifying `Language` as a handl value, exposing
  `eval`/`self`/`compile` as handl functions, and replacing the flat
  `KitManifest` with a structured description touches the platform, the
  kit layer, the host, and every tool. This is a multi-phase migration,
  not a single PR.

## Implementation phases

### Phase 1 — Tooling parameterized by `Language`

Lowest risk, immediate value, platform already supports it. Make
`BasicFormatter`, `BasicLinter`, `BasicTypechecker`, and the LSP take a
`Language` argument instead of being wired to one canonical kit. One
formatter formats any language you have a value for.

### Phase 2 — `Language` as a handl value; `eval` as a handl function

Reify `Language` as a `Term`/`Value` in the platform's value space. Replace
the flat `KitManifest { plugins: Vec<String> }` with a structured
description (`lexer`/`grammar`/`eval` sections). Expose `eval` as a
handl-level function. Drive `rule_for_plugin_id` from data so the
canonical kit stops being the single source of which grammar rules exist.

### Phase 3 — `self` and the self-hosting bootstrap

Expose `runtime.self()` and `runtime.source-of()`. Ship a
`handl-defining-handl.hnl` that defines `handl` as a `Language` value and
evals itself under itself. The bootstrap becomes a compilation step via
`compile : Language -> Target`.

### Phase 4 — `vau` as a typed escape

Add the vau form — behaviors that receive the unevaluated expression and
decide what evaluation means in place. Track vau behaviors in the type
system; pure/structured code compiles, vau code interprets. The
`ArrowApply` escape, opt-in.

### Phase 5 — Capability discipline at the language level

Static capability verification of `Language` values at load time. A
`Language` carries its capability set; `eval` checks it. The mods
application: shared value space, no serialization, no runtime sandbox.

## Related work

- [ADR-0001](./0001-generic-node-trait-based-capabilities.md) — Generic
  Node, Trait-Based Capabilities, and Plugin-Driven Canonical Kit
- [ADR-0039](./0039-kits-are-curated-plugin-lists.md) — Kits Are Curated
  Plugin Lists (this ADR makes kits value-returning, not just
  registry-mutating)
- [ADR-0040](./0040-vm-is-an-operation-not-a-subsystem.md) — VM is an
  Operation, Not a Subsystem (this ADR makes that operation a handl-level
  function over a first-class value)
- 3-Lisp — reflective tower
- Kernel (vau/fexprs) — first-class operatives
- MetaOCaml — staged metaprogramming, first-class staging
- Racket `#lang` — languages as modules (module-level, not runtime)
- Arrow calculus (Lindley, Wadler, Yallop) — static structure with
  monad-like notation; `ArrowApply` as the escape
- Futamura projections — specialization of interpreters into compilers
