**handl_frontend > yaml**

# Module: yaml

## Contents

**Structs**

- [`YamlFrontend`](#yamlfrontend)

---

## handl_frontend::yaml::YamlFrontend

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Frontend**
  - `fn parse(self: &Self, input: &str) -> Result<NonFunctionValue, <Self as >::Error>`
  - `fn serialize(self: &Self, value: &NonFunctionValue) -> Result<String, <Self as >::Error>`
  - `fn parse_with_locations(self: &Self, input: &str) -> Result<(NonFunctionValue, SourceMap), <Self as >::Error>`



