**ac_runtime_values**

# Module: ac_runtime_values

## Contents

**Modules**

- [`frontend`](#frontend) - Frontend trait — the abstract interface for format parsers.

**Type Aliases**

- [`Error`](#error) - Concrete error type for the JSON-backed runtime.

---

## ac_runtime_values::Error

*Type Alias*: `ac_runtime::Error<NonFunctionValue>`

Concrete error type for the JSON-backed runtime.



## Module: frontend

Frontend trait — the abstract interface for format parsers.

Each frontend (JSON, YAML, TOML, etc.) implements this trait.
The trait is generic over the error type, allowing each frontend
to expose its own error type while sharing a common interface.

## Architectural rule

> **Formats translate external representations.** The `Frontend`
> trait formalizes this contract — any format that can parse into
> `NonFunctionValue` and serialize back can implement it.



