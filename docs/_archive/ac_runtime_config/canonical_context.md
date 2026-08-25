**ac_runtime_config > canonical_context**

# Module: canonical_context

## Contents

**Structs**

- [`CanonicalContext`](#canonicalcontext) - The execution context for a VM evaluation.

---

## ac_runtime_config::canonical_context::CanonicalContext

*Struct*

The execution context for a VM evaluation.

A context maintains a stack of variable scopes. Each scope is
a map of name-value pairs with O(1) lookup. Variable lookup
searches from the innermost scope outward, returning the first
match.

Scopes are pushed and popped to model function call boundaries.
The bottommost scope is the global scope and is never popped.

The context may also hold an [`Eval`] callback, which allows host
functions to evaluate expression strings (e.g. function bodies).
The callback is optional — when not set, [`Context::eval_expr`]
returns an error.

**Fields:**
- `scopes: Vec<indexmap::IndexMap<String, crate::NonFunctionValue>>`
- `eval: Option<Box<dyn Eval>>`
- `async_eval: Option<Box<dyn AsyncEval>>`
- `max_depth: Option<usize>`
- `max_calls: Option<usize>`
- `call_count: std::cell::Cell<usize>`

**Methods:**

- `fn new() -> Self` - Creates a new context with a single empty global scope.
- `fn from_namespaces(namespaces: &HashMap<String, JsonValue>) -> Self` - Creates a context with each namespace root bound as a variable.
- `fn set_eval(self: & mut Self, eval: Box<dyn Eval>)` - Sets the [`Eval`] callback for this context.
- `fn has_eval(self: &Self) -> bool` - Returns `true` if an [`Eval`] callback is set.
- `fn eval_expr(self: &Self, expr: &str, vars: &[(String, NonFunctionValue)]) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Evaluates an expression string with the given variable bindings.
- `fn set_async_eval(self: & mut Self, eval: Box<dyn AsyncEval>)` - Sets the [`AsyncEval`] callback for this context.
- `fn has_async_eval(self: &Self) -> bool` - Returns `true` if an [`AsyncEval`] callback is set.
- `fn eval_expr_async<'a>(self: &'a Self, expr: &'a str, vars: &'a [(String, NonFunctionValue)]) -> Pin<Box<dyn Future>>` - Evaluates an expression string asynchronously with the given
- `fn push_scope(self: & mut Self)` - Pushes a new empty scope onto the scope stack.
- `fn pop_scope(self: & mut Self) -> Option<IndexMap<String, NonFunctionValue>>` - Pops the topmost scope from the stack.
- `fn depth(self: &Self) -> usize` - Returns the current scope depth.
- `fn set_max_depth(self: & mut Self, max: usize)` - Sets the maximum scope depth allowed during evaluation.
- `fn set_max_calls(self: & mut Self, max: usize)` - Sets the maximum number of function calls allowed during evaluation.
- `fn max_depth(self: &Self) -> Option<usize>` - Returns the configured max scope depth, if any.
- `fn max_calls(self: &Self) -> Option<usize>` - Returns the configured max call count, if any.
- `fn call_count(self: &Self) -> usize` - Returns the number of calls made so far in this context.
- `fn increment_call_count(self: &Self) -> Result<(), Error<NonFunctionValue>>` - Increments the call counter. Called by the VM on each `Call`.
- `fn check_depth(self: &Self) -> Result<(), Error<NonFunctionValue>>` - Checks whether the current scope depth is within the limit.
- `fn define<impl Into<String>>(self: & mut Self, name: impl Trait, value: NonFunctionValue)` - Defines a variable in the current (topmost) scope.
- `fn get(self: &Self, name: &str) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Looks up a variable by name, searching from the innermost
- `fn collect_vars(self: &Self) -> Vec<(String, NonFunctionValue)>` - Collects all variables from all scopes, with innermost scope

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Context**
  - `fn define(self: & mut Self, name: &str, value: NonFunctionValue)`
  - `fn get(self: &Self, name: &str) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
  - `fn push_scope(self: & mut Self)`
  - `fn pop_scope(self: & mut Self) -> Option<IndexMap<String, NonFunctionValue>>`
  - `fn depth(self: &Self) -> usize`
  - `fn call_count(self: &Self) -> usize`
  - `fn increment_call_count(self: &Self) -> Result<(), Error<NonFunctionValue>>`
  - `fn check_depth(self: &Self) -> Result<(), Error<NonFunctionValue>>`
  - `fn has_eval(self: &Self) -> bool`
  - `fn has_async_eval(self: &Self) -> bool`
  - `fn set_eval(self: & mut Self, eval: Box<dyn Eval>)`
  - `fn set_async_eval(self: & mut Self, eval: Box<dyn AsyncEval>)`
  - `fn eval_expr(self: &Self, expr: &str, vars: &[(String, NonFunctionValue)]) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
  - `fn eval_expr_async<'a>(self: &'a Self, expr: &'a str, vars: &'a [(String, NonFunctionValue)]) -> Pin<Box<dyn Future>>`
  - `fn set_max_depth(self: & mut Self, max: usize)`
  - `fn set_max_calls(self: & mut Self, max: usize)`
  - `fn max_depth(self: &Self) -> Option<usize>`
  - `fn max_calls(self: &Self) -> Option<usize>`
  - `fn collect_vars(self: &Self) -> Vec<(String, NonFunctionValue)>`



