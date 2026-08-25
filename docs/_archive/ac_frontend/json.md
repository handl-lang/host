**handl_frontend > json**

# Module: json

## Contents

**Structs**

- [`JsonFrontend`](#jsonfrontend)

---

## handl_frontend::json::JsonFrontend

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Frontend**
  - `fn parse(self: &Self, input: &str) -> Result<NonFunctionValue, <Self as >::Error>`
  - `fn serialize(self: &Self, value: &NonFunctionValue) -> Result<String, <Self as >::Error>`
  - `fn parse_with_locations(self: &Self, input: &str) -> Result<(NonFunctionValue, SourceMap), <Self as >::Error>`



