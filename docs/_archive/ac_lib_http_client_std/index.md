# handl_lib_http_client_std

# handl-lib-http-client-std

Concrete HTTP client implementation backed by `ureq`.

Provides [`StdHttpClient`] using the [`ureq`] crate.

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_http_client::HttpClientModule;
use handl_lib_http_client_std::StdHttpClient;

let host = ModuleHost::new()
    .with(HttpClientModule::new(StdHttpClient));
```

## Modules

### [`handl_lib_http_client_std`](handl_lib_http_client_std.md)

*1 module*

### [`http_client`](http_client.md)

*1 struct, 3 functions*

