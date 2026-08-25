**ac_runtime_config > canonical_context > CanonicalContext**

# Module: canonical_context::CanonicalContext

## Contents

**Functions**

- [`eval_expr`](#eval_expr) - Evaluates an expression string with the given variable bindings.

---

## ac_runtime_config::canonical_context::CanonicalContext::eval_expr

*Function*

Evaluates an expression string with the given variable bindings.

Delegates to the [`Eval`] callback set via [`Context::set_eval`].
Returns [`Error::PrimitiveError`] if no callback is set.

```rust
fn eval_expr(self: &Self, expr: &str, vars: &[(String, NonFunctionValue)]) -> Result<NonFunctionValue, Error<NonFunctionValue>>
```



