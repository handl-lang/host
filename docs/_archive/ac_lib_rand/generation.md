**handl_lib_rand > generation**

# Module: generation

## Contents

**Structs**

- [`Bytes`](#bytes) - `rand.bytes(length: number) → array<number>`
- [`String_`](#string_) - `rand.string(length: number, charset?: string) → string`
- [`Uuid`](#uuid) - `rand.uuid() → string`

---

## handl_lib_rand::generation::Bytes

*Struct*

`rand.bytes(length: number) → array<number>`

Returns an array of `length` random bytes (integers in the range 0–255).

`length` must be a non-negative integer.

Example:
- `rand.bytes(8)` → e.g. `[42, 255, 0, 128, 1, 64, 200, 17]`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Bytes`



## handl_lib_rand::generation::String_

*Struct*

`rand.string(length: number, charset?: string) → string`

Returns a random string of the given length using the specified
character set.

`length` must be a non-negative integer. `charset` defaults to
`"alphanumeric"` and must be one of:

- `"alphanumeric"` — `a-z`, `A-Z`, `0-9` (default)
- `"alpha"` — `a-z`, `A-Z`
- `"numeric"` — `0-9`
- `"hex"` — `0-9`, `a-f`
- `"lower"` — `a-z`
- `"upper"` — `A-Z`

Examples:
- `rand.string(16)` → e.g. `"aB3xK9mN2pQr7sT1"`
- `rand.string(8, "hex")` → e.g. `"3f7a1c9d"`
- `rand.string(10, "numeric")` → e.g. `"4729103856"`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> String_`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::generation::Uuid

*Struct*

`rand.uuid() → string`

Returns a version-4 UUID string in the canonical 8-4-4-4-12 format.

Version and variant bits are set per RFC 4122. The randomness is
sourced from the system CSPRNG via the `rand` crate.

Example:
- `rand.uuid()` → `"f47ac10b-58cc-4372-a567-0e02b2c3d479"`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Uuid`
- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



