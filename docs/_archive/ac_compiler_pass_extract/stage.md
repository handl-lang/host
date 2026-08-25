**handl_compiler_pass_extract > stage**

# Module: stage

## Contents

**Structs**

- [`ExtractStage`](#extractstage) - Extract phase: `ParsedDoc → ExtractedDoc`.

---

## handl_compiler_pass_extract::stage::ExtractStage

*Struct*

Extract phase: `ParsedDoc → ExtractedDoc`.

Extracts fields, functions, and data from the parsed document.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, parsed: ParsedDoc, session: & mut Session<Config>) -> Result<ExtractedDoc, DriverError>`



