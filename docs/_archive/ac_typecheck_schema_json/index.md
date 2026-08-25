# handl_typecheck_schema_json

# handl-typecheck-schema-json

JSON Schema → [`Type`] mapping for the handl typechecker.

Implements the [`SchemaInterpreter`] trait from `handl-typecheck-core`,
converting JSON Schema fragments into neutral [`Type`] values.

## Architectural rule

> **The interpreter reads the schema vocabulary; the core provides
> the type system.**

## Modules

### [`handl_typecheck_schema_json`](handl_typecheck_schema_json.md)

*1 module, 1 struct*

### [`interpret`](interpret.md)

*3 functions*

