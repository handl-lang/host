**ac_runtime_common > machine**

# Module: machine

## Contents

**Structs**

- [`AsyncEvalClone`](#asyncevalclone)
- [`ContextBuilder`](#contextbuilder) - Builds a canonical VM context from small, composable decisions.
- [`EvalClone`](#evalclone)
- [`Machine`](#machine) - A reusable, pre-configured VM instance.

---

## ac_runtime_common::machine::AsyncEvalClone

*Struct*

**Tuple Struct**: `(std::rc::Rc<dyn AsyncEval>)`

**Methods:**

- `fn new(inner: Rc<dyn AsyncEval>) -> Self`

**Trait Implementations:**

- **AsyncEval**
  - `fn eval_expr_async<'a>(self: &'a Self, expr: &'a str, vars: &'a [(String, NonFunctionValue)]) -> Pin<Box<dyn Future>>`



## ac_runtime_common::machine::ContextBuilder

*Struct*

Builds a canonical VM context from small, composable decisions.

You can chain variable namespaces, resource limits, and eval
callbacks without touching `CanonicalContext` directly.

**Fields:**
- `namespaces: std::collections::HashMap<String, serde_json::Value>`
- `max_depth: Option<usize>`
- `max_calls: Option<usize>`
- `eval: Option<std::rc::Rc<dyn Eval>>`
- `async_eval: Option<std::rc::Rc<dyn AsyncEval>>`

**Methods:**

- `fn new() -> Self` - Creates a new empty builder.
- `fn with_namespace<impl Into<String>>(self: Self, name: impl Trait, value: NonFunctionValue) -> Self` - Adds a single variable to the top-level namespace.
- `fn with_namespaces<I, K>(self: Self, namespaces: I) -> Self` - Adds many top-level variables at once.
- `fn with_max_depth(self: Self, max: usize) -> Self` - Sets the maximum scope depth.
- `fn with_max_calls(self: Self, max: usize) -> Self` - Sets the maximum number of function calls.
- `fn with_eval<impl Eval<NonFunctionValue> + 'static>(self: Self, eval: impl Trait) -> Self` - Sets the synchronous expression-evaluation callback.
- `fn with_async_eval<impl AsyncEval<NonFunctionValue> + 'static>(self: Self, eval: impl Trait) -> Self` - Sets the asynchronous expression-evaluation callback.
- `fn build(self: &Self) -> CanonicalContext` - Builds the configured `CanonicalContext`.

**Trait Implementations:**

- **Default**
  - `fn default() -> ContextBuilder`



## ac_runtime_common::machine::EvalClone

*Struct*

**Tuple Struct**: `(std::rc::Rc<dyn Eval>)`

**Methods:**

- `fn new(inner: Rc<dyn Eval>) -> Self`

**Trait Implementations:**

- **Eval**
  - `fn eval_expr(self: &Self, expr: &str, vars: &[(String, NonFunctionValue)]) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::machine::Machine

*Struct*

A reusable, pre-configured VM instance.

`Machine` is generic over any [`VirtualMachine`], so it works with
the canonical tree-walking VM or any future alternative implementation.
The context is built fresh on each evaluation from a [`ContextBuilder`],
keeping the VM stateless and concurrency-friendly.

**Generic Parameters:**
- V

**Fields:**
- `vm: V`
- `context: ContextBuilder`

**Methods:**

- `fn new(vm: V) -> Self` - Creates a machine from a virtual-machine implementation.
- `fn with_context<impl FnOnce(ContextBuilder) -> ContextBuilder>(self: Self, f: impl Trait) -> Self` - Configures the context builder for every evaluation.
- `fn set_context(self: Self, context: ContextBuilder) -> Self` - Sets a fresh context builder.
- `fn eval(self: &Self, expr: &Expr<NonFunctionValue>) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Evaluates an expression synchronously.
- `fn eval_async(self: &Self, expr: &Expr<NonFunctionValue>) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Evaluates an expression asynchronously.



