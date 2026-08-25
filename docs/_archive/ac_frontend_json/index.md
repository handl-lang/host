# handl_frontend_json

# handl-frontend-json

JSON frontend for the handl language.

Translates JSON into the core [`NonFunctionValue`] representation.
Since `NonFunctionValue` wraps `serde_json::Value`, this frontend is
a thin adapter — parsing produces a `NonFunctionValue` directly.

## Architectural rule

> **Formats translate external representations.**

## Modules

### [`handl_frontend_json`](handl_frontend_json.md)

*1 enum, 1 module, 1 struct, 5 functions*

### [`scanner`](scanner.md)

*1 constant, 3 functions*

