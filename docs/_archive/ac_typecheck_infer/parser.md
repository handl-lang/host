**handl_typecheck_infer > parser**

# Module: parser

## Contents

**Structs**

- [`HandlerClauseAst`](#handlerclauseast) - A parsed handler clause for type inference.

**Enums**

- [`ExprAst`](#exprast) - A parsed expression AST — the intermediate representation for type inference.

**Functions**

- [`find_matching_close`](#find_matching_close) - Find the matching close delimiter for the open delimiter at `start`,
- [`parse_args`](#parse_args) - Parse comma-separated arguments into a list of [`ExprAst`].
- [`parse_expr`](#parse_expr) - Parse an expression string into an [`ExprAst`].
- [`parse_literal`](#parse_literal) - Parse a literal value into an [`ExprAst::Literal`].

---

## handl_typecheck_infer::parser::ExprAst

*Enum*

A parsed expression AST — the intermediate representation for type inference.

**Variants:**
- `Literal(handl_typecheck_core::Type)` - A literal value with its inferred type.
- `Path(String)` - A path lookup (e.g., `doc.name`, `env.HOME`, `name`).
- `Call(String, Vec<ExprAst>)` - A function call: name + argument expressions.
- `Handle{ body: Box<ExprAst>, handlers: Vec<HandlerClauseAst> }` - A handle expression: body + handler clauses.

**Traits:** Eq

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &ExprAst) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> ExprAst`



## handl_typecheck_infer::parser::HandlerClauseAst

*Struct*

A parsed handler clause for type inference.

**Fields:**
- `op_name: String` - The operation name to match (e.g. `"fetch"`).
- `params: Vec<String>` - Parameter names bound to the operation's arguments.
- `continuation_name: String` - The name bound to the captured continuation.
- `body: Box<ExprAst>` - The handler body expression.

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HandlerClauseAst`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &HandlerClauseAst) -> bool`



## handl_typecheck_infer::parser::find_matching_close

*Function*

Find the matching close delimiter for the open delimiter at `start`,
respecting nested delimiters and string literals.

```rust
fn find_matching_close(s: &str, start: usize, open: char, close: char) -> Option<usize>
```



## handl_typecheck_infer::parser::parse_args

*Function*

Parse comma-separated arguments into a list of [`ExprAst`].

```rust
fn parse_args(s: &str, dialect: &dyn Dialect) -> Option<Vec<ExprAst>>
```



## handl_typecheck_infer::parser::parse_expr

*Function*

Parse an expression string into an [`ExprAst`].

Returns `None` if the expression cannot be parsed.

```rust
fn parse_expr(s: &str, dialect: &dyn Dialect) -> Option<ExprAst>
```



## handl_typecheck_infer::parser::parse_literal

*Function*

Parse a literal value into an [`ExprAst::Literal`].

```rust
fn parse_literal(s: &str, dialect: &dyn Dialect) -> Option<ExprAst>
```



