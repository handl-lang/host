**handl_lib_os > sig > sig**

# Module: sig::sig

## Contents

**Structs**

- [`ListFn`](#listfn)
- [`SendFn`](#sendfn)
- [`SigModule`](#sigmodule) - Module adapter for [`Sig`] — exposes `sig.*` functions.

**Traits**

- [`Sig`](#sig) - Abstract signal service.

---

## handl_lib_os::sig::sig::ListFn

*Struct*

**Fields:**
- `sig: std::sync::Arc<dyn Sig>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::sig::sig::SendFn

*Struct*

**Fields:**
- `sig: std::sync::Arc<dyn Sig>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::sig::sig::Sig

*Trait*

Abstract signal service.

Implementations:
- `handl-lib-os-std` — backed by platform signal APIs
- [`sig_stub::StubSig`] — canned responses for testing

**Methods:**

- `send`
- `list`



## handl_lib_os::sig::sig::SigModule

*Struct*

Module adapter for [`Sig`] — exposes `sig.*` functions.

**Fields:**
- `send: SendFn`
- `list: ListFn`

**Methods:**

- `fn new<impl Sig + 'static>(sig: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



