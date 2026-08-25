**handl_ast_canonical > expr**

# Module: expr

## Contents

**Structs**

- [`CanonicalExprFactory`](#canonicalexprfactory) - The canonical handl expression factory.
- [`HandlerClause`](#handlerclause) - A single handler clause in a [`Expr::Handle`] expression.

**Enums**

- [`Expr`](#expr) - An expression in the handl language.
- [`Pattern`](#pattern) - A pattern used in binding positions: left-hand side of `=` or

---

## handl_ast_canonical::expr::CanonicalExprFactory

*Struct*

The canonical handl expression factory.

Implements `ExprFactory<Expr<V>, V>` so lexers, parsers, and embedded
consumers can build `Expr` trees through the generic factory interface.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> CanonicalExprFactory`
- **Default**
  - `fn default() -> CanonicalExprFactory`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **ExprFactory**
  - `fn value(self: &Self, value: V) -> Expr<V>`
  - `fn call<S>(self: &Self, name: S, args: Vec<Expr<V>>) -> Expr<V>`
  - `fn apply(self: &Self, target: Expr<V>, args: Vec<Expr<V>>) -> Expr<V>`
  - `fn array(self: &Self, elements: Vec<Expr<V>>) -> Expr<V>`
  - `fn object(self: &Self, entries: Vec<(String, Expr<V>)>) -> Expr<V>`
  - `fn index(self: &Self, target: Expr<V>, index: Expr<V>) -> Expr<V>`



## handl_ast_canonical::expr::Expr

*Enum*

An expression in the handl language.

Expressions form a tree that the VM evaluates to produce a
value. Ten kinds:

- [`Expr::Value`] — a literal value.
- [`Expr::Var`] — a bare identifier value.
- [`Expr::Assign`] — a pattern binding.
- [`Expr::Function`] — a first-class function literal.
- [`Expr::Call`] — an invocation of a host-provided function.
- [`Expr::Apply`] — an invocation of an arbitrary function value.
- [`Expr::Handle`] — a handler expression that intercepts operations.
- [`Expr::Array`] — an array literal with element expressions.
- [`Expr::Object`] — an object literal with key-value expression pairs.
- [`Expr::Index`] — a postfix index: `arr[0]`, `obj["key"]`, `obj[varName]`.
- [`Expr::FieldAccess`] — a postfix field access: `a.b`, `obj.key`.

**Generic Parameters:**
- V

**Variants:**
- `Value(V)` - A literal value.
- `Var(String)` - A bare identifier value — a variable or a namespaced type like
- `Assign{ target: Pattern, value: Box<Expr<V>> }` - An assignment — `pattern = value`.
- `Function{ type_params: Vec<String>, params: Vec<(Pattern, Option<Box<Expr<V>>>)>, ret: Option<Box<Expr<V>>>, body: String }` - A first-class function literal.
- `Call{ name: String, args: Vec<Expr<V>>, type_args: Vec<String> }` - A call to a host-provided function.
- `Apply{ target: Box<Expr<V>>, args: Vec<Expr<V>> }` - An invocation of an arbitrary expression that evaluates to a
- `Handle{ body: Box<Expr<V>>, handlers: Vec<HandlerClause<V>> }` - A handler expression — intercepts operations performed in `body`.
- `Array(Vec<Expr<V>>)` - An array literal — `[expr, expr, ...]`.
- `Object(Vec<(String, Expr<V>)>)` - An object literal — `{key: expr, key: expr, ...}`.
- `Index{ target: Box<Expr<V>>, index: Box<Expr<V>> }` - A postfix index — `arr[0]`, `obj["key"]`, `obj[varName]`.
- `FieldAccess{ target: Box<Expr<V>>, field: String }` - A postfix field access — `a.b`, `obj.key`.

**Methods:**

- `fn value(value: V) -> Self` - Creates a value expression.
- `fn call<impl Into<String>>(name: impl Trait, args: Vec<Expr<V>>) -> Self` - Creates a call expression.
- `fn call_with_type_args<impl Into<String>>(name: impl Trait, args: Vec<Expr<V>>, type_args: Vec<String>) -> Self` - Creates a call expression with resolved type arguments.
- `fn apply(target: Expr<V>, args: Vec<Expr<V>>) -> Self` - Creates an apply expression that calls an arbitrary function value.
- `fn handle(body: Expr<V>, handlers: Vec<HandlerClause<V>>) -> Self` - Creates a handle expression.
- `fn array(elements: Vec<Expr<V>>) -> Self` - Creates an array literal expression.
- `fn object(entries: Vec<(String, Expr<V>)>) -> Self` - Creates an object literal expression.
- `fn index(target: Expr<V>, index: Expr<V>) -> Self` - Creates a postfix index expression.
- `fn field_access<impl Into<String>>(target: Expr<V>, field: impl Trait) -> Self` - Creates a postfix field access expression.
- `fn var<impl Into<String>>(name: impl Trait) -> Self` - Creates a variable expression.
- `fn assign(target: Pattern, value: Expr<V>) -> Self` - Creates an assignment expression.
- `fn function<impl Into<String>>(type_params: Vec<String>, params: Vec<(Pattern, Option<Box<Expr<V>>>)>, ret: Option<Box<Expr<V>>>, body: impl Trait) -> Self` - Creates a first-class function expression.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Expr<V>) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Expr<V>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_ast_canonical::expr::HandlerClause

*Struct*

A single handler clause in a [`Expr::Handle`] expression.

**Generic Parameters:**
- V

**Fields:**
- `op_name: String` - The operation name to match (e.g. `"fetch"`).
- `params: Vec<String>` - Parameter names bound to the operation's arguments.
- `continuation_name: String` - The name bound to the captured continuation.
- `body: Expr<V>` - The handler body expression.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &HandlerClause<V>) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> HandlerClause<V>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_ast_canonical::expr::Pattern

*Enum*

A pattern used in binding positions: left-hand side of `=` or
function parameters.

**Variants:**
- `Identifier(String)` - A simple identifier binding, e.g. `x`.
- `Array(Vec<Pattern>)` - An array destructuring pattern, e.g. `[a, b]` or `[a, ...rest]`.
- `Object(Vec<(String, Pattern)>)` - An object destructuring pattern, e.g. `{x, y}` or `{x: a, y: b}`.
- `Rest(String)` - A rest pattern, e.g. `...rest`, capturing the remainder of

**Methods:**

- `fn bound_names(self: &Self) -> Vec<String>` - Returns the set of variable names bound by this pattern, in

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Pattern) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Pattern`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



