**handl_lib_test > error**

# Module: error

## Contents

**Enums**

- [`TestError`](#testerror)

---

## handl_lib_test::error::TestError

*Enum*

**Variants:**
- `ArgumentCount{ expected: usize, got: usize }`
- `TypeMismatch{ expected: &'static str, got: String }`
- `AssertionFailed{ message: String }`
- `PropertyFailed{ counterexample: serde_json::Value, message: String }`
- `FuzzFailure{ run: usize, message: String }`
- `SnapshotMismatch{ name: String, actual: serde_json::Value, expected: serde_json::Value }`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> TestError`



