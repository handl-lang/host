**handl_lib_os > time > time**

# Module: time::time

## Contents

**Structs**

- [`EpochFn`](#epochfn)
- [`NowFn`](#nowfn)
- [`TimeModule`](#timemodule) - Module adapter for [`Time`] — exposes `time.*` functions.

**Traits**

- [`Time`](#time) - Abstract time service.

---

## handl_lib_os::time::time::EpochFn

*Struct*

**Fields:**
- `time: std::sync::Arc<dyn Time>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::time::time::NowFn

*Struct*

**Fields:**
- `time: std::sync::Arc<dyn Time>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::time::time::Time

*Trait*

Abstract time service.

Implementations:
- `handl-lib-os-std` — backed by `std::time::SystemTime`
- [`time_stub::StubTime`] — fixed time for testing

**Methods:**

- `now_millis`: Returns the current time as epoch milliseconds.
- `now_seconds`: Returns the current time as epoch seconds.



## handl_lib_os::time::time::TimeModule

*Struct*

Module adapter for [`Time`] — exposes `time.*` functions.

**Fields:**
- `now: NowFn`
- `epoch: EpochFn`

**Methods:**

- `fn new<impl Time + 'static>(time: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



