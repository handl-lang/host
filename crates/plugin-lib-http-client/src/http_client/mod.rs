mod http_client;

pub use http_client::{
    register_http_client_ops, HttpClient, HttpClientModule, HttpMethod, HttpRequest,
    HttpResponse, HTTP_CLIENT_OPS,
};

pub mod http_client_stub;

#[cfg(test)]
mod http_client_tests;
