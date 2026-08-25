**handl_frontend**

# Module: handl_frontend

## Contents

**Modules**

- [`handl`](#handl) - # handl text frontend
- [`json`](#json)
- [`toml`](#toml)
- [`yaml`](#yaml)

**Functions**

- [`frontend_for`](#frontend_for) - Return a `Frontend` for the given format name.

---

## Module: handl

# handl text frontend

Passes through raw canonical handl source as a string. The string is
intended to be lexed and parsed by `handl-lexer-canonical` / `handl-parser-canonical`
downstream (e.g. in the `EvalStage`), rather than being interpreted as a
JSON-compatible document.



## handl_frontend::frontend_for

*Function*

Return a `Frontend` for the given format name.

Supported formats: `yaml`/`yml`, `json`, `toml`, `handl`.

```rust
fn frontend_for(format: &str) -> Result<Box<dyn Frontend>, String>
```



## Module: json



## Module: toml



## Module: yaml



