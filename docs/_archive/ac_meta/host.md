**handl_meta > host**

# Module: host

## Contents

**Structs**

- [`MetaHost`](#metahost) - A host that provides all [`DefaultHost`] functions plus meta

---

## handl_meta::host::MetaHost

*Struct*

A host that provides all [`DefaultHost`] functions plus meta
functions for reflection, dynamic evaluation, and AST manipulation.

Meta functions:
- `typeof(value)` → type name string
- `keys(object)` → array of key names
- `is_function(value)` → boolean
- `eval(expr_string)` → evaluated value
- `parse(expr_string)` → JSON AST
- `ast_to_string(ast)` → expression string
- `ast_name(ast)` → function name or null
- `ast_args(ast)` → array of arg ASTs
- `ast_is_call(ast)` → boolean

**Fields:**
- `default: ac_runtime_example_1::DefaultHost`
- `type_of: crate::functions::TypeOf`
- `keys: crate::functions::Keys`
- `is_function: crate::functions::IsFunction`
- `eval: crate::functions::Eval`
- `parse: crate::functions::Parse`
- `ast_to_string: crate::functions::AstToString`
- `ast_name: crate::functions::AstName`
- `ast_args: crate::functions::AstArgs`
- `ast_is_call: crate::functions::AstIsCall`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MetaHost`
- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Default**
  - `fn default() -> Self`



