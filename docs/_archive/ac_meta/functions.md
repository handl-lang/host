**handl_meta > functions**

# Module: functions

## Contents

**Structs**

- [`AstArgs`](#astargs) - `ast_args(ast) → array<ast>` — returns the argument list from a
- [`AstIsCall`](#astiscall) - `ast_is_call(ast) → boolean` — returns true if the AST node is a
- [`AstName`](#astname) - `ast_name(ast) → string|null` — returns the function name from a
- [`AstToString`](#asttostring) - `ast_to_string(ast) → string` — serializes a JSON AST back to an
- [`Eval`](#eval) - `eval(expr_string) → value` — evaluates an expression string at
- [`IsFunction`](#isfunction) - `is_function(value) → boolean` — returns true if the value is a
- [`Keys`](#keys) - `keys(object) → array<string>` — returns the key names of an object.
- [`Parse`](#parse) - `parse(expr_string) → ast` — parses an expression string into a
- [`TypeOf`](#typeof) - `typeof(value) → string` — returns the type name of a value.

**Functions**

- [`ast_to_string_inner`](#ast_to_string_inner) - Serialize a JSON AST back to an expression string.
- [`find_matching_close`](#find_matching_close) - Find the matching close delimiter for the open delimiter at `start`,
- [`parse_expr_to_json`](#parse_expr_to_json) - Parse an expression string into a JSON AST representation.

---

## handl_meta::functions::AstArgs

*Struct*

`ast_args(ast) → array<ast>` — returns the argument list from a
call AST node, or an empty array for non-call nodes.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> AstArgs`



## handl_meta::functions::AstIsCall

*Struct*

`ast_is_call(ast) → boolean` — returns true if the AST node is a
call node.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> AstIsCall`



## handl_meta::functions::AstName

*Struct*

`ast_name(ast) → string|null` — returns the function name from a
call AST node, or null for non-call nodes.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> AstName`



## handl_meta::functions::AstToString

*Struct*

`ast_to_string(ast) → string` — serializes a JSON AST back to an
expression string.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> AstToString`



## handl_meta::functions::Eval

*Struct*

`eval(expr_string) → value` — evaluates an expression string at
runtime using the current variable scope.

The expression string is parsed and evaluated with the current
context's variables bound as parameters. Requires an `Eval`
callback to be set on the context.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Eval`



## handl_meta::functions::IsFunction

*Struct*

`is_function(value) → boolean` — returns true if the value is a
function value (an object with a `body` string field).

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> IsFunction`



## handl_meta::functions::Keys

*Struct*

`keys(object) → array<string>` — returns the key names of an object.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Keys`



## handl_meta::functions::Parse

*Struct*

`parse(expr_string) → ast` — parses an expression string into a
JSON AST representation.

The AST is a JSON object with a `kind` field:
- `{"kind": "literal", "value": ..., "type": "string|number|boolean|null"}`
- `{"kind": "path", "path": "doc.name"}`
- `{"kind": "call", "name": "add", "args": [...]}`

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Parse`



## handl_meta::functions::TypeOf

*Struct*

`typeof(value) → string` — returns the type name of a value.

Returns one of: `"string"`, `"number"`, `"boolean"`, `"null"`,
`"array"`, `"object"`, `"function"`.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> TypeOf`



## handl_meta::functions::ast_to_string_inner

*Function*

Serialize a JSON AST back to an expression string.

```rust
fn ast_to_string_inner(ast: &serde_json::Value, dialect: &dyn Dialect) -> Option<String>
```



## handl_meta::functions::find_matching_close

*Function*

Find the matching close delimiter for the open delimiter at `start`,
respecting nested delimiters and string literals.

```rust
fn find_matching_close(s: &str, start: usize, open: char, close: char) -> Option<usize>
```



## handl_meta::functions::parse_expr_to_json

*Function*

Parse an expression string into a JSON AST representation.

```rust
fn parse_expr_to_json(s: &str, dialect: &dyn Dialect) -> Option<serde_json::Value>
```



