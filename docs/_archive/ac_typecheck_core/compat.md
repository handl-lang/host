**handl_typecheck_core > compat**

# Module: compat

## Contents

**Functions**

- [`is_compatible`](#is_compatible) - Check if `actual` is compatible with `expected`.
- [`variance_compatible`](#variance_compatible) - Check if `actual` is compatible with `expected` under a variance annotation.

---

## handl_typecheck_core::compat::is_compatible

*Function*

Check if `actual` is compatible with `expected`.

Rules:
- `Unknown` is compatible with anything (and vice versa).
- `Never` is compatible with anything (it's uninhabited).
- `Var` is compatible with `Unknown`/`Never` (graceful degradation)
  and with an identical `Var` (same name). Outside unification,
  `Var` is **not** compatible with concrete types.
- Exact matches are compatible.
- `Number` accepts `Number` (no integer subtyping yet).
- `Array<T>` is compatible if item types are compatible.
- `Object` is compatible if every expected property exists with a
  compatible type. Extra properties in `actual` are allowed.
- `Function` is compatible if param types are contravariantly compatible
  and return types are covariantly compatible.

```rust
fn is_compatible(actual: &crate::Type, expected: &crate::Type) -> bool
```



## handl_typecheck_core::compat::variance_compatible

*Function*

Check if `actual` is compatible with `expected` under a variance annotation.

- [`Variance::Covariant`]: `is_compatible(actual, expected)` — actual
  can be a subtype of expected.
- [`Variance::Contravariant`]: `is_compatible(expected, actual)` — flip
  the direction (used for function parameter positions).
- [`Variance::Invariant`]: `actual == expected` (exact match required).
- [`Variance::Bivariant`]: always compatible.

`Unknown` and `Never` are always compatible regardless of variance.

```rust
fn variance_compatible(actual: &crate::Type, expected: &crate::Type, variance: crate::Variance) -> bool
```



