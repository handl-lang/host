**ac_runtime > eval**

# Module: eval

## Contents

**Traits**

- [`AsyncEval`](#asynceval) - An async callback that evaluates an expression string with variable bindings.
- [`Eval`](#eval) - A callback that evaluates an expression string with variable bindings.

---

## ac_runtime::eval::AsyncEval

*Trait*

An async callback that evaluates an expression string with variable bindings.

Like [`Eval`], but returns a `Future`. This allows async host
functions (e.g. `fp.map` with an async predicate) to evaluate
expression strings that may themselves contain async calls.

The async VM ([`VirtualMachine::eval_async`](crate::VirtualMachine::eval_async))
uses this when set on a [`Context`](crate::Context) via
[`Context::set_async_eval`](crate::Context::set_async_eval).

**Methods:**

- `eval_expr_async`: Evaluate an expression string with the given variable bindings.



## ac_runtime::eval::Eval

*Trait*

A callback that evaluates an expression string with variable bindings.

This trait bridges the VM's [`Function`] trait to the higher-level
expression evaluator (parser + resolver). It allows host functions
like `fp.map` to evaluate function bodies — expression strings that
reference bound parameters.

The implementor is responsible for parsing the expression string,
resolving variable references, and dispatching nested function calls.

## Implementing

Typically implemented in the compiler layer, where the dialect parser
and function resolver are available:

```ignore
impl Eval<MyValue> for ExprEval {
    fn eval_expr(
        &self,
        expr: &str,
        vars: &[(String, MyValue)],
    ) -> Result<MyValue, Error<MyValue>> {
        // Parse expr, bind vars, evaluate, return result
    }
}
```

**Methods:**

- `eval_expr`: Evaluate an expression string with the given variable bindings.



