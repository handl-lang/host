**handl_lib_rand > distribution**

# Module: distribution

## Contents

**Structs**

- [`Binomial`](#binomial) - `rand.binomial(n: number, p: number) → number`
- [`Exponential`](#exponential) - `rand.exponential(lambda: number) → number`
- [`Gauss`](#gauss) - `rand.gauss(mean: number, stddev: number) → number`
- [`Poisson`](#poisson) - `rand.poisson(lambda: number) → number`

---

## handl_lib_rand::distribution::Binomial

*Struct*

`rand.binomial(n: number, p: number) → number`

Returns a random non-negative integer sampled from a binomial
distribution with `n` trials and success probability `p`.

`n` must be a non-negative integer and `p` must be in `[0, 1]`.

Examples:
- `rand.binomial(10, 0.5)` → e.g. 4, 5, 6
- `rand.binomial(100, 0.1)` → e.g. 8, 9, 10, 11

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Binomial`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::distribution::Exponential

*Struct*

`rand.exponential(lambda: number) → number`

Returns a random float sampled from an exponential distribution
with rate parameter `lambda`. The result is non-negative.

`lambda` must be positive.

Examples:
- `rand.exponential(1)` → mean 1.0
- `rand.exponential(0.5)` → mean 2.0

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Exponential`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::distribution::Gauss

*Struct*

`rand.gauss(mean: number, stddev: number) → number`

Returns a random float sampled from a normal (Gaussian) distribution
with the given `mean` and standard deviation `stddev`.

Uses the Box-Muller transform. `stddev` must be non-negative.

Examples:
- `rand.gauss(0, 1)` → standard normal (mean 0, stddev 1)
- `rand.gauss(100, 15)` → IQ-scale distribution

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Gauss`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::distribution::Poisson

*Struct*

`rand.poisson(lambda: number) → number`

Returns a random non-negative integer sampled from a Poisson
distribution with rate parameter `lambda`.

`lambda` must be non-negative.

Examples:
- `rand.poisson(3)` → e.g. 2, 3, 4, 5
- `rand.poisson(0)` → always 0

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Poisson`



