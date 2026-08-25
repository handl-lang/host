**handl_typecheck_infer > table**

# Module: table

## Contents

**Structs**

- [`FunctionTable`](#functiontable) - A lookup table for function types — maps function names to their
- [`HostFunctionSigs`](#hostfunctionsigs) - Host function signatures for typechecking.

**Type Aliases**

- [`RawFnSig`](#rawfnsig) - A raw function signature: `(name, type_params, params, return_schema)`.

---

## handl_typecheck_infer::table::FunctionTable

*Struct*

A lookup table for function types — maps function names to their
[`FunctionType`] (param types + return type).

Built from raw function signatures using a [`SchemaInterpreter`]
to convert raw schema values into [`Type`]s.

**Fields:**
- `functions: std::collections::BTreeMap<String, handl_typecheck_core::FunctionType>`

**Methods:**

- `fn new() -> Self` - Create an empty table.
- `fn from_signatures(sigs: &[RawFnSig], interpreter: &dyn SchemaInterpreter) -> Self` - Build a table from raw function signatures.
- `fn insert<impl Into<String>>(self: & mut Self, name: impl Trait, fn_type: FunctionType)` - Insert a function type directly.
- `fn lookup(self: &Self, name: &str) -> Option<&FunctionType>` - Look up a function's type by name.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



## handl_typecheck_infer::table::HostFunctionSigs

*Struct*

Host function signatures for typechecking.

Passed to the typechecker via `Config::with_extension`. The runner
merges these with document-declared function signatures to build
the full `FunctionTable`.

# Example
```ignore
use handl_compiler::{Config, typecheck_only_config};
use handl_typecheck_infer::HostFunctionSigs;

let config = typecheck_only_config().with_extension(HostFunctionSigs::default_host());
```

**Tuple Struct**: `(Vec<RawFnSig>)`

**Methods:**

- `fn default_host() -> Self` - Default host function signatures for the reference runtime.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> HostFunctionSigs`



## handl_typecheck_infer::table::RawFnSig

*Type Alias*: `(String, Vec<handl_typecheck_core::TypeParam>, Vec<(String, serde_json::Value)>, Option<serde_json::Value>)`

A raw function signature: `(name, type_params, params, return_schema)`.

`type_params` is a list of type parameter declarations (e.g. `[TypeParam::new("T")]`).
`params` is a list of `(param_name, raw_schema)` pairs.
`return_schema` is the raw return type schema, if declared.



