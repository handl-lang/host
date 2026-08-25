**handl_compiler > driver**

# Module: driver

## Contents

**Structs**

- [`CheckedDoc`](#checkeddoc) - Output of the typecheck phase (after resolve) — all artifacts plus type errors.
- [`ExtractedDoc`](#extracteddoc) - Output of the extract phase — extracted fields plus carried source map.
- [`ParsedDoc`](#parseddoc) - Output of the parse phase — a parsed document value plus source map.
- [`ResolvedDoc`](#resolveddoc) - Output of the resolve phase — extracted fields, typegen context, source map.
- [`TypecheckResult`](#typecheckresult) - Output of the typecheck-only phase (no resolve) — extracted fields + errors.

---

## handl_compiler::driver::CheckedDoc

*Struct*

Output of the typecheck phase (after resolve) — all artifacts plus type errors.

**Fields:**
- `extracted: handl_fields::ExtractedFields`
- `typegen_ctx: handl_typegen_core::TypegenContext`
- `type_errors: Vec<handl_typecheck_core::TypeError>`
- `source_map: handl_source::SourceMap`



## handl_compiler::driver::ExtractedDoc

*Struct*

Output of the extract phase — extracted fields plus carried source map.

**Fields:**
- `extracted: handl_fields::ExtractedFields`
- `source_map: handl_source::SourceMap`
- `base_path: Option<std::path::PathBuf>` - Base path for relative import resolution.



## handl_compiler::driver::ParsedDoc

*Struct*

Output of the parse phase — a parsed document value plus source map.

**Fields:**
- `value: ac_runtime_values::NonFunctionValue`
- `source_map: handl_source::SourceMap`
- `base_path: Option<std::path::PathBuf>` - Base path for relative import resolution (parent dir of source file).
- `format: String` - Source format used to parse the document (e.g. `yaml`, `json`, `toml`, `handl`).



## handl_compiler::driver::ResolvedDoc

*Struct*

Output of the resolve phase — extracted fields, typegen context, source map.

**Fields:**
- `extracted: handl_fields::ExtractedFields`
- `typegen_ctx: handl_typegen_core::TypegenContext`
- `source_map: handl_source::SourceMap`
- `base_path: Option<std::path::PathBuf>` - Base path — carried through for typecheck import resolution.



## handl_compiler::driver::TypecheckResult

*Struct*

Output of the typecheck-only phase (no resolve) — extracted fields + errors.

**Fields:**
- `extracted: handl_fields::ExtractedFields`
- `type_errors: Vec<handl_typecheck_core::TypeError>`
- `source_map: handl_source::SourceMap`



