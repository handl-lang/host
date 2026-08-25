# handl_dialect

# handl-dialect

Dialect abstraction for the handl language implementation.

The handl language specification defines one canonical syntax. This crate
abstracts the syntax decisions, special key names, and schema vocabulary
behind a [`Dialect`] trait so the implementation can be structured around
a single source of truth.

## Architectural rule

> **The language spec fixes syntax. The implementation abstracts it.**

The spec says `${...}` for interpolation, `.` for paths, `fn()` for calls.
The `Dialect` trait lets the Rust implementation be parameterized without
changing the spec. The canonical implementation lives in `handl-dialect-canonical`.

## Modules

### [`handl_dialect`](handl_dialect.md)

*5 modules*

### [`dialect`](dialect.md)

*2 modules*

### [`dialect::dialect`](dialect/dialect.md)

*1 trait*

### [`dialect::dialect_stub`](dialect/dialect_stub.md)

*1 function, 1 static, 1 struct*

### [`document_vocabulary`](document_vocabulary.md)

*2 modules*

### [`document_vocabulary::document_vocabulary`](document_vocabulary/document_vocabulary.md)

*1 trait*

### [`expr_parser`](expr_parser.md)

*2 modules*

### [`expr_parser::expr_parser`](expr_parser/expr_parser.md)

*1 struct, 1 trait*

### [`schema_vocabulary`](schema_vocabulary.md)

*5 modules*

### [`schema_vocabulary::schema`](schema_vocabulary/schema.md)

*1 trait*

### [`schema_vocabulary::schema_stub`](schema_vocabulary/schema_stub.md)

*1 struct*

### [`schema_vocabulary::schema_vocabulary`](schema_vocabulary/schema_vocabulary.md)

*1 trait*

### [`schema_vocabulary::schema_vocabulary_stub`](schema_vocabulary/schema_vocabulary_stub.md)

*1 struct*

### [`schema_vocabulary::validation_error`](schema_vocabulary/validation_error.md)

*1 struct*

### [`syntax`](syntax.md)

*2 modules*

### [`syntax::syntax`](syntax/syntax.md)

*1 trait*

