**ac_runtime > function_repr**

# Module: function_repr

## Contents

**Traits**

- [`FunctionRepr`](#functionrepr) - A pluggable representation of a first-class function value.

---

## ac_runtime::function_repr::FunctionRepr

*Trait*

A pluggable representation of a first-class function value.

This trait is separate from [`Function`](crate::Function) (the host
function trait). It describes how to read the metadata of a
function value that is encoded as an ordinary runtime value —
parameters, optional return schema, and body expression.

Implementations live next to the concrete value type. For the
default JSON-backed `NonFunctionValue`/`FunctionValue` types, see
`handl-runtime-config`.

**Methods:**

- `params`: The declared parameters as `(name, schema)` pairs.
- `return_schema`: The declared return-type schema, if any.
- `body`: The raw body expression string (e.g. `"add(x, 1)"`).



