# ADR-0074: Arrow Surface Syntax v0.1 — Redesign

## Status

Proposed

Establishes the v0.1 surface syntax for the arrow-native language described in
[ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) and
[ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md). Refines the
combinator surface introduced by ADR-0052 (`arr`/`>>>`/`first`/`choice`/`loop`/`app`)
into a human-facing syntax, and corrects two design errors carried over from
the earlier draft: the absence of destructuring renaming, and the misuse of
`rec` as a recursive-arrow constructor.

## Context

ADR-0052 fixed the *kernel*: the calculus is `arr` / `>>>` / `first` / `choice`
/ `loop` / `app`, and `vau` was removed. ADR-0069 fixed the *module model*: a
file is an arrow, imports normalize to projections. Neither addressed the
**surface syntax** that programmers actually write.

The earlier working draft (the "Arrow Language Specification" circulated
before this ADR) proposed a complete v0.1 surface: `~>` for arrow
construction, `>>>` / `***` / `&&&` / `|||` for the categorical combinators,
graph blocks `{ ... }`, `if`/`match`, positional `[a, b]` and record
`{ a, b }` structures, rest destructuring, optional punctuation, structural
type inference, and `rec` for recursion.

Two defects in that draft became apparent on review:

### Defect 1 — Destructuring cannot rename

The draft's destructuring patterns conflated the **source field/position**
with the **local binding name**:

```text
{ name, age } = user
```

This binds `name` and `age` — the same identifiers as the source fields. There
was no way to pull `user.name` and bind it locally as `userName`. Programmers
were forced to either:

- introduce a redundant binding afterwards (`name = user.name; userName = name`), or
- avoid destructuring and write `userName = user.name` directly,

both of which defeat the purpose of pattern syntax and produce noise in graph
blocks where every wire is named.

The same problem affected nested patterns: pulling `user.name` deep inside a
structure forced the local wire to be called `name`, even when several
structures in scope all exposed a `name` field and the surrounding graph
needed distinct wire names.

### Defect 2 — `rec` was modelled as a recursive-arrow constructor

The draft proposed:

```text
countdown = rec x ~> {
  if x <= 0
    return 0
  next = x - 1
  countdown next
}
```

This treats `rec` as syntax for declaring a self-referential arrow — a
"recursive function" with arrow clothing. That is the wrong model for two
reasons:

1. **It re-introduces named function recursion through the back door.** The
   whole point of the arrow calculus (ADR-0052) is that the unit of
   computation is a *graph*, not a function. A "recursive arrow" is just a
   function with a feedback edge drawn on top. The recursion is the feedback
   edge; naming the arrow is incidental.

2. **It does not match Paterson's arrow calculus.** In Paterson's model
   (which ADR-0052 follows), recursion is expressed by `loop`, which
   introduces a *recursive region* where wires may depend on each other
   cyclically. The compiler constructs the feedback graph and checks that it
   is well-typed. There is no "recursive arrow declaration" — there is a
   region with cycles. The draft's `rec x ~> { ... }` form obscures this and
   pushes the language back toward a lambda-with-recursion mental model.

The draft's own complete example exposed the problem. Writing:

```text
retry = rec request ~> {
  result = send request
  if result.ok
    return result
  if result.attempts >= 3
    return error "failed"
  retry result.next
}
```

reads as "define a function `retry` that calls itself." But semantically what
we want is: *inside the arrow `retry`, there is a feedback region whose
current wire is `request`, whose step is `send`, and whose exit conditions
are `result.ok` and `attempts >= 3`.* The recursion is a property of a
*region inside* the arrow, not of the arrow declaration itself.

## Decision

Adopt the v0.1 surface syntax as specified in the accompanying *Arrow Language
Specification v0.1*, with **two corrections** and **one addendum** to the
draft:

1. **Destructuring supports renaming.** The pattern separates source from
   local binding.
2. **`rec` introduces a recursive region, not a recursive arrow.** It follows
   Paterson-style recursive bindings, consistent with `loop` in the kernel.
3. **Control-flow and construction sugar** is added: `while`, `do while`,
   `for x in xs`, `??`, `?.`, and object/positional spread. All six desugar
   to forms already in the calculus (`rec`, `match`, `|||`, structural
   construction); none introduce new kernel combinators.

Everything else in the draft — `~>`, `>>>` / `***` / `&&&` / `|||`, graph
blocks, `if` / `match`, positional and record structures, rest destructuring,
optional punctuation, structural inference, `infer`, `_`, explicit
`name: Type` annotations — is accepted as the v0.1 surface.

### Correction 1 — Destructuring renaming

A pattern element may use `source: local` to bind a source field/position to
a different local wire name.

For records:

```text
{ name: userName, age: userAge } = user
```

means: take the `name` field, bind it locally as `userName`; take the `age`
field, bind it locally as `userAge`.

In arrow input position:

```text
greet = { name: userName } ~> userName
```

is the arrow that selects `.name` and outputs it under the local wire
`userName`.

Nested patterns compose:

```text
{
  user: {
    name: userName
    age: userAge
  }
} ~> {
  userName
  userAge
}
```

For positional structures there is no source name to rename — the pattern
variable *is* the local name:

```text
[first, second] = value
```

Rest destructuring already names the tail naturally, so renaming is not
needed there:

```text
[a, ...remaining]
{ name, ...remaining }
```

The grammar for a single pattern element becomes:

```text
pattern_element ::= source ':' local
                  | identifier
                  | '...' identifier
                  | literal
                  | nested_pattern
```

When the single-identifier form is used, the identifier serves as both
source and local name (the common case, preserved for brevity).

This makes destructuring a first-class pattern feature: the source structure
and the local wire namespace are independent, and the programmer chooses
whether they coincide.

### Correction 2 — `rec` as a recursive region (Paterson-style)

`rec` **does not define a recursive arrow**. It introduces a **recursive
computation region** inside an arrow graph. Bindings inside the region may
depend on other bindings in the same region, including bindings textually
defined later. The compiler constructs the corresponding feedback graph and
checks that the recursive structure is well-typed.

Syntax:

```text
rec {
  binding
  binding
  ...
}
```

The region evaluates to its last binding (or to an explicit `return`), like
a graph block.

The minimal example:

```text
loop = x ~> {
  rec {
    a = f b
    b = g a
  }
  a
}
```

Here `a` and `b` form a cyclic graph:

```text
      ┌──── f ← b
      │
      a
      │
      ↓
      g
      │
      └────→ b
```

A practical feedback loop:

```text
iterate = x ~> {
  rec {
    next = step current
    current = if done next
      next
      else step next
  }
  current
}
```

The earlier `countdown` and `retry` examples are rewritten as arrows that
*contain* a `rec` region, not as `rec`-declared arrows:

```text
countdown = x ~> {
  rec {
    result = if x <= 0
      0
      else countdown (x - 1)
  }
  result
}
```

or, with the feedback made explicit:

```text
countdown = x ~> {
  rec {
    next = x - 1
    result = if x <= 0
      0
      else countdown next
  }
  result
}
```

```text
retry = request ~> {
  rec {
    result = send request
    exit = if result.ok
      result
      else if result.attempts >= 3
        err "failed"
        else retry result.next
  }
  exit
}
```

The defining property is: **`rec` describes a graph with cycles**, not a
recursive function. This keeps `rec` consistent with the language's central
idea (ADR-0052) and with Paterson's `loop` combinator at the kernel level.

The legal-recursive-binding grammar is formalized around the Paterson model:

- Every binding inside a `rec` region is a wire in a cyclic subgraph.
- The compiler must be able to assign a well-typed fixpoint to the region.
- A region that admits no well-typed fixpoint is a compile error, not a
  runtime divergence.

This is the same discipline the kernel applies to `loop`; `rec` is the
surface syntax for the same idea.

### Addendum — Control-flow and construction sugar

The v0.1 draft specified `if`, `match`, and `rec` as the only control-flow
forms. Review against typical TypeScript patterns surfaced six additional
forms that programmers expect and that the calculus already supports via
desugaring. They are added here as **surface sugar** — none introduce new
kernel combinators (ADR-0052), and none change the type system. Each desugars
to `rec`, `match`, `|||`, or structural construction.

#### A.1 `while` — sugar over `rec`

```text
while cond {
  body
}
```

desugars to a `rec` region with an implicit feedback wire: each iteration's
bindings feed back into the next; the loop exits when `cond` is false. The
result is the last expression in the body, evaluated on the final iteration
(see ID.5).

A `while` without a block is also permitted:

```text
while cond
  body
```

Example:

```text
countdown = n ~> {
  while n > 0 {
    n = n - 1
  }
  n
}
```

Conceptually the graph:

```text
n ──→ cond ──→ if ──┬──→ n' = n - 1 ──→ feedback
                    │
                    └──→ exit ──→ output
```

The `n = n - 1` inside the body is a new wire per iteration (shadowing), with
the feedback edge connecting each iteration's `n` to the next. This is exactly
what `rec` already does; `while` is the common case where there is one
condition and one body, with the feedback made implicit.

#### A.2 `for x in xs` — sugar over `rec` + destructuring

```text
for x in xs {
  body
}
```

desugars to a `rec` region that splits head/tail, binds `x` to the head, runs
the body, and recurses on the tail. The result is a collection of body
results — i.e. it is `map`.

A fold form is provided with an explicit accumulator:

```text
for x in xs, acc = init {
  acc = f acc x
}
```

This produces the final `acc` — i.e. it is `fold`.

Examples (map):

```text
doubled = xs ~> {
  for x in xs {
    x * 2
  }
}
```

Example (fold):

```text
sum = xs ~> {
  for x in xs, acc = 0 {
    acc + x
  }
}
```

The graph for `for x in xs { f x }`:

```text
xs ──→ split ──┬──→ x ──→ f x ──→ collect
               │
               └──→ rest ──→ feedback
                                │
                          (exit when empty)
```

The `for` form is the surface counterpart to the `loop`-over-stream pattern
described in [ADR-0072 — Concurrent Arrow Calculus](./0072-concurrent-arrow-calculus.md)
and the state-machine loop in
[ADR-0073 — State Machine Invariants](./0073-state-machine-invariants.md).

#### A.3 `??` — nullish coalescing

```text
a ?? b
```

desugars to a `match` over the option sum:

```text
match a {
  some: value
  none: b
  _: b
}
```

If `a` is `some value`, the result is `value`. If `a` is `none` (or any
non-`some`), the result is `b`. This is a choice operation — it is `|||`
over `some | none` with the right branch being a constant arrow.

Example:

```text
name = user.name ?? "anonymous"
```

The option variant names are lowercase (`some` / `none`), matching the
spec's pattern style. `Option` is a library type in `lib-option` (see
ID.9), not a language primitive.

#### A.4 `?.` — optional chaining

```text
user?.address?.city
```

Each `?.field` is an arrow:

```text
?.field = x ~> match x {
  some: v ~> v.field
  none: none
}
```

And the chain composes via `>>>`:

```text
user?.address?.city  =  user ~> (?.address >>> ?.city)
```

Once a `?.` hits `none`, every subsequent `?.` returns `none` — composition
short-circuits through `none`. This is clean because `>>>` already propagates
`none` through the chain.

Example with coalescing:

```text
city = user?.address?.city ?? "unknown"
```

#### A.5 Spread — construction dual of rest destructuring

Rest destructuring captures `...rest` on the binding side. Spread re-inserts
it on the construction side. They are duals, and the round-trip is the
immutable record update pattern.

Object spread:

```text
{ ...a, b: 2 }
```

means: take all fields of `a`, then add/override `b: 2`. Later keys win (same
as TypeScript):

```text
{ ...a, b: 2 }   -- b: 2 wins if a has b
{ b: 2, ...a }   -- a's b wins if a has b
```

Multiple spreads (merge):

```text
{ ...a, ...b, c: 3 }
```

Union of fields, later spreads override earlier on conflicts. This is record
merge:

```text
merged = { ...defaults, ...overrides }
```

Positional spread:

```text
[...a, b]          -- append
[head, ...middle, last]  -- destructuring from both ends (already supported)
```

Computed keys + spread:

```text
{ ...a, [key]: value }
```

Spread first, then computed key overrides.

The round-trip (immutable record update):

```text
{ b, ...rest } = a          -- extract b, capture the rest
newA = { ...rest, b: 2 }    -- re-insert with updated b
```

This is how a field is "mutated" without mutation. The `...rest` captured on
the left is spread back on the right.

In arrow position:

```text
withDefaults = input ~> {
  { ...defaults, ...input }
}
```

And composed:

```text
parseAndMerge = x ~> {
  parsed = parse x
  { ...defaults, ...parsed }
}
```

**Typing.** Spread is structural, as with everything else:

- `{ ...a, b: 2 }` has type "all fields of `a`, plus `b: Int`". If `a`
  already has `b`, the type is narrowed to `Int` (the override wins).
- `{ ...a, ...b }` has type "union of `a`'s and `b`'s fields, with `b`'s
  types winning on conflicts".
- `[...a, b]` has type "positional structure of `a`'s element type, plus
  `b`'s type".

The solver derives these from the spread operands' structural types — no new
type machinery, just structural unification with override semantics.

**What spread is and is not.** Spread is a *construction form*, not
control-flow sugar. It does not desugar to `match` or `rec`; it is the direct
counterpart to rest destructuring, which is also a primitive pattern form.
The kernel sees it as an `arr` node that merges two structural values.

#### A.6 `do { ... } while` — sugar over `rec` (body-first)

```text
do {
  body
} while cond
```

The body executes **before** the first condition check — the loop runs at
least once. After each body execution, `cond` is evaluated; if true, the
body runs again; if false, the loop exits with the last body result.

This is the post-test loop, complementing `while` (pre-test). It desugars
to a `rec` region that runs the body first, then branches on the condition:

```text
-- desugar of: do { body } while cond
rec {
  result = body
  continue = cond result
  -- if continue, feedback to re-run body
  -- if !continue, exit with result
}
```

Example:

```text
-- read until empty line (always reads at least once)
readUntilEmpty = input ~> {
  do {
    line = readLine input
  } while line != ""
  line
}

-- prompt at least once
prompt = input ~> {
  do {
    answer = askUser input
  } while answer != "yes"
  answer
}
```

The result is the last expression in the body, evaluated on the final
iteration (same semantics as `while`, ID.5).

### What is accepted unchanged from the draft

The following from the v0.1 draft are accepted as-is:

- **Design principles**: arrow-native, statically typed, type-inferred,
  expression-oriented, compositional, dataflow-oriented, structurally typed
  by default, immutable values and wires, parallelism inferred from
  independent branches, explicit control-flow exits via `return`, minimal
  syntax. The graph *is* the type derivation.
- **Program structure**: `name = expression`, optional semicolons, newline
  separation.
- **Arrows**: `x ~> e`, `x ~> { ... }`, juxtaposition as application.
- **Inline and point-free arrows**: `x ~> x * 2`, `* 2`, `+ 1`, `>>> parse
  User`, `.name`.
- **Composition operators**: `>>>` (sequential), `***` (product), `&&&`
  (fan-out), `|||` (choice), with the categorical/arrow laws in §36 of the
  draft.
- **Blocks and final-binding returns**: a final binding both introduces a
  wire and returns its value; an explicit `return` terminates a path early.
- **Conditionals**: `if c e else e`, single-statement and braced forms,
  expression and control-flow uses.
- **Destructuring**: positional `[a, b]`, record `{ a, b }`, rest `[a,
  ...rest]` and `{ a, ...rest }`, nested patterns, simultaneous assignment
  `a, b = b, c`. (Plus the renaming added by Correction 1.)
- **Records and indexing**: `{ name: "Luis", age: 30 }`, optional commas,
  computed keys `{ [val]: x }`, indexing `obj[val]`, field access
  `obj.field`. (Plus spread construction added by Addendum A.5.)
- **Pattern matching**: `match value { pattern: expr }`, literal / variable
  / positional / record / nested patterns, `_` wildcard, optional commas.
- **Records vs blocks**: contextual parsing of `{}` based on syntactic
  position (value/pattern position → record; after `~>` or a control-flow
  construct → block).
- **Control flow**: `if` / `match` / `rec` from the draft, plus `while`,
  `do while`, `for x in xs`, `??`, `?.` added by Addendum A.1–A.4, A.6. All
  desugar to `rec` / `match` / `|||`.
- **Types**: explicit `name: Type`, `type.Str` / `type.Int` / etc.
  namespace, structural inference, `infer x` and `infer (parse _)` to name
  inferred types, `_` as inferred placeholder.
- **Graph semantics**: bindings are named wires, dependencies determine
  order, independent nodes may run concurrently, control flow is graph
  structure, errors are early-exit paths.
- **Minimal core**: `~>` / `>>>` / `***` / `&&&` / `|||` / `rec`. Everything
  else — including `while`, `do while`, `for`, `??`, `?.`, spread — is
  structure, data, control flow, patterns, or type constraints, and desugars
  to the core.

## Consequences

### Positive

- **Destructuring renaming** removes a frequent source of post-destructuring
  rebinding noise. Wire names in graph blocks can match the surrounding
  domain vocabulary rather than the source record's field names. Nested
  patterns no longer force name collisions when two structures share a
  field name.
- **Paterson-style `rec`** keeps the surface syntax aligned with the kernel
  calculus (`loop`) and with the central idea that computation is graphs,
  not functions. Recursion is presented honestly as a cyclic subgraph,
  which is what the compiler actually builds and type-checks.
- **`rec` regions compose with the rest of the syntax.** A `rec` region is
  just a block form usable inside any arrow body, so it composes with
  `if`, `match`, `>>>`, and the other combinators without special-casing.
- **The v0.1 surface is now internally consistent.** The two corrections
  were the only places where the draft contradicted ADR-0052's kernel
  model. With them fixed, the surface is a faithful notation for the
  calculus.
- **Optional punctuation** (semicolons, commas) keeps common code compact
  while permitting dense one-liners, without introducing ambiguity given
  the contextual `{}` rule.
- **Control-flow sugar (`while`, `do while`, `for`, `??`, `?.`)** closes the
  expressiveness gap with TypeScript for the most common imperative and
  option-handling patterns, without growing the kernel. Programmers
  migrating from TS find familiar forms; the calculus stays at six
  combinators. `do while` covers the post-test loop case that `while`
  alone misses.
- **Spread** completes the destructuring/construction duality. Immutable
  record update — the single most common TS pattern written as
  `{ ...obj, field: value }` — now has a direct equivalent, and the
  round-trip with `...rest` is clean.

### Negative

- **`rec` regions are harder to read than the draft's `rec x ~> { ... }`
  form for the simple "countdown" case.** A reader expecting function-style
  recursion must instead read a feedback region. This is the cost of
  honesty about the underlying model; tooling (graph visualization,
  type-driven expansion) is the mitigation, not reverting the syntax.
- **Pattern grammar is now slightly more complex** with the `source: local`
  form. The single-identifier shorthand covers the common case, but
  parsers and linters must handle both forms and forbid nonsense such as
  `1: x` outside match contexts.
- **Contextual `{}` parsing** (record vs block) is a parser complexity.
  The rule is well-defined (value/pattern position vs after `~>` /
  control-flow), but it is a contextual rule, not a context-free one.
  This is accepted because the alternative — distinct delimiters for
  records and blocks — was rejected as syntactic noise in an
  expression-oriented language.
- **Operator precedence and associativity** are fixed in ID.1, but arrow
  kind signatures for `***` / `&&&` / `|||` are still open — the
  combinators carry their input/output arity in the term grammar but not
  yet in the type system. This is follow-up task #3.
- **`return` semantics across `rec` regions and nested blocks** are fixed
  in ID.14 (innermost enclosing `~>` only). The remaining subtlety is
  how `return` interacts with `while` / `do while` / `for` bodies, which
  are sugar over `rec` — a `return` inside a `while` body should exit
  the enclosing arrow, not just the loop. This needs to be specified in
  the desugaring (follow-up task #4).
- **`while`, `do while`, and `for` desugaring must specify feedback-wire
  naming.** Because these forms desugar to `rec` regions with implicit
  feedback, the compiler must define how shadowed bindings in the body
  connect across iterations (e.g. `n = n - 1` inside `while`). The rule
  is "last binding of a given name feeds back to the next iteration", but
  the precise interaction with parallel independent wires in the body
  needs to be written down.
- **`for x in xs` map vs fold ambiguity.** The bare form produces a
  collection (map); the `acc = init` form produces a scalar (fold). The
  parser must distinguish these by the presence of the accumulator
  binding, and the type checker must enforce that the fold form's body
  produces a value of the accumulator's type.
- **`??` and `?.` commit to `some` / `none` as the option vocabulary.**
  This is consistent with the spec's lowercase pattern style, but it
  forecloses alternative option encodings (e.g. `Result` with `ok` /
  `err`). If `Result` is later given its own sugar, `??` and `?.` may
  need to generalize or be joined by a `Result`-specific operator.
- **Spread typing with conflicting field types** requires the solver to
  pick the later operand's type on conflict. This is a structural
  override rule, not a subtyping rule, and may surprise programmers who
  expect subtyping-based merging. The rule should be documented as
  "last wins, structurally".

### Risks

- **Readers familiar with Haskell's `proc` notation / arrow do-notation**
  may expect `rec` to behave like Haskell's `rec` in `do` blocks (which is
  also Paterson-style recursive bindings). That is in fact the model we
  adopt, so the risk is mostly positive — but the surface syntax differs
  from Haskell's, so the analogy should be documented in the language
  guide rather than assumed.
- **The `infer` form** is powerful but its precise interaction with the
  type solver (especially `infer (parse _)`, which extracts a type from
  an arrow expression with a placeholder input) needs a precise typing
  rule. The `_` placeholder scope is resolved (ID.10: `infer` and
  patterns only), but the `infer` typing rule itself is still to be
  formalized.

## Implementation Decisions

The following decisions were made during ADR review to resolve ambiguities
in the v0.1 spec. They are normative for the v0.1 implementer.

### ID.1 Operator precedence and associativity

```
field access / indexing          (highest, left-assoc)
application (juxtaposition)      (left-assoc)
* / +  (arithmetic, library)     (left-assoc)
?? / ?.  (option sugar)          (left-assoc)
*** / &&&  (product / fan-out)   (left-assoc)
>>>  (sequential composition)    (left-assoc)
|||  (choice)                    (left-assoc)
~>  (arrow construction)         (right-assoc, lowest)
```

Application binds tighter than `>>>`: `f x >>> g` = `(f x) >>> g`.
Arithmetic binds tighter than composition: `x + 1 >>> f` = `(x + 1) >>> f`.
`~>` is the lowest-precedence operator and is right-associative.

### ID.2 Multi-arg arrows — explicit tuple, no currying

Arrows take exactly one input. Multi-input arrows use a positional
structure: `add = [x, y] ~> x + y`. Application requires the explicit
tuple: `add [a, b]`, NOT `add a b`. No auto-currying.

### ID.3 `rec` typing — Paterson loop

`rec` regions follow Paterson's `loop` model: the region has a state wire
and an exit wire; the fixpoint is over the state. Termination is NOT
checked at the type level — it is a runtime/effect concern. The type is
the fixpoint of the state transition. No guard/`return` is required for
well-typedness. A `rec` without a well-typed fixpoint is a compile error.

### ID.4 Sum and option construction — constructor prefix

Tags are lowercase, prefix-position: `some value`, `none`, `Left value`,
`Right value`, `ok value`, `err value`. Nullary tags (`none`, `err` when
no payload) are bare identifiers. These are library constructors, not
language primitives (see ID.8).

### ID.5 `while` result — last expression

`while c { body }` returns the last expression in the body, evaluated on
the final iteration (the iteration where `c` becomes false). If the body
never runs (condition false on entry), the result is a type-appropriate
default inferred by the solver. To return multiple wires, write an
explicit record as the last expression.

### ID.6 `for` iterator — requires `Iterator` trait (ADR-0075)

`for x in xs` requires the input to satisfy the `Iterator` trait defined
in [ADR-0075 — Traits as Arrow Signatures](./0075-traits-as-arrow-signatures.md).
Traits are in scope for v0.1. The `for` form desugars to a `rec` region
that calls `Iterator.next` and dispatches via `match`/`|||`.

### ID.7 Structural subsumption — row polymorphism

Row polymorphism (OCaml-style). `{ name } ~> name` has type
`{ name: Str | r } ~> Str` where `r` is a row variable. A record with
additional fields unifies with the row constraint. The solver tracks
extra fields through composition. This requires row variables in the type
system (ADR-0054).

### ID.8 Operators and literals — all library arrows

There are no built-in operators in the language. `+`, `-`, `*`, `/`, `==`,
`!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!` are all library arrows. The
parser recognizes operator characters and desugars operator expressions
to arrow application: `x + y` desugars to `add [x, y]` (or the library's
named arrow). Point-free operators follow the general prefix rule: any
binary operator in prefix position is a point-free arrow (`* 2` =
`x ~> x * 2`).

Literals are language-level: `true`/`false` (booleans, lowercase), numeric
(`1`, `1.5`), string (`"..."`). Operations on literals are library arrows.

### ID.9 `Option` and `Result` — library types, not built-ins

`Option` (`some`/`none`) is in `lib-option`, a handl library. `Result`
(`ok`/`err`) is in `lib-result`, a handl library. Neither is a language
primitive. `??` and `?.` are surface sugar that desugar to `match` over
the library's `Option` type. The spec examples using `return error "msg"`
should be read as `return (err "msg")` using `lib-result`'s constructor.

### ID.10 `_` placeholder — `infer` and patterns only

`_` is allowed in two contexts: (1) `infer (parse _)` to extract a type
from an arrow expression, (2) as a wildcard pattern in `match` and
destructuring. It is NOT allowed as a function argument (`f _` is a
syntax error). Point-free arrows use explicit `~>` instead.

### ID.11 Contextual `{}` — position-based

`{}` is a block when it immediately follows `~>`, `if cond`, `else`,
`match val`, `while cond`, `for ...`, or `rec`. Otherwise it is a record.
In `match val { ... }`, the outer `{}` is the match body (block-like,
holding pattern arms). Any `{}` inside a pattern position is a record
pattern.

### ID.12 Arrow type notation — `A ~> B`

Arrow types use `~>` at the type level, matching the term-level syntax:
`parse: Str ~> User = ...`. Input and output are both type expressions.

### ID.13 Match exhaustiveness — required

The compiler checks exhaustiveness. Non-exhaustive `match` is a compile
error. A `_` catch-all (or proof that the listed patterns cover all
cases) is required.

### ID.14 `return` scope — innermost arrow only

`return` exits the innermost enclosing `~>` arrow. A `return` inside a
nested arrow exits that inner arrow, not any outer one. To exit an outer
arrow from inside a nested one, propagate the value explicitly via
`match`/`|||`.

### ID.15 User-defined types — structural, no `type` keyword

Types are structural and inferred. `infer x` names the inferred type of
`x`. `infer (parse _)` names the inferred type of an arrow expression.
There is no `type` declaration keyword in v0.1. Sum types are
library-provided (`lib-option`, `lib-result`) or inferred from constructor
usage. Nominal tagging uses branded types (ADR-0055).

### ID.16 Module syntax — `import name from "path"`

Per [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md), the
surface syntax for imports is `import name from "path"`. This normalizes
to a projection of the module's record arrow. Multi-file programs are in
scope for v0.1.

### ID.17 Traits — per ADR-0075

The trait system is fully specified in
[ADR-0075 — Traits as Arrow Signatures](./0075-traits-as-arrow-signatures.md).
Trait inference is the primary mode (annotation-free, laws recognized from
wiring). Explicit `trait`/`impl`/`law` declarations are optional sugar.
No global coherence in v1. Traits are structural, not nominal. The
`Iterator` trait is required for `for x in xs`.

## Implementation

This ADR establishes the v0.1 surface syntax. The Implementation Decisions
above resolve all ambiguities needed for a v0.1 implementer to proceed
with full autonomy. The expected follow-ups are:

1. A formal grammar with operator precedence and associativity (ID.1
   above), turning §43 of the draft into a parser-ready specification.
   This builds on
   [ADR-0053 — Generic Pratt Parser](./0053-generic-pratt-parser.md).
2. A typing rule for `rec` regions, formalized around Paterson's `loop`
   (ID.3 above) and the fixpoint discipline in
   [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
   and [ADR-0073 — State Machine Invariants](./0073-state-machine-invariants.md).
3. Arrow kind signatures for `***` / `&&&` / `|||`, so that the
   categorical combinators carry their input/output arity in the type
   system rather than only in the term grammar.
4. A desugaring specification for the Addendum forms (`while`, `do while`,
   `for`, `??`, `?.`, spread), defining precisely how each expands into `rec` /
   `match` / `|||` / structural construction, including the feedback-wire
   naming rule for `while` / `do while` / `for`, and the structural override
   rule for spread.
5. Row variable support in the type solver (ID.7 above), building on
   [ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md).
6. A reference parser and pretty-printer for the v0.1 surface, targeting
   the kernel IR from ADR-0052.

The accompanying *Arrow Language Specification v0.1* document (in
`platform/research/arrow-calculus-kernel-poc/`) is the authoritative source
for the surface syntax as accepted by this ADR, with the two corrections
and the control-flow/construction addendum above applied.

## Related ADRs

- [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) —
  defines the kernel (`arr` / `>>>` / `first` / `choice` / `loop` / `app`)
  that this surface syntax denotes.
- [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md) — the
  module model and `import` syntax this surface syntax programs against.
- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
  — the fixpoint discipline that `rec` regions will rely on for typing.
- [ADR-0073 — State Machine Invariants](./0073-state-machine-invariants.md)
  — `loop`-based fixpoint discovery; `rec` is the surface counterpart,
  and `while` / `for` desugar to it.
- [ADR-0072 — Concurrent Arrow Calculus](./0072-concurrent-arrow-calculus.md)
  — the loop-over-event-stream pattern that `for x in xs` is the surface
  counterpart of for collection iteration.
- [ADR-0075 — Traits as Arrow Signatures](./0075-traits-as-arrow-signatures.md)
  — the trait system (`trait`/`impl`/`law`, trait inference, catalog).
  Required for `for x in xs` (Iterator trait) and in scope for v0.1.
- [ADR-0055 — Branded Values](./0055-branded-values.md) — nominal tagging
  for structural types, used when nominal identity is needed.
- [ADR-0053 — Generic Pratt Parser](./0053-generic-pratt-parser.md) —
  parser infrastructure the v0.1 surface will be implemented against.
- [ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md)
  — typechecking infrastructure for the structural inference, row
  polymorphism, and `rec` fixpoint checks.
