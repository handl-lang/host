use serde_json::{Value, json};
use std::sync::Arc;

use handl_platform::{EvalContext, Module};

use crate::http_method::HttpMethod;
use crate::http_server::{HttpServer, HttpServerModule, StubHttpServer};
use crate::request::HttpServerRequest;
use crate::response::HttpServerResponse;

fn s(v: &str) -> Value {
    json!(v)
}

fn call(module: &dyn Module, name: &str, args: &[Value]) -> Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

#[test]
fn module_name() {
    let module: HttpServerModule<fn() -> Arc<dyn HttpServer>> =
        HttpServerModule::new(|| Arc::new(StubHttpServer::new()));
    assert_eq!(module.name(), "http_server");
}

#[test]
fn new_returns_id() {
    let module: HttpServerModule<fn() -> Arc<dyn HttpServer>> =
        HttpServerModule::new(|| Arc::new(StubHttpServer::new()));
    let result = call(&module, "new", &[]);
    assert!(result.as_f64().is_some());
}

#[test]
fn start_stop_lifecycle() {
    let module: HttpServerModule<fn() -> Arc<dyn HttpServer>> =
        HttpServerModule::new(|| Arc::new(StubHttpServer::new()));
    let id = call(&module, "new", &[]);
    call(&module, "start", &[id.clone(), s("127.0.0.1:0")]);
    call(&module, "stop", &[id]);
}

#[test]
fn stub_route_and_response() {
    let server = StubHttpServer::new();
    server
        .route(
            HttpMethod::Get,
            "/hello",
            Box::new(|_| HttpServerResponse::new(200, "hi")),
        )
        .unwrap();
    let resp = server
        .response_for(HttpServerRequest::new(HttpMethod::Get, "/hello"))
        .expect("route matched");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "hi");
}

#[test]
fn stub_not_found() {
    let server = StubHttpServer::new();
    assert!(
        server
            .response_for(HttpServerRequest::new(HttpMethod::Get, "/missing"))
            .is_none()
    );
}
