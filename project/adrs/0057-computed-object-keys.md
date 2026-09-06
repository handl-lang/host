# ADR-0057: Computed Object Keys via Bracket Syntax

## Status

Proposed

Refines the object-literal evaluation rule introduced by
[ADR-0053 — Generic Pratt Parser](./0053-generic-pratt-parser.md) (which
defines the `{ entry: ... }` delimited form) and the arrow-calculus `object`
prim from [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md).

## Context

Object literals in handl support three key forms:

- **bare identifier** — `{ n: 5 }` uses the identifier _name_ `"n"` as the
  key (the variable is not evaluated).
- **string literal** — `{ "mykey": 5 }` uses the literal string.
- **any other expression** — `{ (k): 5 }` evaluates the expression and uses
  its value's string representation as the key.

The third form is the only way to compute a key from a variable's value, and
it requires parenthesizing the key expression. This is unfamiliar to users
coming from JavaScript, Python (PEP 692 / `**{k: v}` unpacking), and most
modern languages, where bracket syntax `[expr]:` is the idiomatic computed
key.

The handl parser already reserves `[...]` for array literals and `xs[i]` for
index access, so `[k]` in a general expression position is a one-element
array. The question is whether `[expr]:` _inside an object literal_ should be
reinterpreted as a computed key.

## Decision

Accept `[expr]: value` as a computed object key, evaluated by all three eval
engines (tree-walk, bytecode, cranelift).

**Disambiguation rule:** in object-literal entry position, a key node of kind
`array` with exactly one child is treated as a computed key — the inner
expression is evaluated and its value's string representation becomes the
key. A multi-element array, or an array appearing anywhere other than the key
slot of an entry, remains an array literal.

This is unambiguous because:

- An array literal is never followed by `:` in expression position (the
  `token.colon` infix row binds as `type-annot`, which a bare array cannot
  meaningfully take as its lhs in an entry context — the parser already
  requires `entry` nodes to be `lhs : rhs` pairs inside `{...}`).
- The `entry` node's key slot is the only place this reinterpretation
  applies; `xs[0]` (postfix index) and `[1, 2, 3]` (array literal in
  value position) are unaffected.

The pre-existing `(expr):` parenthesized form continues to work — it parses
as a `paren` node, which already falls into the "evaluate the expression"
branch. Both forms are now valid and produce the same result.

**Error case:** a bracket key with zero or more than one element
(e.g. `{ [a, b]: 1 }`) raises `object: computed key [expr] requires exactly
one element`.

## Consequences

- **Familiarity:** users from JS/TS, Python, and most modern languages can
  compute keys idiomatically.
- **No parser change required:** the Pratt parser already produces an
  `array` node for `[expr]`; the disambiguation happens entirely in the
  eval engines' object-key handling.
- **Three engines updated in lockstep:** `plugin-engine-exec-tree-walk`,
  `plugin-engine-exec-bytecode`, and `plugin-engine-exec-cranelift` each
  gain an `array`-key branch mirroring the existing `var` / `string` /
  evaluate-and-stringify cases.
- **`(expr):` remains valid** — no breaking change for existing code that
  uses parenthesized computed keys.
- **`[expr]` in value position is unchanged** — `{ k: [1, 2, 3] }` still
  produces an array value, not a computed key.
- **Pretty-printer / formatter** does not need changes; it round-trips the
  parsed tokens.
