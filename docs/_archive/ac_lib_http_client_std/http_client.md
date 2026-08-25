**handl_lib_http_client_std > http_client**

# Module: http_client

## Contents

**Structs**

- [`StdHttpClient`](#stdhttpclient) - HTTP client implementation backed by [`ureq`].

**Functions**

- [`build_request`](#build_request)
- [`extract_headers`](#extract_headers)
- [`into_http_response`](#into_http_response)

---

## handl_lib_http_client_std::http_client::StdHttpClient

*Struct*

HTTP client implementation backed by [`ureq`].

**Unit Struct**

**Trait Implementations:**

- **Default**
  - `fn default() -> StdHttpClient`
- **HttpClient**
  - `fn request(self: &Self, req: &HttpRequest) -> Result<HttpResponse, HttpError>`



## handl_lib_http_client_std::http_client::build_request

*Function*

```rust
fn build_request(req: &handl_lib_http_client::HttpRequest) -> ureq::Request
```



## handl_lib_http_client_std::http_client::extract_headers

*Function*

```rust
fn extract_headers(resp: &ureq::Response) -> std::collections::HashMap<String, String>
```



## handl_lib_http_client_std::http_client::into_http_response

*Function*

```rust
fn into_http_response(resp: ureq::Response) -> Result<handl_lib_http_client::HttpResponse, handl_lib_http_client::HttpError>
```



