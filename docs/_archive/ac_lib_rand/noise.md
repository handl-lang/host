**handl_lib_rand > noise**

# Module: noise

## Contents

**Structs**

- [`Perlin1D`](#perlin1d) - `rand.perlin_1d(x: number) → number`
- [`Perlin2D`](#perlin2d) - `rand.perlin_2d(x: number, y: number) → number`
- [`Perlin3D`](#perlin3d) - `rand.perlin_3d(x: number, y: number, z: number) → number`
- [`Simplex2D`](#simplex2d) - `rand.simplex_2d(x: number, y: number) → number`
- [`Simplex3D`](#simplex3d) - `rand.simplex_3d(x: number, y: number, z: number) → number`

**Functions**

- [`fade`](#fade)
- [`grad`](#grad)
- [`grad3`](#grad3)
- [`lerp`](#lerp)
- [`perlin_1d`](#perlin_1d)
- [`perlin_2d`](#perlin_2d)
- [`perlin_3d`](#perlin_3d)
- [`perm_index`](#perm_index)
- [`simplex_2d`](#simplex_2d)
- [`simplex_3d`](#simplex_3d)

**Constants**

- [`F2`](#f2)
- [`F3`](#f3)
- [`G2`](#g2)
- [`G3`](#g3)
- [`GRAD3`](#grad3)
- [`PERM`](#perm)

---

## handl_lib_rand::noise::F2

*Constant*: `f64`



## handl_lib_rand::noise::F3

*Constant*: `f64`



## handl_lib_rand::noise::G2

*Constant*: `f64`



## handl_lib_rand::noise::G3

*Constant*: `f64`



## handl_lib_rand::noise::GRAD3

*Constant*: `[(f64, f64, f64); 12]`



## handl_lib_rand::noise::PERM

*Constant*: `[u8; 256]`



## handl_lib_rand::noise::Perlin1D

*Struct*

`rand.perlin_1d(x: number) → number`

Returns 1D Perlin noise value at coordinate `x`. Output is
approximately in the range `[-1, 1]`.

Uses a fixed permutation table (Ken Perlin's original) for
deterministic results across runs.

Example:
- `rand.perlin_1d(0.5)` → noise at x=0.5

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Perlin1D`



## handl_lib_rand::noise::Perlin2D

*Struct*

`rand.perlin_2d(x: number, y: number) → number`

Returns 2D Perlin noise value at coordinates `(x, y)`. Output is
approximately in the range `[-1, 1]`.

Uses a fixed permutation table (Ken Perlin's original) for
deterministic results across runs.

Example:
- `rand.perlin_2d(1.0, 2.0)` → noise at (1, 2)

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Perlin2D`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::noise::Perlin3D

*Struct*

`rand.perlin_3d(x: number, y: number, z: number) → number`

Returns 3D Perlin noise value at coordinates `(x, y, z)`. Output is
approximately in the range `[-1, 1]`.

Uses a fixed permutation table (Ken Perlin's original) for
deterministic results across runs.

Example:
- `rand.perlin_3d(0.1, 0.2, 0.3)` → noise at (0.1, 0.2, 0.3)

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Perlin3D`



## handl_lib_rand::noise::Simplex2D

*Struct*

`rand.simplex_2d(x: number, y: number) → number`

Returns 2D Simplex noise value at coordinates `(x, y)`. Output is
approximately in the range `[-1, 1]`.

Uses a fixed permutation table for deterministic results across runs.
Simplex noise has fewer directional artifacts than Perlin noise.

Example:
- `rand.simplex_2d(1.0, 2.0)` → noise at (1, 2)

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Simplex2D`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::noise::Simplex3D

*Struct*

`rand.simplex_3d(x: number, y: number, z: number) → number`

Returns 3D Simplex noise value at coordinates `(x, y, z)`. Output is
approximately in the range `[-1, 1]`.

Uses a fixed permutation table for deterministic results across runs.
Simplex noise has fewer directional artifacts than Perlin noise.

Example:
- `rand.simplex_3d(0.1, 0.2, 0.3)` → noise at (0.1, 0.2, 0.3)

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Simplex3D`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_rand::noise::fade

*Function*

```rust
fn fade(t: f64) -> f64
```



## handl_lib_rand::noise::grad

*Function*

```rust
fn grad(hash: u8, x: f64, y: f64, z: f64) -> f64
```



## handl_lib_rand::noise::grad3

*Function*

```rust
fn grad3(hash: u8, x: f64, y: f64, z: f64) -> f64
```



## handl_lib_rand::noise::lerp

*Function*

```rust
fn lerp(a: f64, b: f64, t: f64) -> f64
```



## handl_lib_rand::noise::perlin_1d

*Function*

```rust
fn perlin_1d(x: f64) -> f64
```



## handl_lib_rand::noise::perlin_2d

*Function*

```rust
fn perlin_2d(x: f64, y: f64) -> f64
```



## handl_lib_rand::noise::perlin_3d

*Function*

```rust
fn perlin_3d(x: f64, y: f64, z: f64) -> f64
```



## handl_lib_rand::noise::perm_index

*Function*

```rust
fn perm_index(i: i32) -> usize
```



## handl_lib_rand::noise::simplex_2d

*Function*

```rust
fn simplex_2d(x: f64, y: f64) -> f64
```



## handl_lib_rand::noise::simplex_3d

*Function*

```rust
fn simplex_3d(x: f64, y: f64, z: f64) -> f64
```



