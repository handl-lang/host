**handl_compiler_pass_generate > stage**

# Module: stage

## Contents

**Structs**

- [`GenerateFromResolvedStage`](#generatefromresolvedstage) - Generate phase (skip typecheck): `ResolvedDoc → JsonValue`.
- [`GenerateStage`](#generatestage) - Generate phase: `CheckedDoc → JsonValue`.

---

## handl_compiler_pass_generate::stage::GenerateFromResolvedStage

*Struct*

Generate phase (skip typecheck): `ResolvedDoc → JsonValue`.

For the typegen-only pipeline where typecheck is not needed.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ResolvedDoc, session: & mut Session<Config>) -> Result<JsonValue, DriverError>`



## handl_compiler_pass_generate::stage::GenerateStage

*Struct*

Generate phase: `CheckedDoc → JsonValue`.

Generates output from the resolved and typechecked artifacts.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: CheckedDoc, session: & mut Session<Config>) -> Result<JsonValue, DriverError>`



