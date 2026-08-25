**handl_compiler_pass_resolve > stage**

# Module: stage

## Contents

**Structs**

- [`ResolveStage`](#resolvestage) - Resolve phase: `ExtractedDoc → ResolvedDoc`.

---

## handl_compiler_pass_resolve::stage::ResolveStage

*Struct*

Resolve phase: `ExtractedDoc → ResolvedDoc`.

1. Builds a `TypegenContext` from extracted schemas.
2. Filters merge fields via the configured `KeyHandler`.
3. Strips all special keys via the configured `KeyHandler`.
4. Resolves imports via the configured `ImportResolver`.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ExtractedDoc, session: & mut Session<Config>) -> Result<ResolvedDoc, DriverError>`



