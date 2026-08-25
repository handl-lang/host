# ac_runtime_config

# handl-runtime-config

The configuration-language value representation for handl.

`NonFunctionValue` is a transparent newtype around `serde_json::Value`,
and `FunctionValue` is the JSON-backed, closure-free first-class function
encoding. These are the concrete value types for YAML/JSON/TOML-style
configuration documents. They are not the universal default for all
handl runtimes — the core VM is generic over the value type.

## Modules

### [`ac_runtime_config`](ac_runtime_config.md)

*1 type alias, 2 modules*

### [`canonical_context`](canonical_context.md)

*1 struct*

### [`canonical_context::CanonicalContext`](canonical_context/CanonicalContext.md)

*1 function*

### [`value`](value.md)

*1 type alias, 2 structs*

