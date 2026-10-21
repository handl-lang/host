# Graded Modal Sanada Arrow Calculus

Handl's canonical execution calculus is the **Graded Modal Sanada Arrow
Calculus (GMSAC)**. It layers three ideas on top of one another:

1. **Arrows** — Hughes' arrow calculus, with `arr`, `>>>`, `first`, `choice`,
   `loop`, and `app`.
2. **Grades** — a row of algebraic capabilities that annotate an arrow type,
   such as `~[(Op, Query)]~>`.
3. **Sanada's handlers** — an operation request with an explicit
   continuation, and a scoped interpretation of those requests.

This layering keeps the kernel tiny and generic while allowing an open-ended
set of effect algebras to be declared in ordinary packages.

## The three layers

### Arrows (Hughes)

The base is a standard arrow calculus. Arrows are closed under sequential
composition, firsting, choice, looping, and application. These combinators are
all pure; they build computations but do not themselves interpret any effect.

### Grades (modal capabilities)

A grade row is a multiset of grade terms attached to an arrow type:

```text
f : A ~[(Op, Query)]~> B
```

`Op` is the marker grade that says "this arrow is an operation request".
`Query` is the algebra grade — the arrow is interpreted by the active
`Query` handler. Multiple grades combine in one row:

```text
g : A ~[(Op, Clock, FFI)]~> B
```

Here `Clock` and `FFI` are two algebra grades the operation carries. The
innermost handler matching any of them may intercept the operation.

The grade row is a modality. It does not just describe side effects; it
states in which contexts the arrow may be interpreted. The `Op` marker
turns a pure arrow type into a modal type.

### Sanada handlers

`Term::Op` and `Term::Handle` give the calculus deep, rebindable effects.
An `Op` is a request with an explicit continuation `k`; a handler is a
natural arrow that returns a record of operation implementations.

An operation declaration like `query : (String, Array) ~[(Op, Query)]~>
Value` lowers to `Term::Op { name: "query", grade: [Op, Query] }`.
Installing a handler scopes an arrow under it — `Term::Handle` — and every
`Op` inside whose grade row includes `Query` is dispatched to that handler.
The handler is an ordinary arrow value, not a special language construct;
handler installation is the elimination of the modal grade.

## User-space algebras

Query/command/event-style algebras are not kernel concepts. They are
packages that export grades and operation arrows:

- `Query` — a resumable request/response operation, possibly cacheable.
- `Command` / `Mutate` — a state-changing request.
- `Event` / `Emit` — a fire-and-forget notification.

All of them lower to `Term::Op` and `Term::Handle`. The runtime knows only
the calculus; it never pattern-matches on the strings "query" or "event".
The algebra and typing live in package code.

## Relationship to the surface language

Surface declarations are syntactic sugar for `Term::Op` and `Term::Handle`
bindings. Records, `do` blocks, natural arrow assignments, and graded
arrows all lower into the same GMSAC terms — the calculus never sees the
sugar.

## Summary

GMSAC lets Handl keep a small, reusable kernel — `Arr`, `Op`/`Handle`, and
the arrow combinators — while allowing arbitrary user-space effect
algebras to be declared in packages. The grade row is the user-defined
modality; the kernel only dispatches on it.
