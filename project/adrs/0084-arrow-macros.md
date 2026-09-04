# ADR-0084: Arrow Macros — Type-Safe Compile-Time Metaprogramming

## Status

Proposed. Depends on [ADR-0052 — Arrow Calculus
Kernel](./0052-arrow-calculus-kernel.md), [ADR-0044 — Language as
Data](./0044-language-as-data-and-runtime-split.md), [ADR-0078 — Value
Types as Data](./0078-value-types-as-data.md), [ADR-0054 — Generic
Typecheck Engine](./0054-generic-typecheck-engine.md). Builds on
[ADR-0081 — Data-Driven AST Child Roles](./0081-data-driven-ast-child-roles.md)
and the `plugin-lib-lang-*` meta-function bridge (ADR-0083 §7).

## Context

### Why macros

Handl is data-driven: a `Language` is YAML data, an `Ast` is a data
structure, a `Scheme` (arrow graph) is data. The `plugin-lib-lang-*`
crates expose `lex`/`parse`/`eval`/`compile`/`typecheck` as callable
functions — handl code can already inspect and transform ASTs at
runtime. But there is no **compile-time** mechanism for programs to
generate code. Every abstraction must be expressed at runtime
granularity.

This matters for:

- **Constants and configuration** — POSIX flag composition
  (`O_RDONLY | O_CLOEXEC`), platform-specific values, compile-time
  arithmetic (ADR-0083 §5.1).
- **DSL embedding** — SQL queries, regex patterns, HTML templates, schema
  validators — domain-specific syntax that should expand to handl arrow
  graphs at compile time, not be interpreted at runtime.
- **Derivation** — generating `Eq`/`Show`/`Serialize` implementations
  from a type declaration (ADR-0082 inductive types).
- **Performance** — eliminating indirection: a macro that specializes a
  generic function for a known type removes dynamic dispatch at compile
  time.
- **Pattern abstraction** — recurring code shapes (state machines,
  parsers, retry loops) that are tedious to write but mechanical to
  generate.

### Why arrow macros are not Lisp macros

Lisp macros operate on S-expressions — nested lists. The transformation
is arbitrary list manipulation. There is no structure to check against;
a macro can produce any list, valid or not. Type safety is impossible at
the macro level — you find out at runtime (or at a separate typecheck
pass that sees the expanded code, not the macro itself).

Handl macros operate on **arrow graphs** — `Scheme` values with six
structural combinators (`arr`/`>>>`/`first`/`choice`/`loop`/`app`). An
arrow graph is not an opaque blob; it has compositional structure that
the typechecker understands. A macro that transforms `Scheme A B` to
`Scheme A B` can be **typechecked as an arrow**: input type `A`, output
type `B`, the macro itself is an arrow `Macro A B` where `A` and `B` are
AST types.

This is the key advantage: **macros are arrows, arrows are typed, the
typechecker verifies macros before expansion.**

### Why arrow macros are not Template Haskell

Template Haskell (and Rust's `macro_rules!`/proc-macros) operates on
lambda-calculus terms. Lambda terms are opaque — you cannot inspect a
lambda body without applying it. A `Q [Dec]` (quoted declaration list)
is a black box; you can splice it but not analyze its structure without
pattern matching on a separate AST representation.

Arrow calculus has no this limitation. Arrows are **descriptions**, not
functions. `Seq(f, g)` is a data structure — you can inspect `f` and
`g`, transform them, recompose. `Arr { op, args, source }` is a record
— you can read `op`, rewrite `args`, change `source`. The arrow is the
AST is the runtime representation. There is no gap between "what the
macro sees" and "what the compiler sees."

### What already exists

- **`Scheme` is data** (ADR-0052) — six combinators as a Rust enum,
  serializable, inspectable. Arrow graphs are first-class values.
- **`Ast` is data** (ADR-0044) — nodes with `kind` + children, resolved
  to `Scheme` via `EvalSchemeDef`. The AST → Scheme lowering is
  data-driven.
- **`plugin-lib-lang-*`** (6 crates) — `parse.parse(lang, text)`,
  `eval.eval(lang, ast)`, `compile.compile(lang, ast, target)`,
  `typecheck.typecheck(lang, ast)`. Handl code can already call these.
- **`AstShapeDef` with child roles** (ADR-0081) — AST nodes have named
  roles, not just positional indices. Macros can reference children by
  role.
- **Inductive types** (ADR-0082) — user-defined algebraic datatypes as
  desugared predicates. AST node types can be inductive types, enabling
  pattern matching on AST structure.
- **Value types as data** (ADR-0078) — `value_types.yaml` declares
  types. AST types (`Ast`, `Scheme`, `Token`) can be declared as value
  types with capabilities (`Matchable`, `Serializable`).

### The gap

There is no `macro` declaration form. No way to say "this name expands
to this arrow graph at compile time." No way to pattern match on AST
structure and produce a transformed AST. No way to typecheck a macro
before it fires.

## Decision

### 1. Macros are arrows that transform arrows

A macro is an arrow `Ast ~> Ast` (or more generally `Scheme A B ~> 
Scheme A' B'`). It is a first-class arrow value, typechecked like any
other arrow. The compiler runs it at compile time, feeds it the AST of
the macro call site, and splices the result.

```handl
--- macro: specialize list.map for a known function
macro map_of(f) = ast ~>
  { fn = ast.children.fn
  ; list = ast.children.list
  ; prim.loop({
      step = (state, item) ~> (f(item), state ++ [f(item)])
    })(list)
  }
```

The macro is an arrow. Its type is `Ast ~> Ast`. The typechecker
verifies that it produces a well-typed AST before the compiler uses it.

### 2. The `macro` declaration form

A new top-level declaration, alongside `op`/`handl`/`cap`/`const`:

```handl
macro name(pattern) = body
```

- **`name`** — the macro name, invoked as `name(args)` in source.
- **`pattern`** — pattern match on the call site's AST. Binds sub-AST
  variables.
- **`body`** — an arrow graph that produces an AST. Executed at compile
  time.

Example — compile-time flag composition (ADR-0083 §5.1):

```handl
macro flags(...fs) = prim.or_all(fs.map(fn x => x))

--- usage:
libc.open(path, flags(O_RDONLY, O_CLOEXEC))
--- expands to:
libc.open(path, prim.or(O_RDONLY, O_CLOEXEC))
```

Example — derive `Show` for an inductive type (ADR-0082):

```handl
macro derive_show(type_name, constructors) = ast ~>
  { show = constructors.map(fn c =>
      match c.name {
        "True"  => "true"
        "False" => "false"
        _       => c.name ++ "(" ++ show_args(c.fields) ++ ")"
      })
  ; prim.arr("show_" ++ type_name, [show])
  }
```

### 3. Hygiene

Macro hygiene prevents variable capture: a macro that introduces a
variable `x` must not capture a caller's `x`. Handl's arrow calculus
makes this natural:

- **Arrows don't have free variables in the lambda sense.** An arrow
  `arr f` is a closed description — `f` is a named op, not a free
  variable. `Seq(f, g)` composes two arrows; their environments are
  threaded by `first`, not by lexical scope.
- **Bindings are wires, not names.** In graph blocks (`{ x = f(a); y =
  g(x); y }`), `x` is a wire name local to the block. The compiler
  resolves wire names to positional indices at assembly time
  (ADR-0079). A macro that introduces a wire `tmp` generates a fresh
  index, not a name that could collide.
- **Quotation produces fresh names.** When a macro splices in an AST
  fragment, the compiler assigns fresh wire indices to any bindings the
  fragment introduces. The caller's wires are unaffected.

This is simpler than Lisp's gensym or Rust's hygiene machinery because
arrow graphs don't have lexical scope in the traditional sense —
composition is structural (`>>>`), not scoping.

### 4. Type safety

Macros are typechecked in two phases:

**Phase 1: Macro definition typecheck.** The macro's arrow type is
verified. `macro map_of(f) = ast ~> ...` has type `Ast ~> Ast`. The
typechecker checks that the body produces an `Ast` given an `Ast`
input. This catches macro bugs (wrong AST shape, missing children) at
macro definition time, not at call sites.

**Phase 2: Expansion typecheck.** After the macro fires and produces an
AST, the result is typechecked in the call site's context. This catches
type errors in the expanded code (wrong argument types, missing
capabilities).

Together: the macro is verified to produce well-shaped AST, and the
expanded AST is verified to be well-typed. A macro that typechecks but
produces ill-typed code is caught at the call site, not at definition
time — same as any code generator.

**Typed AST patterns.** The `pattern` in `macro name(pattern) = body`
can carry type annotations:

```handl
macro map_of(f: Ast Arrow, list: Ast) = ...
```

The typechecker verifies that the call site's arguments match the
pattern's types. `f: Ast Arrow` means "an AST node that resolves to an
arrow." This is pattern matching on typed AST, not untyped S-expressions.

### 5. Compile-time execution

Macros run at compile time, not runtime. The execution model:

```
Source
  ↓ (parse)
Ast (with macro calls)
  ↓ (macro expansion — run macro arrows via eval engine)
Ast (expanded — no macro calls)
  ↓ (typecheck)
TypedAst
  ↓ (compile)
Target code
```

Macro expansion happens **after parse, before typecheck**. The macro
arrows are evaluated using the same eval engine (tree-walk or cranelift)
that runs runtime code, but in a **compile-time context** — a separate
runtime instance with access to `plugin-lib-lang-*` meta functions
(`parse`, `eval`, `typecheck`) but no I/O capabilities (no `fs`, no
`net`, no `libc`).

This is the two-level architecture from ADR-0083 §9, applied to
compilation: the **host** runs the compiler, the compiler evals macro
arrows in a sandboxed compile-time runtime, the macros produce AST
fragments that are spliced into the program being compiled.

### 6. Quotation and splicing

Two syntax forms for building AST in macros:

- **`quote { ... }`** — produces an AST from source syntax. The `...`
  is parsed as handl source and returned as an `Ast` value.
- **`splice(expr)`** — inserts an `Ast` value into a surrounding
  `quote`.

```handl
macro unless(cond) = ast ~>
  quote {
    if splice(ast.children.cond) {
      Unit
    } else {
      splice(ast.children.body)
    }
  }
```

`quote` and `splice` are not new calculus primitives — they are
syntactic sugar for AST construction. `quote { f(x) }` desugars to
`Arr { op: "call", args: [Arr { op: "var", args: ["f"] }, Arr { op:
"var", args: ["x"] }], source: Native }`. `splice(e)` desugars to `e`
(the AST value is inserted directly into the enclosing AST construction).

This is homoiconicity: the source syntax and the AST representation are
the same thing, because the AST is data (ADR-0044) and the language is
data (ADR-0044). There is no separate "AST quotation language" — you
write handl, and `quote` gives you the AST for what you wrote.

### 7. Pattern matching on AST

Macros pattern match on AST structure. With ADR-0081 (child roles) and
ADR-0082 (inductive types), this is ordinary pattern matching:

```handl
macro map_of(f, list) = match ast {
  Call(fn=fn, args=[f, list]) => quote { ... }
  _ => Error("map_of expects a call expression")
}
```

The `Call` pattern matches an AST node with `kind == "call"` and binds
its children by role (`fn`, `args`). This is the same pattern matching
used for inductive types (ADR-0082) — AST node kinds are inductive type
constructors.

No special macro-specific pattern language. The same `match` that
works on runtime values works on AST values, because AST values are
ordinary values (ADR-0044).

### 8. What macros cannot do

- **No I/O at compile time.** Macro expansion runs in a sandboxed
  runtime with no `fs`/`net`/`libc` capabilities. A macro cannot read a
  file, make a network call, or fork a process. This prevents
  build-time side effects and keeps builds reproducible.
- **No mutation of the caller's scope.** A macro produces an AST
  fragment. It cannot modify the caller's bindings, inject imports, or
  change the language definition. The fragment is spliced in; the
  caller's scope is unchanged.
- **No infinite expansion.** The compiler tracks expansion depth and
  rejects recursive macro expansions that don't terminate. A macro that
  expands to itself (or to a cycle of macros) is a compile error.

### 9. Relationship to `const` (ADR-0083 §5.1)

`const` is the simplest macro — a nullary macro that produces a literal
value:

```handl
const O_RDONLY = 0
```

is equivalent to:

```handl
macro O_RDONLY() = quote { 0 }
```

`const` is kept as a separate form because it's common, simple, and
doesn't need the full macro machinery (no pattern matching, no
quotation, no splicing). But it is semantically a degenerate macro.

`prim.or` for flag composition (ADR-0083 §5.1) is the next step up — a
macro that composes constants:

```handl
macro flags(...fs) = prim.or_all(fs)
```

This replaces the `prim.or(O_RDONLY, O_CLOEXEC)` call with a
compile-time-computed value. No runtime call, no indirection.

### 10. Relationship to `data` (ADR-0082)

`data` declarations (inductive types) are desugared to predicates,
constructors, and recursors. Today this desugaring is hardcoded in the
compiler. With macros, `data` can be a macro:

```handl
macro data(name, constructors) = ast ~>
  { predicate = build_predicate(name, constructors)
  ; ctors = constructors.map(build_constructor)
  ; recursor = build_recursor(name, constructors)
  ; quote {
      splice(predicate)
      splice(ctors)
      splice(recursor)
    }
  }
```

This makes `data` extensible — users can define their own
`data`-like declarations with custom desugaring. The compiler doesn't
need a special `data` pass; it's a macro that runs at compile time.

This is not required for ADR-0082 (which defines `data` as compiler
desugaring), but it's the natural evolution: once macros exist, `data`
becomes a library macro, not a compiler builtin.

## Consequences

### Positive

- **Type-safe metaprogramming.** Macros are arrows, typechecked before
  expansion. No untyped code generation.
- **No lambda opacity.** Arrow graphs are inspectable descriptions.
  Macros can pattern match on `Scheme` structure, transform
  sub-arrows, recompose. This is impossible with lambda terms.
- **Homoiconic by construction.** `quote`/`splice` are syntactic sugar
  for AST construction. No separate quotation language — the AST is
  data, the source produces data, the macro transforms data.
- **Hygiene for free.** Arrow composition is structural (`>>>`), not
  lexical. Wire names are resolved to indices at assembly time. No
  gensym, no hygiene algorithm, no variable capture.
- **Compile-time sandboxed.** Macros run in a sandboxed runtime with no
  I/O. Builds are reproducible. No build-time side effects.
- **`const` and `data` are degenerate macros.** The mechanism is
  unified — `const` is a nullary macro, `data` is a macro that
  generates predicates/constructors/recursors. One system, not three.
- **DSL embedding.** Domain-specific syntax (SQL, regex, HTML, schemas)
  expands to handl arrow graphs at compile time. No runtime
  interpreter for embedded DSLs.
- **Derivation.** `derive_show`, `derive_eq`, `derive_serialize` are
  macros that inspect a type declaration and generate implementations.
  No compiler builtins for derivation.

### Negative / trade-offs

- **Compile-time cost.** Macro expansion runs arrows at compile time.
  Complex macros (deep AST transformation, recursive derivation) add
  to compile time. Mitigated by caching expanded ASTs and running
  macros under the cranelift JIT (ADR-0083 §10 — the kernel is free to
  choose the engine for compile-time eval).
- **Macro debugging is hard.** A macro that produces wrong AST can be
  hard to trace. Need tooling: macro expansion viewer, step-through
  debugging of macro arrows, AST diff between before/after expansion.
  This is tooling, not architecture — the mechanism supports it (macros
  are arrows, arrows are inspectable).
- **Error messages point to expanded code, not macro call.** When the
  expanded code has a type error, the error location is in the expanded
  AST, not the original macro call. Need source-map tracking through
  expansion (maintain a mapping from expanded AST nodes back to the
  macro call that produced them). Standard problem; solvable with
  source location metadata on AST nodes.
- **Hygiene is not absolute.** While arrow composition avoids lexical
  capture, a macro that explicitly references a wire name from the
  caller's scope (by string) can still collide. This is intentional —
  macros that need to reference caller bindings should receive them as
  pattern arguments, not by name lookup. Documented as a convention,
  not enforced by the type system.

### Compatibility

- **ADR-0083 §5.1 `const` is a degenerate macro.** The `const` form
  stays as a separate declaration (simpler syntax for the common case)
  but is semantically subsumed by macros. No change to ADR-0083.
- **ADR-0082 `data` can become a macro.** Not required — `data` stays
  as compiler desugaring — but macros enable user-defined
  `data`-like declarations in the future. No change to ADR-0082.
- **ADR-0052 arrow kernel is unchanged.** Macros use the same six
  combinators. No new calculus primitives.
- **ADR-0044 language-as-data is unchanged.** Macros are a new
  declaration form, not a new data fragment type. The `macro` form is
  parsed by the existing Pratt parser and lowered to an arrow graph
  via the existing `EvalSchemeDef` mechanism.
- **`plugin-lib-lang-*` is the bridge.** Macros use
  `parse`/`eval`/`typecheck` meta functions to inspect and transform
  ASTs. No new kernel crates.

## Rationale

Lisp macros are powerful but untyped — any list can be produced, type
errors surface at runtime. Rust's macros are typed-ish but operate on
token streams, not structured AST — pattern matching is syntactic, not
semantic. Template Haskell operates on lambda terms — opaque, hard to
inspect.

Arrow macros are different because arrows are **descriptions with
structure**. A `Scheme` value is not a black box — it's six
combinators with inspectable children. Pattern matching on `Scheme` is
pattern matching on the computation's structure, not on surface syntax.
Typechecking a macro is typechecking an arrow — the same machinery that
typechecks runtime code.

The key insight: **in arrow calculus, there is no gap between the
program's structure and its representation.** The AST is the arrow graph
is the runtime value. Macros transform one representation into another,
and all three are the same thing — data. This is ADR-0044 ("language as
data") applied to metaprogramming: the macro system doesn't need a
separate representation, a separate type system, or a separate
evaluation model. It's arrows all the way down.

## Related ADRs

- [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) —
  the six combinators macros operate on. No new primitives.
- [ADR-0044 — Language as Data](./0044-language-as-data-and-runtime-split.md)
  — AST and Language are data. Macros transform data.
- [ADR-0078 — Value Types as Data](./0078-value-types-as-data.md) —
  AST types (`Ast`, `Scheme`, `Token`) as value types with capabilities
  (`Matchable`, `Serializable`). Prerequisite for typed AST patterns.
- [ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md)
  — typechecks macro arrows before expansion.
- [ADR-0081 — Data-Driven AST Child Roles](./0081-data-driven-ast-child-roles.md)
  — macros reference AST children by role, not position.
- [ADR-0082 — Inductive Types](./0082-inductive-types.md) — AST node
  kinds as inductive constructors. `data` as a degenerate macro.
- [ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md) — `const`
  and `prim.or` (§5.1) are degenerate macros. This ADR subsumes them.

## Appendix: Implementation phases

### Phase 1 — `macro` declaration + `quote`/`splice`

1. Add `macro` to the lexer/parser — new top-level declaration form.
2. Add `quote { ... }` and `splice(expr)` syntax — desugar to AST
   construction.
3. Implement macro expansion in the compiler: after parse, before
   typecheck. Run macro arrows in a sandboxed compile-time runtime.
4. Typecheck macro definitions (`Ast ~> Ast`) at macro definition time.
5. Typecheck expanded AST at call site.

**Done when:** a handl program can define `macro unless(cond, body) =
...` and use it in source code.

### Phase 2 — AST pattern matching

1. Declare AST node types as inductive types (ADR-0082) — `Call`,
   `Assign`, `Block`, etc. as constructors with child roles (ADR-0081).
2. Enable `match` on AST values in macro bodies.
3. Typed patterns: `match ast { Call(fn=f, args=[a, b]) => ... }`.

**Done when:** macros can pattern match on AST structure and bind
children by role.

### Phase 3 — `const` and `data` as macros

1. Implement `const` as a degenerate macro (nullary, produces literal).
2. Implement `data` as a macro that generates predicates/constructors/
   recursors (ADR-0082 desugaring becomes a library macro).
3. Remove compiler builtins for `const` and `data` desugaring.

**Done when:** `const` and `data` are library macros, not compiler
builtins.

### Phase 4 — Derivation macros

1. `derive_show`, `derive_eq`, `derive_serialize` as macros that
   inspect a type declaration and generate implementations.
2. Standard library of derivation macros.

**Done when:** users can write `data Point = Point(x: Int, y: Int)
derive [Show, Eq]` and get generated implementations.

### Phase 5 — DSL embedding

1. Macros that parse domain-specific syntax (SQL, regex, HTML) and
   produce handl arrow graphs.
2. Quasi-quotation for embedded DSLs — `sql { SELECT * FROM users }`
   as a macro call that produces a query arrow.

**Done when:** users can embed DSLs in handl source that expand to
compile-time arrow graphs.
