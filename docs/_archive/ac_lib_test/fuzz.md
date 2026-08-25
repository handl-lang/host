**handl_lib_test > fuzz**

# Module: fuzz

## Contents

**Structs**

- [`FuzzModule`](#fuzzmodule) - The `fuzz` module — structure-agnostic random-input stress testing.
- [`Run`](#run)

---

## handl_lib_test::fuzz::FuzzModule

*Struct*

The `fuzz` module — structure-agnostic random-input stress testing.

Functions: `fuzz.run`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> FuzzModule`



## handl_lib_test::fuzz::Run

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Run`



