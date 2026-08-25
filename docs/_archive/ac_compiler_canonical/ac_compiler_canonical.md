**handl_compiler_canonical**

# Module: handl_compiler_canonical

## Contents

**Modules**

- [`eval`](#eval)
- [`fields`](#fields)
- [`parse`](#parse)
- [`resolver`](#resolver) - VM-backed compiler host for field interpolation and computation.
- [`typecheck`](#typecheck) - Static type checking for handl documents.
- [`typegen`](#typegen)
- [`validate`](#validate)

**Functions**

- [`default_cli_config`](#default_cli_config) - Default configuration for CLI usage:
- [`full_pipeline`](#full_pipeline) - Full pipeline: parse → extract → resolve → typecheck → generate.
- [`typecheck_only_config`](#typecheck_only_config) - Typecheck-only config — no backend needed.
- [`typecheck_pipeline`](#typecheck_pipeline) - Typecheck pipeline: parse → extract → typecheck.
- [`typegen_pipeline`](#typegen_pipeline) - Full typegen pipeline: parse → extract → resolve → generate.

---

## handl_compiler_canonical::default_cli_config

*Function*

Default configuration for CLI usage:
- `FileResolver` with `MemoryModuleCache` for filesystem imports
- `JsonSchemaInterpreter` for type inference
- `JsonSchemaBackend` for JSON Schema generation

```rust
fn default_cli_config() -> Config
```



## Module: eval



## Module: fields



## handl_compiler_canonical::full_pipeline

*Function*

Full pipeline: parse → extract → resolve → typecheck → generate.

```rust
fn full_pipeline() -> DriverPipeline<Config, Then<Then<Then<Then<ParseStage, ExtractStage>, ResolveStage>, TypecheckStage>, GenerateStage>>
```



## Module: parse



## Module: resolver

VM-backed compiler host for field interpolation and computation.

`CompilerHost` wraps [`ac_runtime_example_1::DefaultHost`] and adds `import()`
resolution plus user-defined functions stored in the default namespace
(`doc.<name>` or `doc.functions.<name>`).



## Module: typecheck

Static type checking for handl documents.

Delegates to `typecheck_pipeline` — parse → extract → typecheck.



## handl_compiler_canonical::typecheck_only_config

*Function*

Typecheck-only config — no backend needed.

```rust
fn typecheck_only_config() -> Config
```



## handl_compiler_canonical::typecheck_pipeline

*Function*

Typecheck pipeline: parse → extract → typecheck.

```rust
fn typecheck_pipeline() -> DriverPipeline<Config, Then<Then<ParseStage, ExtractStage>, TypecheckOnlyStage>>
```



## Module: typegen



## handl_compiler_canonical::typegen_pipeline

*Function*

Full typegen pipeline: parse → extract → resolve → generate.

```rust
fn typegen_pipeline() -> DriverPipeline<Config, Then<Then<Then<ParseStage, ExtractStage>, ResolveStage>, GenerateFromResolvedStage>>
```



## Module: validate



