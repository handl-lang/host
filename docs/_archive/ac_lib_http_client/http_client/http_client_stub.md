**handl_lib_http_client > http_client > http_client_stub**

# Module: http_client::http_client_stub

## Contents

**Structs**

- [`StubHttpClient`](#stubhttpclient) - An HTTP client that returns canned responses by URL.

---

## handl_lib_http_client::http_client::http_client_stub::StubHttpClient

*Struct*

An HTTP client that returns canned responses by URL.

**Fields:**
- `responses: std::sync::Mutex<std::collections::HashMap<String, crate::http_client::HttpResponse>>`

**Methods:**

- `fn new() -> Self`
- `fn with<impl Into<String>>(url: impl Trait, response: HttpResponse) -> Self`

**Trait Implementations:**

- **HttpClient**
  - `fn request(self: &Self, req: &HttpRequest) -> Result<HttpResponse, HttpError>`
- **Default**
  - `fn default() -> StubHttpClient`



