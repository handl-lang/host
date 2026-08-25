**ac_runtime > runtime**

# Module: runtime

## Contents

**Traits**

- [`Context`](#context) - The execution context for a VM evaluation.
- [`VirtualMachine`](#virtualmachine) - An abstract virtual-machine evaluator.

---

## ac_runtime::runtime::Context

*Trait*

The execution context for a VM evaluation.

A `Context` maintains a stack of variable scopes, resource limits,
and optional expression-evaluation callbacks. It is the only thing the
VM knows about the execution environment; concrete context types can
implement this trait with different backing stores or policies.

**Methods:**

- `define`: Defines a variable in the current (topmost) scope.
- `get`: Looks up a variable by name, searching from the innermost
- `push_scope`: Pushes a new empty scope onto the scope stack.
- `pop_scope`: Pops the topmost scope from the stack.
- `depth`: Returns the current scope depth.
- `call_count`: Returns the number of calls made so far in this context.
- `increment_call_count`: Increments the call counter. Called by the VM on each `Call`.
- `check_depth`: Checks whether the current scope depth is within the limit.
- `has_eval`: Returns `true` if a synchronous eval callback is set.
- `has_async_eval`: Returns `true` if an asynchronous eval callback is set.
- `set_eval`: Sets the synchronous eval callback.
- `set_async_eval`: Sets the asynchronous eval callback.
- `eval_expr`: Evaluates an expression string with the given variable bindings.
- `eval_expr_async`: Evaluates an expression string asynchronously with the given
- `set_max_depth`: Sets the maximum scope depth allowed during evaluation.
- `set_max_calls`: Sets the maximum number of function calls allowed during evaluation.
- `max_depth`: Returns the configured max scope depth, if any.
- `max_calls`: Returns the configured max call count, if any.
- `collect_vars`: Collects all variables from all scopes.



## ac_runtime::runtime::VirtualMachine

*Trait*

An abstract virtual-machine evaluator.

Implementors transform an [`Expr`] tree into a runtime value inside a
[`Context`]. The canonical tree-walking implementation is
`ac_runtime_canonical::CanonicalVm`.

**Methods:**

- `eval`: Evaluates an expression synchronously.
- `eval_async`: Evaluates an expression asynchronously.



