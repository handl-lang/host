# Expression Object and Array Literals

## Context

handl expressions currently support only scalar literals (`"..."`, `42`, `true`, `false`, `null`), path lookups (`doc.name`), and function calls (`add(1, 2)`). Array bracket syntax `[a, b, c]` is already partially recognized in the runtime argument evaluator, but it is not consistently supported across the whole expression parser. Object literals `{...}` are not supported at all.

A number of proposed features — most immediately an handl-native `s.*` vocabulary — need to construct record and array values inside expressions. Example:

```yaml
$:
  config:
    schema: s.record({ host: s.string, port: s.number })
  tags:
    schema: s.array(s.string)
```

Without object and array literals, record and array schemas must be written as nested function calls such as `s.record("host", s.string, "port", s.number)`. This is verbose, order-sensitive, and does not scale to nested records. Object and array literals are the minimal extension that makes the `s.*` DSL readable.

## Decision

Add first-class object and array literal syntax to handl expressions.

- **Array literal:** `[expr, expr, ...]`
- **Object literal:** `{ key: expr, key: expr, ... }`, where a key is a bare identifier or a quoted string and a value is any handl expression.

The canonical expression parser will:

1. Treat `{` and `}` as nesting tokens alongside `(` and `[` in argument splitting and depth tracking (`split_args`, `split_handler_clauses`, `find_block_open`).
2. Add `parse_array_literal` and `parse_object_literal` to the `ExprParser` trait. Implementations will parse the content recursively, reusing `split_args` for element/field separation and the existing literal parsers for leaf values.
3. Use the existing `Expr::Value` variant for object and array results; no new AST node is required because `V` is already generic over JSON-compatible values.
4. Update the runtime `eval_arg` and the typechecker `parse_expr` to recognize `[]` and `{}` as value expressions, evaluate/parse their contents recursively, and infer `Type::Array(...)` / `Type::Object(...)`.

This is a language-level change, not a frontend-only change: it affects `ExprParser`, `handl-fields` expression evaluation, and `handl-typecheck-infer` type inference.

## Consequences

**Positive:**

- Enables `s.record({...})` and `s.array(...)` as the primary record/array schema constructors.
- Makes handl expression syntax closer to the JSON values it already manipulates, reducing the gap between data and expression syntax.
- The array literal `[...]` formalizes behavior that already partially exists in `handl-fields` `eval_arg`.
- No new `Expr` AST variants are required; `Expr::Value` already holds any `V`.

**Negative:**

- `,` and `:` inside `{}` and `[]` must be disambiguated from function argument and handler-clause separators. This increases parser complexity and the risk of subtle bugs in `split_args` and `split_handler_clauses`.
- `find_block_open` must be careful not to confuse an object literal with a `handle` block. We keep the rule that `handle` is recognized by the `handle` keyword before the block, not by the first `{`.
- The `:` in object field syntax becomes a reserved character inside object literals, although it is otherwise not a special token in handl.
- Type inference for object literals must recursively infer field types; the existing `Type` enum already supports `Object(BTreeMap<String, Type>)` and `Array(Box<Type>)`.

**Open questions (out of scope for this ADR):**

- Should object keys allow arbitrary expressions or only identifiers/quoted strings?
- Should object/array literals be allowed in the `.handl` frontend and in handler body contexts?
- Should this be gated by a new dialect version/flag, or is it a non-breaking addition to the canonical dialect?

**Non-goals:**

- This ADR does not define the `s.*` vocabulary itself; that will be a separate ADR. It only provides the literal syntax required by it.
- It does not add object property access syntax (`obj.key` is still a path lookup), nor does it add methods or computed field access.

## Status

Proposed
