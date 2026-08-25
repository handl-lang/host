**handl_compiler_canonical > typecheck**

# Module: typecheck

## Contents

**Structs**

- [`TypecheckReport`](#typecheckreport) - Report from a typecheck run.

**Functions**

- [`run`](#run)

---

## handl_compiler_canonical::typecheck::TypecheckReport

*Struct*

Report from a typecheck run.

**Fields:**
- `errors: Vec<handl_typecheck_core::TypeError>`

**Methods:**

- `fn is_valid(self: &Self) -> bool`



## handl_compiler_canonical::typecheck::run

*Function*

```rust
fn run(input: &str, format: &str, base_path: Option<&std::path::Path>) -> Result<TypecheckReport, crate::DriverError>
```



