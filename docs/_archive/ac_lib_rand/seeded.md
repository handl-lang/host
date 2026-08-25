**handl_lib_rand > seeded**

# Module: seeded

## Contents

**Structs**

- [`SeededBool`](#seededbool) - `rand.seeded_bool(seed: number) → bool`
- [`SeededChoice`](#seededchoice) - `rand.seeded_choice(seed: number, array: array, count: number) → array`
- [`SeededFloat`](#seededfloat) - `rand.seeded_float(seed: number, min: number, max: number) → number`
- [`SeededGauss`](#seededgauss) - `rand.seeded_gauss(seed: number, mean: number, stddev: number) → number`
- [`SeededInt`](#seededint) - `rand.seeded_int(seed: number, min: number, max: number) → number`
- [`SeededPick`](#seededpick) - `rand.seeded_pick(seed: number, array: array) → value`
- [`SeededSample`](#seededsample) - `rand.seeded_sample(seed: number, array: array, count: number) → array`
- [`SeededShuffle`](#seededshuffle) - `rand.seeded_shuffle(seed: number, array: array) → array`
- [`SeededString`](#seededstring) - `rand.seeded_string(seed: number, length: number, charset?: string) → string`

**Functions**

- [`seeded_rng`](#seeded_rng)

---

## handl_lib_rand::seeded::SeededBool

*Struct*

`rand.seeded_bool(seed: number) → bool`

Returns a deterministic random boolean based on `seed`.

Example:
- `rand.seeded_bool(42)` → always the same result

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SeededBool`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::seeded::SeededChoice

*Struct*

`rand.seeded_choice(seed: number, array: array, count: number) → array`

Returns `count` elements chosen deterministically **with replacement**
from the array based on `seed`.

Example:
- `rand.seeded_choice(42, [1, 2, 3], 5)` → always the same 5 elements

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SeededChoice`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::seeded::SeededFloat

*Struct*

`rand.seeded_float(seed: number, min: number, max: number) → number`

Returns a deterministic random float in `[min, max)` based on `seed`.

Example:
- `rand.seeded_float(42, 0, 1)` → always the same value in [0, 1)

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> SeededFloat`



## handl_lib_rand::seeded::SeededGauss

*Struct*

`rand.seeded_gauss(seed: number, mean: number, stddev: number) → number`

Returns a deterministic random float from a Gaussian distribution
based on `seed`.

Example:
- `rand.seeded_gauss(42, 0, 1)` → always the same standard normal sample

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SeededGauss`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::seeded::SeededInt

*Struct*

`rand.seeded_int(seed: number, min: number, max: number) → number`

Returns a deterministic random integer in `[min, max]` based on `seed`.
Same seed + same args always produces the same result.

Examples:
- `rand.seeded_int(42, 1, 6)` → always the same die roll
- `rand.seeded_int(123, 0, 100)` → always the same value

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SeededInt`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::seeded::SeededPick

*Struct*

`rand.seeded_pick(seed: number, array: array) → value`

Returns a single deterministic element from the array based on `seed`.

Example:
- `rand.seeded_pick(42, ["a", "b", "c"])` → always the same element

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> SeededPick`



## handl_lib_rand::seeded::SeededSample

*Struct*

`rand.seeded_sample(seed: number, array: array, count: number) → array`

Returns `count` elements chosen deterministically **without replacement**
from the array based on `seed`.

Example:
- `rand.seeded_sample(42, [1, 2, 3, 4, 5], 3)` → always the same 3 elements

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> SeededSample`



## handl_lib_rand::seeded::SeededShuffle

*Struct*

`rand.seeded_shuffle(seed: number, array: array) → array`

Returns a new array with elements deterministically shuffled based on `seed`.

Example:
- `rand.seeded_shuffle(42, [1, 2, 3, 4, 5])` → always the same order

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SeededShuffle`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::seeded::SeededString

*Struct*

`rand.seeded_string(seed: number, length: number, charset?: string) → string`

Returns a deterministic random string of the given length based on `seed`.

Example:
- `rand.seeded_string(42, 16)` → always the same 16-char string

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> SeededString`



## handl_lib_rand::seeded::seeded_rng

*Function*

```rust
fn seeded_rng(seed: f64) -> Result<rand::rngs::StdRng, ac_runtime_values::Error>
```



