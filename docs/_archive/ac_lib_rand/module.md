**handl_lib_rand > module**

# Module: module

## Contents

**Structs**

- [`RandModule`](#randmodule) - The `rand` module — random number generation and sampling.

---

## handl_lib_rand::module::RandModule

*Struct*

The `rand` module — random number generation and sampling.

Provides functions callable as `rand.int(max)`, `rand.int(min, max)`,
`rand.float()`, `rand.float(max)`, `rand.float(min, max)`,
`rand.bool()`, `rand.pick(array)`, `rand.choice(array, count)`, `rand.sample(array, count)`,
`rand.shuffle(array)`, `rand.weighted_pick(items, weights)`,
`rand.string(length, charset?)`, `rand.bytes(length)`,
`rand.uuid()`, `rand.gauss(mean, stddev)`, `rand.exponential(lambda)`,
`rand.poisson(lambda)`, `rand.binomial(n, p)`,
`rand.perlin_1d(x)`, `rand.perlin_2d(x, y)`, `rand.perlin_3d(x, y, z)`,
`rand.simplex_2d(x, y)`, `rand.simplex_3d(x, y, z)`,
`rand.seeded_int(seed, min, max)`, `rand.seeded_float(seed, min, max)`,
`rand.seeded_bool(seed)`, `rand.seeded_pick(seed, array)`,
`rand.seeded_choice(seed, array, count)`, `rand.seeded_sample(seed, array, count)`,
`rand.seeded_shuffle(seed, array)`, `rand.seeded_string(seed, length, charset?)`,
`rand.seeded_gauss(seed, mean, stddev)` in handl expressions.

All non-seeded functions use the system RNG via the `rand` crate.
Each call is independent — there is no shared state between calls.
Seeded functions are deterministic — same seed + same args always
produce the same result.
Perlin noise functions are deterministic (fixed permutation table).

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> RandModule`
- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



