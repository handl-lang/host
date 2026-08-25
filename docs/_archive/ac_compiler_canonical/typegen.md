**handl_compiler_canonical > typegen**

# Module: typegen

## Contents

**Structs**

- [`TypegenInput`](#typegeninput) - Input parameters for a typegen run.

**Enums**

- [`TypegenResult`](#typegenresult) - Result of a typegen run.

**Functions**

- [`run`](#run)

---

## handl_compiler_canonical::typegen::TypegenInput

*Struct*

Input parameters for a typegen run.

**Generic Parameters:**
- 'a

**Fields:**
- `input: &'a str`
- `format: &'a str`
- `input_path: &'a std::path::Path`
- `output_format: &'a str`
- `output_dir: &'a std::path::Path`
- `stdout: bool`



## handl_compiler_canonical::typegen::TypegenResult

*Enum*

Result of a typegen run.

**Variants:**
- `Stdout(String)` - Schema was printed to stdout (the serialized schema).
- `FileWritten(std::path::PathBuf)` - Schema was written to a file (the file path).



## handl_compiler_canonical::typegen::run

*Function*

```rust
fn run(args: &TypegenInput) -> Result<TypegenResult, crate::DriverError>
```



