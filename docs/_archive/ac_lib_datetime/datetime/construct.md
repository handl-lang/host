**handl_lib_datetime > datetime > construct**

# Module: datetime::construct

## Contents

**Structs**

- [`FromComponents`](#fromcomponents)

**Functions**

- [`get_number`](#get_number)

---

## handl_lib_datetime::datetime::construct::FromComponents

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> FromComponents`



## handl_lib_datetime::datetime::construct::get_number

*Function*

```rust
fn get_number(args: &[ac_runtime_values::NonFunctionValue], index: usize, default: f64) -> Result<f64, ac_runtime_values::Error>
```



