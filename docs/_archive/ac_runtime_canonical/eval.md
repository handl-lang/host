**ac_runtime_canonical > eval**

# Module: eval

## Contents

**Structs**

- [`CanonicalEval`](#canonicaleval) - A synchronous [`Eval`] callback for the canonical handl runtime.

---

## ac_runtime_canonical::eval::CanonicalEval

*Struct*

A synchronous [`Eval`] callback for the canonical handl runtime.

Parses the expression string with the canonical parser, binds the
provided variables, and evaluates the tree with a fresh VM using
the supplied host. This enables higher-order functions like
`fn.tap`, `fn.apply`, and `fn.pipe` in the canonical VM.

**Generic Parameters:**
- H

**Fields:**
- `host: H`
- `max_depth: Option<usize>`
- `max_calls: Option<usize>`

**Methods:**

- `fn new(host: H, max_depth: Option<usize>, max_calls: Option<usize>) -> Self` - Creates a new canonical eval callback.

**Trait Implementations:**

- **Eval**
  - `fn eval_expr(self: &Self, expr: &str, vars: &[(String, NonFunctionValue)]) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalEval<H>`



