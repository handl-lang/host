**handl_compiler_canonical > eval**

# Module: eval

## Contents

**Structs**

- [`EvalStage`](#evalstage) - Eval phase: `ParsedDoc → NonFunctionValue`.

**Enums**

- [`VmMode`](#vmmode) - Which VM host to use for evaluation.

**Functions**

- [`run`](#run) - Evaluate an handl document through the parse → VM pipeline.

---

## handl_compiler_canonical::eval::EvalStage

*Struct*

Eval phase: `ParsedDoc → NonFunctionValue`.

Runs the canonical handl VM over the parsed document, treating the
document root as the expression to evaluate.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ParsedDoc, session: & mut Session<Config>) -> Result<NonFunctionValue, DriverError>`



## handl_compiler_canonical::eval::VmMode

*Enum*

Which VM host to use for evaluation.

**Variants:**
- `Sandbox` - Pure, sandboxed evaluation (default).
- `Server` - Effects-enabled evaluation (files, HTTP, OS, ...).

**Traits:** Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> VmMode`



## handl_compiler_canonical::eval::run

*Function*

Evaluate an handl document through the parse → VM pipeline.

```rust
fn run(input: &str, format: &str, output_format: &str, vm: &str) -> Result<String, String>
```



