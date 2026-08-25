**handl_ast_canonical > stmt**

# Module: stmt

## Contents

**Structs**

- [`CanonicalStmtFactory`](#canonicalstmtfactory) - The canonical handl statement factory.

**Enums**

- [`Stmt`](#stmt) - A statement in the handl language.

---

## handl_ast_canonical::stmt::CanonicalStmtFactory

*Struct*

The canonical handl statement factory.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> CanonicalStmtFactory`
- **Default**
  - `fn default() -> CanonicalStmtFactory`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **StmtFactory**
  - `fn expression(self: &Self, expr: Expr<V>) -> Stmt<V>`
  - `fn let_<N>(self: &Self, name: N, value: Expr<V>) -> Stmt<V>`
  - `fn block(self: &Self, stmts: Vec<Stmt<V>>) -> Stmt<V>`



## handl_ast_canonical::stmt::Stmt

*Enum*

A statement in the handl language.

Statements are top-level program elements that do not produce a value
themselves, but contain expressions that do. `Stmt` is generic over the
value type `V` so it can share the same value representation as `Expr`.

**Generic Parameters:**
- V

**Variants:**
- `Expression(crate::Expr<V>)` - An expression used as a statement.
- `Let{ name: String, value: crate::Expr<V> }` - A name bound to an expression value.
- `Block(Vec<Stmt<V>>)` - A sequence of statements evaluated in order.

**Methods:**

- `fn expression(expr: Expr<V>) -> Self` - Creates an expression statement.
- `fn let_<impl Into<String>>(name: impl Trait, value: Expr<V>) -> Self` - Creates a `let` statement.
- `fn block(stmts: Vec<Stmt<V>>) -> Self` - Creates a block of statements.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Stmt<V>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Stmt<V>) -> bool`



