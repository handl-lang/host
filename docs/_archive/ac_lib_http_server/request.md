**handl_lib_http_server > request**

# Module: request

## Contents

**Structs**

- [`HttpServerRequest`](#httpserverrequest) - HTTP request descriptor passed to route handlers.

---

## handl_lib_http_server::request::HttpServerRequest

*Struct*

HTTP request descriptor passed to route handlers.

**Fields:**
- `method: crate::http_method::HttpMethod`
- `path: String`
- `params: std::collections::HashMap<String, String>`
- `query: std::collections::HashMap<String, String>`
- `headers: std::collections::HashMap<String, String>`
- `body: String`

**Methods:**

- `fn new<impl Into<String>>(method: HttpMethod, path: impl Trait) -> Self`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HttpServerRequest`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



