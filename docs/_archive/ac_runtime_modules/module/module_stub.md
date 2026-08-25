**ac_runtime_modules > module > module_stub**

# Module: module::module_stub

## Contents

**Structs**

- [`AsyncEcho`](#asyncecho)
- [`Echo`](#echo)
- [`Raise`](#raise)
- [`StubModule`](#stubmodule) - A minimal module implementation for testing.

**Statics**

- [`STUB_MODULE`](#stub_module) - A static [`StubModule`] instance for convenience.

---

## ac_runtime_modules::module::module_stub::AsyncEcho

*Struct*

**Unit Struct**

**Trait Implementations:**

- **AsyncFunction**
  - `fn call<'a>(self: &'a Self, args: &'a [NonFunctionValue], _ctx: &'a  mut dyn Context) -> Pin<Box<dyn std::future::Future>>`
- **Clone**
  - `fn clone(self: &Self) -> AsyncEcho`



## ac_runtime_modules::module::module_stub::Echo

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Echo`



## ac_runtime_modules::module::module_stub::Raise

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Raise`
- **Operation**
  - `fn signature(self: &Self) -> OperationSig<NonFunctionValue>`
  - `fn perform(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## ac_runtime_modules::module::module_stub::STUB_MODULE

*Static*

A static [`StubModule`] instance for convenience.

```rust
static STUB_MODULE: StubModule
```



## ac_runtime_modules::module::module_stub::StubModule

*Struct*

A minimal module implementation for testing.

**Unit Struct**

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`



