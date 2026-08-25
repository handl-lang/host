**handl_compiler_canonical > fields**

# Module: fields

## Contents

**Structs**

- [`FieldsReport`](#fieldsreport) - Report from a fields validation run.
- [`FieldsStage`](#fieldsstage) - Fields phase: `ExtractedDoc → FieldsReport`.

**Functions**

- [`run`](#run) - Validate `$:` fields through the parse → extract → fields pipeline.

---

## handl_compiler_canonical::fields::FieldsReport

*Struct*

Report from a fields validation run.

**Fields:**
- `field_types: Vec<(String, String)>` - `(field_name, type_name)` pairs for declared fields (excluding merge fields).
- `ok: Vec<String>` - Field names that validated successfully.
- `errors: Vec<String>` - Validation error messages.
- `undeclared: Vec<String>` - Undeclared field names found in the data.

**Methods:**

- `fn error_count(self: &Self) -> usize`
- `fn is_valid(self: &Self) -> bool`



## handl_compiler_canonical::fields::FieldsStage

*Struct*

Fields phase: `ExtractedDoc → FieldsReport`.

Interpolates, computes, merges, and validates the `$:` field
declarations against the document data.

**Fields:**
- `base_path: Option<std::path::PathBuf>`

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ExtractedDoc, _session: & mut Session<Config>) -> Result<FieldsReport, DriverError>`



## handl_compiler_canonical::fields::run

*Function*

Validate `$:` fields through the parse → extract → fields pipeline.

```rust
fn run(input: &str, format: &str, base_path: Option<&std::path::Path>) -> Result<FieldsReport, String>
```



