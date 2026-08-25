**ac_runtime_values > frontend**

# Module: frontend

## Contents

**Traits**

- [`Frontend`](#frontend) - A format frontend that parses and serializes `NonFunctionValue`s.

---

## ac_runtime_values::frontend::Frontend

*Trait*

A format frontend that parses and serializes `NonFunctionValue`s.

**Methods:**

- `Error`: The error type returned by parse and serialize operations.
- `parse`: Parses a string in this format into a [`NonFunctionValue`].
- `serialize`: Serializes a [`NonFunctionValue`] to a string in this format.
- `parse_with_locations`: Parses a string and builds a [`SourceMap`] mapping dotted paths



