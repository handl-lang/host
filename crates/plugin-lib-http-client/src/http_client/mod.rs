mod http_client;

pub use http_client::{
    HTTP_CLIENT_OPS, HttpClient, HttpClientModule, HttpMethod, HttpRequest, HttpResponse,
    register_http_client_ops,
};

pub mod http_client_stub;

#[cfg(test)]
mod http_client_tests;
