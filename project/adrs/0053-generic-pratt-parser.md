# ADR-0053: Generic Pratt Parser

## Status

Proposed

Refines [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
§3 "Grammar rows as data, engines as interpreters": completes the data-driven
promise for the parse operation by removing the five hardcoded special-case
branches in `plugin-engine-parse-pratt` and the substring-heuristic literal
dispatch in `atom_payload`. After this ADR, adding a Pratt-expressible
construct to handl requires only new grammar/token/AST/eval data rows — no
parser source changes.

## Context

ADR-0044 §3 specifies that the parse operation is driven by `Language.grammar`
data rows: a Pratt engine reads `GrammarRow`s keyed by `(token, position)` and
executes the prefix/infix loop from data. The shipped engine
(`crates/plugin-engine-parse-pratt/src/lib.rs`) honors this for the **spine**
— prefix/infix dispatch, `lbp < rbp` precedence, and the generic
`parse_delimited_block` driven by `DelimitedShape` — but `expression_inner`
contains five special-case branches that bypass the grammar and dispatch on
hardcoded token-kind and node-kind strings:

1. **`if/then/else`** — `expression_inner` intercepts `token.keyword.if`
   before the prefix lookup and calls `parse_if`, which hardcodes the
   `match cond { true: a, _: b }` desugar and the literal node kinds
   `bool`/`var`/`arm`/`block`/`match`. A `grammar.if.prefix` row exists in
   `data/grammar.yaml` (arity 3, `node_kind: if`) but is shadowed and never
   reached.

2. **`x = y in body`** — the infix loop intercepts `token.equal` before the
   infix lookup and calls `parse_binding`, which hardcodes `token.keyword.in`
   and the `bind` node shape. A `grammar.assign.infix` row exists (arity 2,
   `node_kind: assign`) but is shadowed.

3. **`json{...}(args)`** — the prefix path special-cases `kind == "json"`,
   hardcodes the `"json{"`/`"}"` token-text shape and the `(arg1, …)` clause
   against `token.lparen`/`token.comma`/`token.rparen`. A `grammar.json.prefix`
   row exists (arity 0) but is shadowed for the text-stripping and arg clause.

4. **`array`/`object`/`paren` literals** — the prefix path special-cases
   `kind == "array" || "object" || "paren"`, hardcodes
   `token.rbracket`/`token.rbrace`/`token.rparen`, `token.comma`,
   `token.colon`, `token.ellipsis`, and the `entry` node kind. Grammar rows
   exist for all three but are shadowed.

5. **Variable-arity `call`** — the infix path special-cases
   `infix_kind == "call"`, hardcodes a comma/`token.rparen` arg-parsing loop
   after the fixed-arity grammar row has already been applied. The code
   comment admits the grammar row stays arity 2 and this branch patches
   around it.

Additionally, `atom_payload` dispatches literal interpretation by
**substring match** on the token kind id: `kind_id.contains("number")`,
`contains("bool")`, `contains("null")`, `contains("string")`. This is
data-adjacent (it reads `lang.tokens`) but the dispatch is by naming
convention, not by a typed field on `TokenKindDef`.

### Why this matters

ADR-0044's defining property is that a `Language` carries no Rust: it is
serializable, inspectable, authorable as text, and loadable by a runtime with
no compile-time knowledge of it. The five special-case branches violate this
property for the parser: each is keyed to handl's specific token-kind naming
scheme (`token.keyword.if`, `token.lparen`, …) and handl's specific AST node
kinds (`match`, `arm`, `bind`, `entry`, `call`). A different language reusing
`plugin-engine-parse-pratt` would either need the same naming conventions or
would silently fail to parse these constructs. Adding a new construct in the
same family (e.g. `switch/case/default`, `try/catch`, a ternary `? :`)
currently requires editing `expression_inner`, not just adding data rows.

### What is already generic

The engine's core is sound and stays as-is:

- `lookup_row(lang, token_kind, position)` — generic grammar lookup.
- The prefix/infix dispatch and `lbp < rbp` precedence check.
- `parse_delimited_block` — driven entirely by the `DelimitedShape`
  descriptor (`open`/`close`/`separator`/`pair`); already used by `match`
  and `handle` without hardcoding.
- The prefix path's `delimited` handling for the **last** child of a prefix
  shape (the `{ arms }` of `match x { ... }`).

The gap is that `delimited` is only consulted on the prefix path, the
delimiter-is-the-operator-token case (array/object/paren/call) is not
expressed, the `child_separators` between prefix/infix children is not
expressed, literal interpretation is by substring, and the two desugars
(`if → match`, `assign → bind`) live in parser code instead of data.

## Decision

Extend the grammar/token data schema with four new descriptor fields, then
delete all five special-case branches and the substring dispatch from
`expression_inner`. The parser becomes a fixed interpreter of grammar data:
no string literals, no `kind == "..."`, no `token.kind == "..."`.

### 1. `TokenKindDef.literal: Option<LiteralKind>` — typed literal dispatch

Replaces the `kind_id.contains(...)` heuristics in `atom_payload`.

```rust
pub enum LiteralKind {
    /// Integer (fallback: float). Replaces `contains("number")`.
    Number,
    /// `"true"` → true, anything else → false. Replaces `contains("bool")`.
    Bool,
    /// Always `Value::Null`. Replaces `contains("null")`.
    Null,
    /// Strip `delim_chars` from each end, apply the escape table.
    /// Replaces the hardcoded quote-strip + `\\n`/`\\t`/`\\\"`/`\\\\` table.
    String {
        delim_chars: usize,
        escapes: Vec<(String, String)>,
    },
    /// Strip a fixed prefix and suffix, optionally wrap the remainder.
    /// Covers `json{...}`: prefix `"json{"`, suffix `"}"`, wrap `Some(("{", "}"))`.
    StripAffix {
        prefix: String,
        suffix: String,
        wrap: Option<(String, String)>,
    },
    /// Use the token text verbatim. The default for identifiers and keywords.
    Raw,
}
```

`atom_payload` dispatches on `kind_def.literal.unwrap_or(LiteralKind::Raw)`.
No substring matching. Adding a new literal form (e.g. regex, raw string) is
a new `LiteralKind` variant + a token row, not a parser edit.

**Layering note:** string unescaping is arguably a lexer concern — the lexer
could emit interpreted values rather than raw text. This ADR keeps it in
`atom_payload` because the current lexer emits raw text and changing the
lexer's token-value contract is a larger, separate change. The escape table
being data (not hardcoded) means a future lexer-side move is a relocation,
not a redesign.

### 2. `AstShapeRef.child_separators: Vec<ChildSeparator>` — separators between children

Replaces the `if/then/else` and `x = y in body` hardcoding.

```rust
pub enum ChildSeparator {
    /// The next token must have this kind; it is consumed.
    Required(String),
    /// If the next token has this kind, it is consumed; otherwise skipped.
    Optional(String),
    /// The next token must have one of these kinds; the match is consumed.
    OneOf(Vec<String>),
}
```

The generic prefix loop, after parsing child `i` and before child `i+1`,
consumes `child_separators[i]` if present. The generic infix loop, after the
operator and the first operand, consumes `child_separators[i]` between
remaining operands. `if` becomes a prefix row with
`child_separators: [Required("token.keyword.then"), Required("token.keyword.else")]`.
`=` becomes an infix row with
`child_separators: [Required("token.keyword.in")]` and `node_kind: bind`,
arity 3 (left, value, body) — the desugar is gone, the row directly produces
the `bind` node that ADR-0052 already defines.

### 3. `DelimitedShape.open_is_operator_token: bool` — operator-token-as-open

Replaces the `array`/`object`/`paren`/`call` hardcoding. When true, the open
delimiter is the prefix/infix token itself (already consumed by
`cursor.advance()` in the caller), so `parse_delimited_block` skips the
open-consume step and begins at element parsing.

`array`/`object`/`paren` become prefix rows with `delimited` set:
`open_is_operator_token: true`, `close: token.rbracket`/`token.rbrace`/
`token.rparen`, `separator: token.comma`, `container_kind: array`/`object`/
`paren`. `object` additionally sets `pair: { separator: token.colon,
node_kind: entry }`.

### 4. `DelimitedShape.flatten: bool` — splice elements into parent's children

Resolves the `call` AST-shape problem. The current `call` node has flat
`children: [callee, arg1, arg2, ...]`; every eval engine consumes this
shape. If `call` became arity 1 + a nested args container node, all six
engine paths (v1 + v2 × tree-walk/bytecode/cranelift) would break.

`flatten: true` tells `parse_delimited_block` to **not** emit a container
node — instead, the parsed element indices are spliced directly into the
parent node's `children` array. `call` becomes an infix row with arity 1
(callee) + `delimited: { open_is_operator_token: true, close: token.rparen,
separator: token.comma, container_kind: args, flatten: true }`. The produced
node is `call { children: [callee, arg1, arg2, ...] }` — identical to today,
no eval changes.

### 5. `DelimitedShape.rest_token: Option<String>` — rest/spread elements

Replaces the `token.ellipsis` special case inside the object/array loops.
When the next token matches `rest_token`, `parse_delimited_block` parses a
bare expression (via `expression_inner`) instead of a pair, and pushes it
as a regular element. `object` and `array` rows set
`rest_token: Some("token.ellipsis")`.

### 6. Infix path consults `delimited`

The infix path in `expression_inner` currently has the hardcoded `call`
branch. After this ADR, the infix path mirrors the prefix path: when
`infix_row.child_shape.delimited` is set, the last child is parsed by
`parse_delimited_block` (with `open_is_operator_token` and `flatten` honored).

### 7. Desugars move to a data-driven post-parse pass

The `if → match` desugar currently lives in `parse_if`. After this ADR,
`if` is a real AST node produced by a grammar row (arity 3, no desugar in
the parser). The `if → match` rewrite becomes a **data-driven desugar pass**
driven by a new optional field on `AstShapeDef`:

```rust
pub struct AstShapeDef {
    // ...existing fields...
    /// When set, nodes of this kind are rewritten by this scheme expression
    /// after parse. The scheme receives the node's children as input and
    /// produces a replacement node. Used for surface-sugar → kernel-form
    /// rewrites (e.g. `if` → `match`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desugar: Option<DesugarRule>,
}

pub struct DesugarRule {
    /// The node kind to produce.
    pub target_kind: String,
    /// A scheme expression over the node's children, producing the
    /// replacement node's children array.
    pub rewrite: Scheme,
}
```

This is a small, honest mechanism: desugars are rewrites on `Ast` nodes,
expressed as eval schemes over children. The `if` desugar rule produces a
`match` node with two `arm` children (`true: then`, `_: else`). The parser
emits the honest `if` node; the desugar pass (run between parse and eval,
in `handl-runtime`) rewrites it. No eval scheme for `if` is needed — the
existing `match` eval handles the rewritten node.

The `=`/`bind` case does **not** need a desugar: the infix row directly
produces `bind` (ADR-0052's node kind), arity 3, with `child_separators:
[Required("token.keyword.in")]`. The `grammar.assign.infix` row is deleted.

### Resulting `expression_inner` (no string literals)

```
prefix:
  token = advance
  row = lookup(token, Prefix) or error
  shape = row.child_shape
  if shape.arity == 0:
    push atom_payload(token, lang.tokens)  // dispatches on LiteralKind
  else:
    normal = shape.arity - (shape.delimited ? 1 : 0)
    children = []
    for i in 0..normal:
      children.push(expression(row.prefix_rbp))
      if i < normal - 1: consume(shape.child_separators[i])
    if shape.delimited:
      children.push(parse_delimited_block(shape.delimited))  // honors flatten
    push node(shape.node_kind, children)

infix:
  row = lookup(token, Infix) or break
  (lbp, rbp_right) = row.binding_power or error
  if lbp < rbp: break
  advance operator
  shape = row.child_shape
  normal = shape.arity - 1 - (shape.delimited ? 1 : 0)
  children = [left]
  for i in 0..normal:
    children.push(expression(rbp_right))
    if i < normal - 1: consume(shape.child_separators[i])
  if shape.delimited:
    if shape.delimited.flatten: children.extend(parse_delimited_block(...).elements)
    else: children.push(parse_delimited_block(...))
  push node(shape.node_kind, children)
```

`atom_payload`, `parse_delimited_block`, and the `consume` helper are the
only sub-routines. No `kind == "..."`, no `token.kind == "..."`.

## Consequences

### Positive

- **Adding a Pratt-expressible construct = data only.** A new ternary `? :`
  is a new token kind, an infix grammar row with `child_separators:
  [Required("token.colon")]`, an AST shape, and an eval scheme. Zero parser
  edits.
- **The parser is a fixed interpreter of grammar data**, symmetric with the
  eval engines being fixed interpreters of scheme data. ADR-0044's
  "engines as interpreters" property now holds for parse, not just eval.
- **Other languages can reuse `plugin-engine-parse-pratt`** without adopting
  handl's token-kind naming scheme.
- **`if` and `bind` become honest AST nodes** with explicit grammar rows,
  instead of parser-internal desugars that produce `match`/`bind` nodes
  invisibly. The AST a user inspects matches the surface syntax.
- **The `call` AST shape is unchanged** (`flatten: true` preserves
  `children: [callee, arg1, ...]`), so no eval engine changes are required.

### Negative

- **Six new schema fields** (`LiteralKind` enum + `TokenKindDef.literal`,
  `ChildSeparator` enum + `AstShapeRef.child_separators`,
  `DelimitedShape.open_is_operator_token`, `DelimitedShape.flatten`,
  `DelimitedShape.rest_token`, `AstShapeDef.desugar` +
  `DesugarRule`). Each is `Option`/`Vec` with `skip_serializing_if`, so
  existing YAML remains valid; the canonical YAML must be updated to use
  them.
- **A new desugar pass** runs between parse and eval in `handl-runtime`.
  This is one extra `Ast → Ast` walk per parse. It is small (only nodes
  with `AstShapeDef.desugar` set are rewritten) but it is new machinery.
- **The `json{...}(args)` case is not fully generic.** The `(args)` clause
  is parsed by the generic infix `call` path (since `json` is an atom and
  `(args)` is a normal call), producing `call(json_node, [args])`. This is
  a **semantic change**: `json` becomes a callable whose application
  substitutes `$0`/`$1`/… placeholders. The eval scheme for `json` must
  handle being applied, or a `json.call` eval scheme must be added. The
  alternative — keeping `args` in the `json` node's payload via a
  `post_parse` op — is rejected as more contorted than making `json`
  callable. This is the one construct where the generic form changes
  semantics, not just representation.
- **`LiteralKind::String` escapes are data but still parser-side.** A
  future move to lexer-side unescaping is a relocation, not a redesign,
  but until then the parser carries the escape table.

### Scope boundary — what this ADR does NOT promise

This ADR makes the parser **generic for handl's construct family and
Pratt-expressible extensions**. It does not make the parser
Turing-complete in grammar description. Constructs outside Pratt's
expressive range cannot be added by data alone:

- **List comprehensions** `[x | x <- xs, g x]` — two separators in one
  delimited block, plus a generator form. Not expressible with one
  `DelimitedShape`.
- **Multi-block constructs** `match a {} with b {}` — two delimited
  children. The current `delimited` field permits only one, as the last
  child.
- **Guard patterns** `y if g => b` — an optional prefix in a pair's rhs.
  Not expressible with the current `PairShape`.

These would require further schema extension (multiple delimited children,
nested delimited shapes) and are explicitly out of scope. The honest claim
is: **for any construct whose parse structure is a Pratt prefix/infix with
separated operands and at most one trailing delimited block, no parser code
change is needed.** That covers handl's current constructs and the obvious
extensions (ternary, `try/catch`, `switch/case/default`, postfix
operators, new literal forms).

## Status

Proposed — awaiting implementation. Migration order (lowest risk first):

1. `DelimitedShape.rest_token` + `open_is_operator_token` + `flatten` —
   unblocks `array`/`object`/`paren`/`call`; delete those four branches.
2. `AstShapeRef.child_separators` — unblocks `if` and `=`; delete
   `parse_if` and `parse_binding`.
3. `TokenKindDef.literal` — unblocks `atom_payload` rewrite; delete
   substring dispatch.
4. `AstShapeDef.desugar` + desugar pass in `handl-runtime` — relocate
   `if → match` from parser to data.
5. `json` semantic change (`call(json, args)`) — add eval scheme for
   applied `json`; update `data/eval.yaml`.

Verification gate: `cargo test --workspace`. The self-hosting corpus
(`crates/handl-defining-handl/tests/v2_parity.rs`, 42 cases × 6 engines) is
the real test — if all pass, the generic parser produces equivalent ASTs
and the desugar pass preserves eval results.
