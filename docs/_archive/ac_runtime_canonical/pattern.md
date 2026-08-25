**ac_runtime_canonical > pattern**

# Module: pattern

## Contents

**Functions**

- [`define_pattern`](#define_pattern) - Defines each name bound by `pattern` to the matching part of
- [`define_pattern_inner`](#define_pattern_inner)
- [`pattern_from_json`](#pattern_from_json) - Deserializes a [`Pattern`] from JSON.
- [`pattern_to_json`](#pattern_to_json) - Serializes a [`Pattern`] to a JSON value.

---

## ac_runtime_canonical::pattern::define_pattern

*Function*

Defines each name bound by `pattern` to the matching part of
`value`, then returns the original value.

```rust
fn define_pattern(ctx: & mut dyn Context, pattern: &ac_runtime::Pattern, value: ac_runtime_values::NonFunctionValue) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime::Error<ac_runtime_values::NonFunctionValue>>
```



## ac_runtime_canonical::pattern::define_pattern_inner

*Function*

```rust
fn define_pattern_inner(ctx: & mut dyn Context, pattern: &ac_runtime::Pattern, value: &serde_json::Value) -> Result<(), ac_runtime::Error<ac_runtime_values::NonFunctionValue>>
```



## ac_runtime_canonical::pattern::pattern_from_json

*Function*

Deserializes a [`Pattern`] from JSON.

```rust
fn pattern_from_json(value: &serde_json::Value) -> Option<ac_runtime::Pattern>
```



## ac_runtime_canonical::pattern::pattern_to_json

*Function*

Serializes a [`Pattern`] to a JSON value.

The JSON format is:
- `{"type":"identifier","name":"x"}`
- `{"type":"array","patterns":[...]}`
- `{"type":"object","entries":[["key", pattern], ...]}`
- `{"type":"rest","name":"rest"}`

```rust
fn pattern_to_json(pattern: &ac_runtime::Pattern) -> serde_json::Value
```



