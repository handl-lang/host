**handl_compiler_pass_parse > stage**

# Module: stage

## Contents

**Structs**

- [`ParseStage`](#parsestage) - Parse phase: `Source → ParsedDoc`.

---

## handl_compiler_pass_parse::stage::ParseStage

*Struct*

Parse phase: `Source → ParsedDoc`.

Parses source text into a `NonFunctionValue` using the format-specified
frontend. Builds a source map for later error location resolution.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, source: Source, _session: & mut Session<Config>) -> Result<ParsedDoc, DriverError>`



