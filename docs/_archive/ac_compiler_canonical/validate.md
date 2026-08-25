**handl_compiler_canonical > validate**

# Module: validate

## Contents

**Structs**

- [`ValidateStage`](#validatestage) - Validate phase: `ParsedDoc → ()`.

**Functions**

- [`run`](#run) - Validate an handl document through the parse → validate pipeline.

---

## handl_compiler_canonical::validate::ValidateStage

*Struct*

Validate phase: `ParsedDoc → ()`.

Validates the parsed document against a supplied JSON Schema using
the canonical handl dialect.

**Fields:**
- `schema: serde_json::Value`

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ParsedDoc, _session: & mut Session<Config>) -> Result<(), DriverError>`



## handl_compiler_canonical::validate::run

*Function*

Validate an handl document through the parse → validate pipeline.

```rust
fn run(input: &str, format: &str, schema_str: &str) -> Result<(), String>
```



