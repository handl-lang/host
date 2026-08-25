**handl_lib_datetime > datetime**

# Module: datetime

## Contents

**Modules**

- [`arithmetic`](#arithmetic)
- [`bounds`](#bounds)
- [`common`](#common)
- [`compare`](#compare)
- [`components`](#components)
- [`construct`](#construct)
- [`convert`](#convert)
- [`duration`](#duration)
- [`format_parse`](#format_parse)
- [`timezone`](#timezone)
- [`validate`](#validate)

**Structs**

- [`DateTimeModule`](#datetimemodule) - The `datetime` module — date and time utility functions.

---

## handl_lib_datetime::datetime::DateTimeModule

*Struct*

The `datetime` module — date and time utility functions.

Provides functions callable as `datetime.from_unix(ts)`, `datetime.format(d, fmt)`,
`datetime.parse(s)`, `datetime.add_days(d, n)`, etc. in handl expressions.

Datetimes are represented as ISO 8601 strings (`YYYY-MM-DDTHH:MM:SSZ`).
Unix timestamps are represented as numbers (seconds since epoch).

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> DateTimeModule`



## Module: arithmetic



## Module: bounds



## Module: common



## Module: compare



## Module: components



## Module: construct



## Module: convert



## Module: duration



## Module: format_parse



## Module: timezone



## Module: validate



