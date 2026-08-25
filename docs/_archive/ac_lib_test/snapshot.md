**handl_lib_test > snapshot**

# Module: snapshot

## Contents

**Structs**

- [`Format`](#format)
- [`Match`](#match)
- [`SnapshotModule`](#snapshotmodule) - The `snapshot` module — snapshot formatting and matching.

---

## handl_lib_test::snapshot::Format

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Format`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_test::snapshot::Match

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Match`



## handl_lib_test::snapshot::SnapshotModule

*Struct*

The `snapshot` module — snapshot formatting and matching.

Functions: `snapshot.format`, `snapshot.match`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> SnapshotModule`



