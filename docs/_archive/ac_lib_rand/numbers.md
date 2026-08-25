**handl_lib_rand > numbers**

# Module: numbers

## Contents

**Structs**

- [`Bool`](#bool) - `rand.bool() → bool`
- [`Float`](#float) - `rand.float() → number` / `rand.float(max: number) → number` / `rand.float(min: number, max: number) → number`
- [`Int`](#int) - `rand.int(max: number) → number` / `rand.int(min: number, max: number) → number`

---

## handl_lib_rand::numbers::Bool

*Struct*

`rand.bool() → bool`

Returns a random boolean with equal probability for `true` and `false`.

Example:
- `rand.bool()` → `true` or `false`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Bool`



## handl_lib_rand::numbers::Float

*Struct*

`rand.float() → number` / `rand.float(max: number) → number` / `rand.float(min: number, max: number) → number`

Returns a random float. With no arguments, the result is in `[0, 1)`.
With one argument, the result is in `[0, max)`.
With two arguments, the result is in `[min, max)`.

Examples:
- `rand.float()` → 0.0 ≤ x < 1.0
- `rand.float(10)` → 0.0 ≤ x < 10.0
- `rand.float(10, 20)` → 10.0 ≤ x < 20.0

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Float`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::numbers::Int

*Struct*

`rand.int(max: number) → number` / `rand.int(min: number, max: number) → number`

Returns a random integer in the inclusive range `[0, max]` (one-arg)
or `[min, max]` (two-arg).

Both `min` and `max` must be integers and `min` must be <= `max`.

Examples:
- `rand.int(6)` → 0–6 inclusive
- `rand.int(1, 6)` → a die roll (1–6)
- `rand.int(0, 100)` → 0–100 inclusive

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Int`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



