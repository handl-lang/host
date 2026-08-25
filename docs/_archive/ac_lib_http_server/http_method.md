**handl_lib_http_server > http_method**

# Module: http_method

## Contents

**Enums**

- [`HttpMethod`](#httpmethod) - HTTP method enum.

---

## handl_lib_http_server::http_method::HttpMethod

*Enum*

HTTP method enum.

**Variants:**
- `Get`
- `Post`
- `Put`
- `Patch`
- `Delete`
- `Head`
- `Options`

**Methods:**

- `fn as_str(self: &Self) -> &'static str`
- `fn parse(s: &str) -> Option<Self>`

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &HttpMethod) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> HttpMethod`



