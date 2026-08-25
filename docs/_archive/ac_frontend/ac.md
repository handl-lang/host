**handl_frontend > handl**

# Module: handl

## Contents

**Structs**

- [`HandlFrontend`](#acfrontend) - Frontend for the canonical handl expression text format.

---

## handl_frontend::handl::HandlFrontend

*Struct*

Frontend for the canonical handl expression text format.

**Unit Struct**

**Trait Implementations:**

- **Frontend**
  - `fn parse(self: &Self, input: &str) -> Result<NonFunctionValue, <Self as >::Error>`
  - `fn serialize(self: &Self, value: &NonFunctionValue) -> Result<String, <Self as >::Error>`
  - `fn parse_with_locations(self: &Self, input: &str) -> Result<(NonFunctionValue, SourceMap), <Self as >::Error>`



