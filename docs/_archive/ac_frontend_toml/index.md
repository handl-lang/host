# handl_frontend_toml

# handl-frontend-toml

TOML frontend for the handl language.

Translates TOML into the core [`NonFunctionValue`] representation.
TOML is a frontend — it produces the same value type the VM
operates on. No execution logic lives here.

## Architectural rule

> **Formats translate external representations.**

## Modules

### [`handl_frontend_toml`](handl_frontend_toml.md)

*1 struct, 3 modules, 4 functions*

### [`convert`](convert.md)

*2 functions*

### [`error`](error.md)

*1 enum*

### [`scanner`](scanner.md)

*1 constant, 1 struct, 3 functions*

