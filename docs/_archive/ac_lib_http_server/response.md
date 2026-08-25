**handl_lib_http_server > response**

# Module: response

## Contents

**Structs**

- [`HttpServerResponse`](#httpserverresponse) - HTTP response.

---

## handl_lib_http_server::response::HttpServerResponse

*Struct*

HTTP response.

**Fields:**
- `status: u16`
- `status_text: String`
- `body: String`
- `headers: std::collections::HashMap<String, String>`

**Methods:**

- `fn new<impl Into<String>>(status: u16, body: impl Trait) -> Self`
- `fn with_header<impl Into<String>, impl Into<String>>(self: Self, name: impl Trait, value: impl Trait) -> Self`
- `fn with_status_text<impl Into<String>>(self: Self, text: impl Trait) -> Self`
- `fn ok(self: &Self) -> bool`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HttpServerResponse`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



