# handl_compiler_canonical

# handl-compiler-canonical

Concrete, canonical compiler — default configurations and preset
[`DriverPipeline`](handl_compiler::DriverPipeline) chains wired from the
reusable stages in `handl-compiler-common`.

## Presets

- `typegen_pipeline` — parse → extract → resolve → generate
- `typecheck_pipeline` — parse → extract → typecheck
- `full_pipeline` — parse → extract → resolve → typecheck → generate

## Non-pipeline operations

`parse`, `eval`, `validate`, and `fields` have their own entry points.

## Modules

### [`handl_compiler_canonical`](handl_compiler_canonical.md)

*5 functions, 7 modules*

### [`eval`](eval.md)

*1 enum, 1 function, 1 struct*

### [`fields`](fields.md)

*1 function, 2 structs*

### [`parse`](parse.md)

*1 function*

### [`resolver`](resolver.md)

*1 function, 3 structs*

### [`typecheck`](typecheck.md)

*1 function, 1 struct*

### [`typegen`](typegen.md)

*1 enum, 1 function, 1 struct*

### [`validate`](validate.md)

*1 function, 1 struct*

