# handl_dialect_canonical

# handl-dialect-canonical

The canonical handl language dialect implementation.

Implements the [`Dialect`] trait with the syntax defined in the handl
Language Specification:
- Interpolation: `${...}`
- Path separator: `.`
- Function calls: `fn(args)`
- String literals: `"..."` or `'...'`
- Number literals: `42`, `1.5`, `-3`
- Boolean literals: `true`, `false`
- Null literal: `null`
- Schema vocabulary: JSON Schema draft 2020-12

This is the only shipped dialect. See [SPEC.md](../dialect/SPEC.md)
for the full dialect specification.

## Modules

### [`handl_dialect_canonical`](handl_dialect_canonical.md)

*5 modules*

### [`dialect`](dialect.md)

*1 module*

### [`dialect::dialect`](dialect/dialect.md)

*1 static, 1 struct*

### [`document_vocabulary`](document_vocabulary.md)

*1 module*

### [`expr_parser`](expr_parser.md)

*1 module*

### [`schema_vocabulary`](schema_vocabulary.md)

*2 modules*

### [`schema_vocabulary::json_schema`](schema_vocabulary/json_schema.md)

*1 struct*

### [`schema_vocabulary::json_schema_vocabulary`](schema_vocabulary/json_schema_vocabulary.md)

*1 static, 1 struct*

### [`syntax`](syntax.md)

*1 module*

