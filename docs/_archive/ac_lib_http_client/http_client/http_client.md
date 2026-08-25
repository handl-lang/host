**handl_lib_http_client > http_client > http_client**

# Module: http_client::http_client

## Contents

**Macros**

- [`body_fn`](#body_fn)
- [`bodyless_fn`](#bodyless_fn)

**Structs**

- [`DeleteFn`](#deletefn)
- [`GetFn`](#getfn)
- [`HeadFn`](#headfn)
- [`HttpClientModule`](#httpclientmodule) - Module adapter for [`HttpClient`] — exposes `http.*` functions.
- [`HttpRequest`](#httprequest) - HTTP request descriptor.
- [`HttpResponse`](#httpresponse) - HTTP response.
- [`PatchFn`](#patchfn)
- [`PostFn`](#postfn)
- [`PutFn`](#putfn)
- [`RequestFn`](#requestfn)

**Enums**

- [`HttpMethod`](#httpmethod) - HTTP method enum.

**Functions**

- [`extract_headers`](#extract_headers)
- [`require_string_at`](#require_string_at)
- [`response_to_value`](#response_to_value)

**Traits**

- [`HttpClient`](#httpclient) - Abstract HTTP client service.

---

## handl_lib_http_client::http_client::http_client::DeleteFn

*Struct*

**Fields:**
- `client: Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::GetFn

*Struct*

**Fields:**
- `client: Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::HeadFn

*Struct*

**Fields:**
- `client: Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::HttpClient

*Trait*

Abstract HTTP client service.

Inspired by the [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API),
this trait uses a single [`request`] method with an [`HttpRequest`] descriptor,
giving full control over method, headers, body, and timeout.

Implementations:
- `handl-lib-http-client-std` — backed by `ureq`
- [`http_client_stub::StubHttpClient`] — canned responses for testing

**Methods:**

- `request`



## handl_lib_http_client::http_client::http_client::HttpClientModule

*Struct*

Module adapter for [`HttpClient`] — exposes `http.*` functions.

| Function | Args | Description |
|----------|------|-------------|
| `http.get(url)` | `(url)` or `(url, headers)` | GET request |
| `http.post(url, body)` | `(url, body)` or `(url, body, headers)` | POST request |
| `http.put(url, body)` | `(url, body)` or `(url, body, headers)` | PUT request |
| `http.patch(url, body)` | `(url, body)` or `(url, body, headers)` | PATCH request |
| `http.delete(url)` | `(url)` or `(url, headers)` | DELETE request |
| `http.head(url)` | `(url)` or `(url, headers)` | HEAD request |
| `http.request(method, url, body, headers)` | `(method, url, body, headers)` | Full control |

`headers` is a JSON object of header name to value.
`body` in `http.request` can be `null` for bodyless requests.

**Fields:**
- `get: GetFn`
- `post: PostFn`
- `put: PutFn`
- `patch: PatchFn`
- `delete: DeleteFn`
- `head: HeadFn`
- `request: RequestFn`

**Methods:**

- `fn new<impl HttpClient + 'static>(client: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_http_client::http_client::http_client::HttpMethod

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

**Traits:** Copy, Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HttpMethod`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &HttpMethod) -> bool`



## handl_lib_http_client::http_client::http_client::HttpRequest

*Struct*

HTTP request descriptor.

**Fields:**
- `method: HttpMethod`
- `url: String`
- `body: Option<String>`
- `headers: std::collections::HashMap<String, String>`
- `timeout_ms: Option<u64>`

**Methods:**

- `fn new<impl Into<String>>(method: HttpMethod, url: impl Trait) -> Self`
- `fn with_body<impl Into<String>>(self: Self, body: impl Trait) -> Self`
- `fn with_header<impl Into<String>, impl Into<String>>(self: Self, name: impl Trait, value: impl Trait) -> Self`
- `fn with_timeout(self: Self, ms: u64) -> Self`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HttpRequest`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_http_client::http_client::http_client::HttpResponse

*Struct*

HTTP response.

**Fields:**
- `status: u16`
- `status_text: String`
- `body: String`
- `headers: std::collections::HashMap<String, String>`
- `url: String`

**Methods:**

- `fn new<impl Into<String>>(status: u16, body: impl Trait) -> Self`
- `fn with_header<impl Into<String>, impl Into<String>>(self: Self, name: impl Trait, value: impl Trait) -> Self`
- `fn with_status_text<impl Into<String>>(self: Self, text: impl Trait) -> Self`
- `fn with_url<impl Into<String>>(self: Self, url: impl Trait) -> Self`
- `fn ok(self: &Self) -> bool` - Returns `true` if the status code is in the 2xx range.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HttpResponse`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_http_client::http_client::http_client::PatchFn

*Struct*

**Fields:**
- `client: Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::PostFn

*Struct*

**Fields:**
- `client: Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::PutFn

*Struct*

**Fields:**
- `client: Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::RequestFn

*Struct*

**Fields:**
- `client: std::sync::Arc<dyn HttpClient>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_http_client::http_client::http_client::body_fn

*Declarative Macro*

```rust
macro_rules! body_fn {
    ($name:ident, $method:expr) => { ... };
}
```



## handl_lib_http_client::http_client::http_client::bodyless_fn

*Declarative Macro*

```rust
macro_rules! bodyless_fn {
    ($name:ident, $method:expr) => { ... };
}
```



## handl_lib_http_client::http_client::http_client::extract_headers

*Function*

```rust
fn extract_headers(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<std::collections::HashMap<String, String>, ac_runtime_values::Error>
```



## handl_lib_http_client::http_client::http_client::require_string_at

*Function*

```rust
fn require_string_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_http_client::http_client::http_client::response_to_value

*Function*

```rust
fn response_to_value(resp: HttpResponse) -> ac_runtime_values::NonFunctionValue
```



