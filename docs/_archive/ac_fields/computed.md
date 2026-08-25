**handl_fields > computed**

# Module: computed

## Contents

**Functions**

- [`check_arg_type`](#check_arg_type) - Minimal type check for arg validation.
- [`compute_fields`](#compute_fields) - Evaluates computed field expressions and inserts results into `data`.

---

## handl_fields::computed::check_arg_type

*Function*

Minimal type check for arg validation.

Checks the `type` field of a raw schema value against the actual value type.
This is a minimal check — full schema validation should use a `Schema<V>` implementation.

```rust
fn check_arg_type(value: &serde_json::Value, schema: &serde_json::Value) -> Result<(), String>
```



## handl_fields::computed::compute_fields

*Function*

Evaluates computed field expressions and inserts results into `data`.

Fields declared with an `expr:` key are computed after interpolation.
The expression uses the same syntax as `${...}` content: function calls,
path lookups (via namespaces), and literals. Computed fields can reference
other fields via the default namespace and can depend on other computed
fields — multiple passes resolve dependencies in order.

The default namespace in `namespaces` is updated with `data` before each
pass so expressions see current field values.

Returns a list of error messages for any computed fields that could not
be evaluated after all passes.

```rust
fn compute_fields<H>(data: & mut serde_json::Value, computed: &[crate::types::ComputedField], namespaces: & mut crate::types::Namespaces, host: &H, dialect: &'static dyn Dialect) -> Vec<String>
```



