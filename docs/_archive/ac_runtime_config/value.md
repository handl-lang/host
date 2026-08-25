**ac_runtime_config > value**

# Module: value

## Contents

**Structs**

- [`FunctionValue`](#functionvalue) - A serializable function value — pure, no closures.
- [`NonFunctionValue`](#nonfunctionvalue) - A runtime value in the language.

**Type Aliases**

- [`HostValue`](#hostvalue) - Alias used at the VM/host boundary.

---

## ac_runtime_config::value::FunctionValue

*Struct*

A serializable function value — pure, no closures.

A first-class function is just its parameter schemas, optional
return schema, and a body expression string. Since the body is
a pure expression (no captured state), the whole value is plain
data that serializes as JSON.

A function value is detected by the presence of a `body` string
field in a JSON object. No special tag is needed.

**Fields:**
- `params: Vec<(String, serde_json::Value)>` - `(param_name, raw_schema)` pairs, in declaration order.
- `return_schema: Option<serde_json::Value>` - Raw schema value for the return type, if declared.
- `body: String` - The body expression string (e.g. `"mul(x, 2)"`).
- `patterns: Vec<serde_json::Value>` - Original pattern JSON for each parameter, in declaration order.

**Methods:**

- `fn to_json(self: &Self) -> JsonValue` - Encodes this function value as a plain JSON object.
- `fn from_json(value: &JsonValue) -> Option<Self>` - Decodes a function value from a JSON object.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &FunctionValue) -> bool`
- **FunctionRepr**
  - `fn params(self: &Self) -> Vec<(String, JsonValue)>`
  - `fn return_schema(self: &Self) -> Option<JsonValue>`
  - `fn body(self: &Self) -> &str`
- **Clone**
  - `fn clone(self: &Self) -> FunctionValue`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **FunctionRepr**
  - `fn params(self: &Self) -> Vec<(String, NonFunctionValue)>`
  - `fn return_schema(self: &Self) -> Option<NonFunctionValue>`
  - `fn body(self: &Self) -> &str`



## ac_runtime_config::value::HostValue

*Type Alias*: `NonFunctionValue`

Alias used at the VM/host boundary.



## ac_runtime_config::value::NonFunctionValue

*Struct*

A runtime value in the language.

A transparent newtype around `serde_json::Value`. Deliberately
boring — the kernel does not define its own type system.

`HostValue` is a type alias for `NonFunctionValue`, used at the
VM/host boundary.

**Tuple Struct**: `(serde_json::Value)`

**Methods:**

- `fn new<impl Into<JsonValue>>(value: impl Trait) -> Self` - Creates a new runtime value from anything convertible to
- `fn inner(self: &Self) -> &JsonValue` - Returns a reference to the underlying `serde_json::Value`.
- `fn inner_mut(self: & mut Self) -> & mut JsonValue` - Returns a mutable reference to the underlying `serde_json::Value`.
- `fn into_inner(self: Self) -> JsonValue` - Consumes self and returns the underlying `serde_json::Value`.
- `fn is_null(self: &Self) -> bool`
- `fn is_bool(self: &Self) -> bool`
- `fn is_number(self: &Self) -> bool`
- `fn is_string(self: &Self) -> bool`
- `fn is_array(self: &Self) -> bool`
- `fn is_object(self: &Self) -> bool`
- `fn as_bool(self: &Self) -> Option<bool>`
- `fn as_number(self: &Self) -> Option<f64>`
- `fn as_str(self: &Self) -> Option<&str>`
- `fn as_array(self: &Self) -> Option<&[JsonValue]>`
- `fn as_object(self: &Self) -> Option<&serde_json::Map<String, JsonValue>>`
- `fn as_object_mut(self: & mut Self) -> Option<& mut serde_json::Map<String, JsonValue>>`
- `fn get(self: &Self, key: &str) -> Option<&JsonValue>`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> NonFunctionValue`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> NonFunctionValue`
- **PartialEq**
  - `fn eq(self: &Self, other: &NonFunctionValue) -> bool`



