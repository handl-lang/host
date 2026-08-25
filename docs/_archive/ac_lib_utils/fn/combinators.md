**handl_lib_utils > fn > combinators**

# Module: fn::combinators

## Contents

**Structs**

- [`Apply`](#apply)
- [`Choice`](#choice)
- [`Compose`](#compose)
- [`Constantly`](#constantly)
- [`Converge`](#converge)
- [`Fanout`](#fanout)
- [`Flip`](#flip)
- [`Flow`](#flow)
- [`IfLazy`](#iflazy) - Lazy `if(cond, then, else)` — only evaluates the chosen branch.
- [`Match`](#match)
- [`Partial`](#partial)
- [`Pipe`](#pipe)
- [`Tap`](#tap)
- [`Unless`](#unless)
- [`When`](#when)

**Functions**

- [`inline_chain`](#inline_chain)
- [`is_ident_char`](#is_ident_char)
- [`match_pattern`](#match_pattern)
- [`next_char_boundary`](#next_char_boundary)
- [`substitute_param`](#substitute_param)

---

## handl_lib_utils::fn::combinators::Apply

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Apply`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Choice

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Choice`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Compose

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Compose`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Constantly

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Constantly`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Converge

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Converge`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Fanout

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Fanout`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Flip

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Flip`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Flow

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Flow`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::IfLazy

*Struct*

Lazy `if(cond, then, else)` — only evaluates the chosen branch.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IfLazy`
- **LazyFunction**
  - `fn call(self: &Self, args: &[Expr<NonFunctionValue>], ctx: & mut dyn Context, evaluate: & mut dyn FnMut) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Match

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Match`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Partial

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Partial`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Pipe

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Pipe`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Tap

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Tap`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::Unless

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Unless`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::When

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> When`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::fn::combinators::inline_chain

*Function*

```rust
fn inline_chain(fns: &[ac_runtime_values::FunctionValue]) -> Result<ac_runtime_values::FunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::fn::combinators::is_ident_char

*Function*

```rust
fn is_ident_char(b: u8) -> bool
```



## handl_lib_utils::fn::combinators::match_pattern

*Function*

```rust
fn match_pattern(pattern: &serde_json::Value, value: &serde_json::Value, out: & mut Vec<(String, serde_json::Value)>) -> Result<bool, ac_runtime_values::Error>
```



## handl_lib_utils::fn::combinators::next_char_boundary

*Function*

```rust
fn next_char_boundary(s: &str, i: usize) -> usize
```



## handl_lib_utils::fn::combinators::substitute_param

*Function*

```rust
fn substitute_param(body: &str, param: &str, replacement: &str) -> String
```



