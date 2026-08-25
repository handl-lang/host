**handl_lib_serialize**

# Module: handl_lib_serialize

## Contents

**Modules**

- [`error`](#error)
- [`traits`](#traits)

**Functions**

- [`deserialize`](#deserialize) - Deserialize input using the provided decoder.
- [`serialize`](#serialize) - Serialize a value using the provided encoder.

---

## handl_lib_serialize::deserialize

*Function*

Deserialize input using the provided decoder.

```rust
fn deserialize<I, T, D>(input: &I, decoder: &D) -> Result<T, SerializeError>
```



## Module: error



## handl_lib_serialize::serialize

*Function*

Serialize a value using the provided encoder.

```rust
fn serialize<T, E>(value: &T, encoder: &E) -> Result<<E as >::Output, SerializeError>
```



## Module: traits



