**handl_fields > expression**

# Module: expression

## Contents

**Structs**

- [`VmEval`](#vmeval)

**Functions**

- [`eval_body`](#eval_body) - Evaluates a user-defined function body expression.
- [`eval_expression`](#eval_expression) - Evaluates a single expression to a `JsonValue`.

---

## handl_fields::expression::VmEval

*Struct*

**Generic Parameters:**
- H

**Fields:**
- `host: H`
- `namespaces: crate::types::Namespaces`
- `dialect: &'static dyn Dialect`

**Methods:**

- `fn new(host: H, namespaces: Namespaces, dialect: &'static dyn Dialect) -> Self`

**Trait Implementations:**

- **Eval**
  - `fn eval_expr(self: &Self, expr: &str, vars: &[(String, NonFunctionValue)]) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> VmEval<H>`



## handl_fields::expression::eval_body

*Function*

Evaluates a user-defined function body expression.

Params are merged into the default namespace (shadowing fields
with the same name), so bare names like `x` resolve via shorthand.
The host is passed through, so the body can call host primitives
and other user-defined functions.

Returns `None` if the body cannot be evaluated.

```rust
fn eval_body<H>(body: &str, params: &[(String, serde_json::Value)], namespaces: &crate::types::Namespaces, host: &H, dialect: &'static dyn Dialect) -> Option<serde_json::Value>
```



## handl_fields::expression::eval_expression

*Function*

Evaluates a single expression to a `JsonValue`.

The expression uses the syntax defined by the dialect:
- Function calls: `add(1, 2)`, `mul(x, 2)`
- Path lookups: `doc.name`, `env.HOME`, `name` (shorthand for `doc.name`)
- Literals: `"string"`, `123`, `true`, `false`, `null`

Returns `None` if the expression cannot be resolved.

```rust
fn eval_expression<H>(expr: &str, namespaces: &crate::types::Namespaces, host: &H, dialect: &'static dyn Dialect) -> Option<serde_json::Value>
```



