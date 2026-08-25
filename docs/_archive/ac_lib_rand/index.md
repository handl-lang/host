# handl_lib_rand

# handl-lib-rand

Random number generation and sampling library for the handl runtime.

Provides functions for generating random integers, floats, booleans,
picking and shuffling elements, generating random strings and byte
arrays, and producing version-4 UUIDs.

## Architecture

```text
handl-runtime-core          handl-lib-rand
─────────────            ────────────
Module trait             RandModule (Copy, zero-alloc)
ModuleHost               *Function structs
                         RandError → VmError
```

All function structs are `Copy` with no internal state. Each call
draws from the system RNG independently. Seeded variants use
`StdRng` seeded from a user-provided value for reproducibility.

## Functions

- [`RandModule`] — `rand.int`, `rand.float`, `rand.bool`,
  `rand.pick`, `rand.choice`, `rand.sample`, `rand.shuffle`, `rand.weighted_pick`,
  `rand.string`, `rand.bytes`, `rand.uuid`, `rand.gauss`,
  `rand.exponential`, `rand.poisson`, `rand.binomial`,
  `rand.perlin_1d`, `rand.perlin_2d`, `rand.perlin_3d`,
  `rand.simplex_2d`, `rand.simplex_3d`,
  `rand.seeded_int`, `rand.seeded_float`, `rand.seeded_bool`,
  `rand.seeded_pick`, `rand.seeded_choice`, `rand.seeded_sample`,
  `rand.seeded_shuffle`, `rand.seeded_string`, `rand.seeded_gauss`

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_rand::RandModule;

let host = ModuleHost::new()
    .with(RandModule);
```

Expressions can then call `rand.int(1, 6)`, `rand.float()`,
`rand.bool()`, `rand.pick(["a", "b", "c"])`, `rand.choice([1,2,3], 5)`, `rand.sample([1,2,3], 2)`,
`rand.shuffle([1,2,3])`, `rand.weighted_pick(["a","b"], [1,3])`,
`rand.string(16)`, `rand.bytes(8)`, `rand.uuid()`, `rand.gauss(0, 1)`,
`rand.perlin_1d(0.5)`, `rand.perlin_2d(1, 2)`, `rand.perlin_3d(0.1, 0.2, 0.3)`,
`rand.seeded_int(42, 1, 6)`, `rand.seeded_shuffle(123, [1,2,3])`, etc.

## Modules

### [`handl_lib_rand`](handl_lib_rand.md)

*9 modules*

### [`common`](common.md)

*7 functions*

### [`distribution`](distribution.md)

*4 structs*

### [`error`](error.md)

*1 enum*

### [`generation`](generation.md)

*3 structs*

### [`module`](module.md)

*1 struct*

### [`noise`](noise.md)

*10 functions, 5 structs, 6 constants*

### [`numbers`](numbers.md)

*3 structs*

### [`sampling`](sampling.md)

*5 structs*

### [`seeded`](seeded.md)

*1 function, 9 structs*

