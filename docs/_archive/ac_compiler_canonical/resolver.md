**handl_compiler_canonical > resolver**

# Module: resolver

## Contents

**Structs**

- [`CompilerHost`](#compilerhost) - A host that combines the default handl primitives with compiler-specific
- [`DataFunction`](#datafunction) - A callable wrapper around a function value stored in `doc` or
- [`Import`](#import) - Resolves `import(path)` by looking up an external module.

**Functions**

- [`collect_data_functions`](#collect_data_functions)

---

## handl_compiler_canonical::resolver::CompilerHost

*Struct*

A host that combines the default handl primitives with compiler-specific
behavior: `import()` resolution and user-defined data functions.

**Fields:**
- `default: ac_runtime_example_1::DefaultHost`
- `import: Import`
- `data_functions: std::collections::HashMap<String, DataFunction>`

**Methods:**

- `fn new(default: DefaultHost, import: Import, namespaces: &handl_fields::Namespaces) -> Self` - Creates a new host from a default host, import resolver, base path,

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> CompilerHost`
- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_compiler_canonical::resolver::DataFunction

*Struct*

A callable wrapper around a function value stored in `doc` or
`doc.functions`.

**Fields:**
- `params: Vec<String>`
- `body: String`
- `return_schema: Option<serde_json::Value>`

**Methods:**

- `fn from(value: &FunctionValue) -> Self`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> DataFunction`



## handl_compiler_canonical::resolver::Import

*Struct*

Resolves `import(path)` by looking up an external module.

**Fields:**
- `base_path: Option<std::path::PathBuf>`
- `import_cache: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, serde_json::Value>>>`
- `resolver: std::rc::Rc<Box<dyn ModuleResolver>>`

**Methods:**

- `fn new(base_path: Option<PathBuf>, resolver: Rc<Box<dyn ModuleResolver>>, import_cache: Rc<RefCell<HashMap<String, JsonValue>>>) -> Self` - Creates a new `import` host function.
- `fn cache_key(self: &Self, path: &str) -> String` - Helper to build a cache key for an import path.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Import`



## handl_compiler_canonical::resolver::collect_data_functions

*Function*

```rust
fn collect_data_functions(prefix: &str, value: &serde_json::Value, out: & mut std::collections::HashMap<String, DataFunction>)
```



