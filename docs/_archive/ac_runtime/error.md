**ac_runtime > error**

# Module: error

## Contents

**Enums**

- [`Error`](#error) - Errors that can occur during evaluation.

---

## ac_runtime::error::Error

*Enum*

Errors that can occur during evaluation.

All errors are cloneable and comparable, making them suitable
for testing and replay scenarios.

**Generic Parameters:**
- V

**Variants:**
- `UnknownFunction(String)` - The function name was not recognized by the host.
- `UnknownVariable(String)` - The variable name was not found in any scope.
- `PrimitiveError(String)` - A primitive returned an error during execution.
- `TypeMismatch{ expected: &'static str, got: &'static str }` - A value did not match the expected type.
- `ArgumentCount{ expected: usize, got: usize }` - The argument count did not match what the primitive expected.
- `Yield(V)` - The VM yielded control to the caller.
- `Operation{ name: String, args: Vec<V> }` - A typed operation was performed and signaled.
- `ResourceLimit(String)` - A resource limit was exceeded during evaluation.
- `ReservedName(String)` - A name is reserved for a host module and may not be assigned to.

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Error<V>) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Error<V>`



