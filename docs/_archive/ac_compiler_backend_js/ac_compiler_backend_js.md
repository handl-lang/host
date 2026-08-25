**handl_compiler_backend_js**

# Module: handl_compiler_backend_js

## Contents

**Enums**

- [`Error`](#error)

**Functions**

- [`compile`](#compile)
- [`compile_expr`](#compile_expr)
- [`compile_expr_body`](#compile_expr_body)
- [`compile_handler`](#compile_handler)
- [`compile_module`](#compile_module)
- [`js_string_literal`](#js_string_literal)
- [`json_literal`](#json_literal)
- [`params_literal`](#params_literal)
- [`pattern_json`](#pattern_json)
- [`pattern_literal`](#pattern_literal)

---

## handl_compiler_backend_js::Error

*Enum*

**Variants:**
- `Parse(String)`
- `Unsupported(String)`

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_compiler_backend_js::compile

*Function*

```rust
fn compile(expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>) -> Result<String, Error>
```



## handl_compiler_backend_js::compile_expr

*Function*

```rust
fn compile_expr(expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>, out: & mut String) -> Result<(), Error>
```



## handl_compiler_backend_js::compile_expr_body

*Function*

```rust
fn compile_expr_body(expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>, out: & mut String) -> Result<(), Error>
```



## handl_compiler_backend_js::compile_handler

*Function*

```rust
fn compile_handler(clause: &handl_ast_canonical::HandlerClause<ac_runtime_values::NonFunctionValue>, out: & mut String) -> Result<(), Error>
```



## handl_compiler_backend_js::compile_module

*Function*

```rust
fn compile_module(expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>) -> Result<String, Error>
```



## handl_compiler_backend_js::js_string_literal

*Function*

```rust
fn js_string_literal(value: &str) -> String
```



## handl_compiler_backend_js::json_literal

*Function*

```rust
fn json_literal(value: &serde_json::Value) -> String
```



## handl_compiler_backend_js::params_literal

*Function*

```rust
fn params_literal(params: &[(handl_ast_canonical::Pattern, Option<Box<handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>>>)]) -> Result<String, Error>
```



## handl_compiler_backend_js::pattern_json

*Function*

```rust
fn pattern_json(pattern: &handl_ast_canonical::Pattern) -> serde_json::Value
```



## handl_compiler_backend_js::pattern_literal

*Function*

```rust
fn pattern_literal(pattern: &handl_ast_canonical::Pattern) -> String
```



