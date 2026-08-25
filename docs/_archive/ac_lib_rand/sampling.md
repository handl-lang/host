**handl_lib_rand > sampling**

# Module: sampling

## Contents

**Structs**

- [`Choice`](#choice) - `rand.choice(array: array, count: number) → array`
- [`Pick`](#pick) - `rand.pick(array: array) → value`
- [`Sample`](#sample) - `rand.sample(array: array, count: number) → array`
- [`Shuffle`](#shuffle) - `rand.shuffle(array: array) → array`
- [`WeightedPick`](#weightedpick) - `rand.weighted_pick(items: array, weights: array<number>) → value`

---

## handl_lib_rand::sampling::Choice

*Struct*

`rand.choice(array: array, count: number) → array`

Returns `count` elements chosen uniformly at random **with
replacement** from the input array. Duplicate elements may appear.

`count` must be a non-negative integer. The array must be non-empty.

Example:
- `rand.choice([1, 2, 3], 5)` → 5 elements, possibly with duplicates

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Choice`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::sampling::Pick

*Struct*

`rand.pick(array: array) → value`

Returns a single random element from the array.

Returns an error if the array is empty.

Example:
- `rand.pick(["a", "b", "c"])` → one of `"a"`, `"b"`, `"c"`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Pick`



## handl_lib_rand::sampling::Sample

*Struct*

`rand.sample(array: array, count: number) → array`

Returns `count` elements chosen uniformly at random **without
replacement** from the input array.

`count` must be a non-negative integer and must not exceed the array
length.

Example:
- `rand.sample([1, 2, 3, 4, 5], 3)` → 3 distinct elements in random order

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Sample`



## handl_lib_rand::sampling::Shuffle

*Struct*

`rand.shuffle(array: array) → array`

Returns a new array with the elements of the input in random order.
The original array is not modified (values are immutable in handl).

Example:
- `rand.shuffle([1, 2, 3, 4, 5])` → e.g. `[3, 1, 5, 2, 4]`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Shuffle`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::sampling::WeightedPick

*Struct*

`rand.weighted_pick(items: array, weights: array<number>) → value`

Returns a single element from `items` chosen with probability
proportional to the corresponding weight in `weights`.

Both arrays must have the same length, and at least one weight must be
positive.

Example:
- `rand.weighted_pick(["a", "b", "c"], [1, 1, 2])` → `"c"` twice as likely

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> WeightedPick`



