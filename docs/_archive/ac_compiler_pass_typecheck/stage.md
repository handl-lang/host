**handl_compiler_pass_typecheck > stage**

# Module: stage

## Contents

**Structs**

- [`TypecheckOnlyStage`](#typecheckonlystage) - Typecheck-only phase (no resolve): `ExtractedDoc → TypecheckResult`.
- [`TypecheckStage`](#typecheckstage) - Typecheck phase (after resolve): `ResolvedDoc → CheckedDoc`.

**Functions**

- [`parse_type_param`](#parse_type_param) - Parse a typeParams schema entry into a TypeParam.

---

## handl_compiler_pass_typecheck::stage::TypecheckOnlyStage

*Struct*

Typecheck-only phase (no resolve): `ExtractedDoc → TypecheckResult`.

Typechecks without resolving imports — for standalone type checking.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ExtractedDoc, session: & mut Session<Config>) -> Result<TypecheckResult, DriverError>`



## handl_compiler_pass_typecheck::stage::TypecheckStage

*Struct*

Typecheck phase (after resolve): `ResolvedDoc → CheckedDoc`.

Typechecks all handl keys and cross-module imports using rayon for parallelism.

**Unit Struct**

**Trait Implementations:**

- **Stage**
  - `fn name(self: &Self) -> &'static str`
  - `fn run(self: & mut Self, doc: ResolvedDoc, session: & mut Session<Config>) -> Result<CheckedDoc, DriverError>`



## handl_compiler_pass_typecheck::stage::parse_type_param

*Function*

Parse a typeParams schema entry into a TypeParam.

```rust
fn parse_type_param(v: &serde_json::Value) -> Option<handl_typecheck_core::TypeParam>
```



