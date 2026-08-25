**handl_fields > types**

# Module: types

## Contents

**Structs**

- [`ComputedField`](#computedfield) - A computed field declaration extracted from `$:`.
- [`ExtractedFields`](#extractedfields) - Result of [`extract_fields`]: field schemas and data.

**Enums**

- [`ArgSpec`](#argspec) - Declares how a computed field's `arg` is typed.

**Constants**

- [`APPLY_FUNCTION`](#apply_function) - The conventional name of the host function that applies a first-class
- [`DEFAULT_NAMESPACE`](#default_namespace) - The conventional name of the default namespace used for shorthand

**Type Aliases**

- [`Namespaces`](#namespaces) - A collection of named namespaces for `${ns.key}` interpolation.

---

## handl_fields::types::APPLY_FUNCTION

*Constant*: `&str`

The conventional name of the host function that applies a first-class
function value to arguments.

This is a runtime host convention, not a language-level requirement.
The language defines no built-in functions — `apply` is simply the name
the reference runtime uses for the function that evaluates a function
value's body with bound arguments. A runtime host may use a different
name or provide function application through a different mechanism.



## handl_fields::types::ArgSpec

*Enum*

Declares how a computed field's `arg` is typed.

**Variants:**
- `Untyped` - The arg is available without type validation.
- `Typed(serde_json::Value)` - The arg is validated against this raw schema value before the expr runs.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ArgSpec`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_fields::types::ComputedField

*Struct*

A computed field declaration extracted from `$:`.

Fields with an `expr:` key are computed after interpolation.
The `arg` field, if present, makes the field's own data value
available as `${arg}` inside the expression.

**Fields:**
- `name: String` - The field name.
- `expr: String` - The expression string to evaluate.
- `arg: Option<ArgSpec>` - If present, the field's data value is available as `${arg}`.

**Trait Implementations:**

- **From**
  - `fn from((name, expr): (String, String)) -> Self`
- **Clone**
  - `fn clone(self: &Self) -> ComputedField`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_fields::types::DEFAULT_NAMESPACE

*Constant*: `&str`

The conventional name of the default namespace used for shorthand
path resolution (bare `name` → `DEFAULT_NAMESPACE.name`).

This is a runtime host convention, not a language-level requirement.
A runtime host may use a different name by inserting it into the
namespaces map under a different key and adjusting shorthand resolution
accordingly.



## handl_fields::types::ExtractedFields

*Struct*

Result of [`extract_fields`]: field schemas and data.

**Fields:**
- `schemas: Vec<(String, serde_json::Value)>` - `(name, raw_schema_value)` pairs for each declared field.
- `data: ac_runtime_values::NonFunctionValue` - The document data with `$:` removed.
- `inheritable: Vec<String>` - Field names declared with `inheritable: true`.
- `immutable: Vec<String>` - Field names declared with `mutable: false`.
- `computed: Vec<ComputedField>` - `ComputedField` entries for fields with an `expr:` key.
- `merge: Vec<String>` - Field names declared with `merge: true`.



## handl_fields::types::Namespaces

*Type Alias*: `std::collections::HashMap<String, serde_json::Value>`

A collection of named namespaces for `${ns.key}` interpolation.

The default namespace (conventionally `doc`) is typically built from
the document's own data fields. `env` is typically built from
environment variables. Custom namespaces can be added by the caller.
Which namespaces exist is a runtime host decision.



