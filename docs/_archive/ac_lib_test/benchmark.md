**handl_lib_test > benchmark**

# Module: benchmark

## Contents

**Structs**

- [`BenchmarkModule`](#benchmarkmodule) - The `benchmark` module — simple wall-clock timing for functions.
- [`Run`](#run)

---

## handl_lib_test::benchmark::BenchmarkModule

*Struct*

The `benchmark` module — simple wall-clock timing for functions.

Functions: `benchmark.run`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> BenchmarkModule`



## handl_lib_test::benchmark::Run

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Run`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



