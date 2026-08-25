# handl_compiler

# handl-compiler

Compiler pipeline abstractions.

This crate exports the typed pipeline primitives from `driver-lang`
(`Stage`, `Session`, `Pipeline`, `Then`), the compiler `Config`, the
`Source` type, and the artifact types (`ParsedDoc`, `ExtractedDoc`, etc.).

Concrete preset pipelines and default configurations live in
`handl-compiler-canonical`; reusable `Stage` implementations live in
`handl-compiler-common`.

## Modules

### [`handl_compiler`](handl_compiler.md)

*5 modules*

### [`config`](config.md)

*1 struct*

### [`config::Config`](config/Config.md)

*2 functions*

### [`diagnostic`](diagnostic.md)

*1 enum, 2 structs*

### [`driver`](driver.md)

*5 structs*

### [`source`](source.md)

*1 struct*

### [`visitor`](visitor.md)

*2 traits, 6 functions*

