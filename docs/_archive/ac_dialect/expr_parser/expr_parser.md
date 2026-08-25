**handl_dialect > expr_parser > expr_parser**

# Module: expr_parser::expr_parser

## Contents

**Structs**

- [`ParsedHandlerClause`](#parsedhandlerclause) - A parsed handler clause — the result of parsing a single

**Traits**

- [`ExprParser`](#exprparser) - Expression parsing operations for handl expressions.

---

## handl_dialect::expr_parser::expr_parser::ExprParser

*Trait*

Expression parsing operations for handl expressions.

These operations parse expression strings into their components —
detecting function calls, splitting arguments, and recognizing literals.
All literal parsing is dialect-configurable: a dialect that uses
`yes`/`no` for booleans or `nil` for null implements
`parse_bool_literal` and `is_null_literal` accordingly.

The value type system (see [LANG.md](../../docs/LANG.md) §3) defines the
_output_ types — boolean, null, floating-point number, string — not the
_input_ syntax. A dialect changes how literals are recognized; the values
they produce remain the same.

**Methods:**

- `find_call_open`: Detect whether `s` contains a function call opening (e.g. `(` in canonical).
- `split_args`: Split function arguments by the argument separator (`,` in canonical),
- `parse_string_literal`: Check if `s` is a string literal and return its inner content.
- `parse_number_literal`: Check if `s` is a number literal and return its value.
- `parse_bool_literal`: Check if `s` is a boolean literal and return its value.
- `is_null_literal`: Check if `s` is the null literal.
- `find_handle_open`: Detect whether `s` starts with the `handle` keyword followed by `(`.
- `find_block_open`: Find the position of the block-open token (`{` in canonical) in `s`.
- `split_handler_clauses`: Split handler clauses by the clause separator (`,` in canonical),
- `parse_handler_clause`: Parse a single handler clause string (e.g. `"fetch"(url) as k: k(42)`)



## handl_dialect::expr_parser::expr_parser::ParsedHandlerClause

*Struct*

A parsed handler clause — the result of parsing a single
`"op"(params) as k: body` clause from a handle expression.

**Fields:**
- `op_name: String` - The operation name to match (e.g. `"fetch"`).
- `params: Vec<String>` - Parameter names bound to the operation's arguments.
- `continuation_name: String` - The name bound to the captured continuation.
- `body: String` - The handler body expression string (unparsed).

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ParsedHandlerClause`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &ParsedHandlerClause) -> bool`



