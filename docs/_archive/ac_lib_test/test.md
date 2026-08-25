**handl_lib_test > test**

# Module: test

## Contents

**Structs**

- [`Run`](#run)
- [`TestModule`](#testmodule) - The `test` module — test suite execution and reporting.

---

## handl_lib_test::test::Run

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Run`



## handl_lib_test::test::TestModule

*Struct*

The `test` module — test suite execution and reporting.

Functions: `test.run`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> TestModule`



