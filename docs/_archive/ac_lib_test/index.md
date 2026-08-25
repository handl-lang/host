# handl_lib_test

# handl-lib-test

First-class testing standard library for the handl runtime.

Provides the runtime modules used by handl expressions:

- [`TestModule`] — `test.run`, `test.parallel`, `test.fixture`
- [`AssertModule`] — `assert.equal`, `assert.not_equal`, `assert.true`,
  `assert.false`, `assert.contains`, `assert.throws`
- [`PropertyModule`] — `property.for_all`
- [`BenchmarkModule`] — `benchmark.run`
- [`FuzzModule`] — `fuzz.run`
- [`SnapshotModule`] — `snapshot.match`, `snapshot.format`

All modules are stateless, `Copy`, and follow the `Function<NonFunctionValue>`
contract used by the rest of the standard library.

## Modules

### [`handl_lib_test`](handl_lib_test.md)

*8 modules*

### [`assert`](assert.md)

*7 structs*

### [`benchmark`](benchmark.md)

*2 structs*

### [`error`](error.md)

*1 enum*

### [`fuzz`](fuzz.md)

*2 structs*

### [`property`](property.md)

*2 structs*

### [`shared`](shared.md)

*12 functions*

### [`snapshot`](snapshot.md)

*3 structs*

### [`test`](test.md)

*2 structs*

