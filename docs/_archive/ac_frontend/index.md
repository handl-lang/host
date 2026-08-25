# handl_frontend

# handl-frontend

Frontend factory — maps format names to `Frontend` implementations.

Provides a single entry point [`frontend_for`] that returns a boxed
`Frontend` for the requested format. Each frontend wraps the raw
parse/serialize functions from the format-specific crates.

This crate is interface-agnostic — usable from CLI, MCP, or any
other consumer that needs to parse/serialize handl documents.

## Modules

### [`handl_frontend`](handl_frontend.md)

*1 function, 4 modules*

### [`handl`](handl.md)

*1 struct*

### [`json`](json.md)

*1 struct*

### [`toml`](toml.md)

*1 struct*

### [`yaml`](yaml.md)

*1 struct*

