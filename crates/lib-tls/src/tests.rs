use handl_ast_canonical::Expr;
use handl_vm_backend_rust_interpreter::Vm;
use handl_vm_common::{Module, ModuleHost};
use handl_vm_values_json::CanonicalContext;
use handl_vm_values_json::{Error, Host, NonFunctionValue};

use super::{TlsError, TlsModule};

fn s(v: &str) -> NonFunctionValue {
    NonFunctionValue::new(v.to_string())
}

fn n(v: f64) -> NonFunctionValue {
    NonFunctionValue::new(v)
}

fn a(values: &[&str]) -> NonFunctionValue {
    NonFunctionValue::new(serde_json::Value::Array(
        values
            .iter()
            .map(|v| serde_json::Value::String(v.to_string()))
            .collect(),
    ))
}

fn null() -> NonFunctionValue {
    NonFunctionValue::new(serde_json::Value::Null)
}

fn call(module: &TlsModule, name: &str, args: &[NonFunctionValue]) -> NonFunctionValue {
    let func = module.function(name).expect("function exists");
    let mut ctx = CanonicalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(
    module: &TlsModule,
    name: &str,
    args: &[NonFunctionValue],
) -> Result<NonFunctionValue, Error> {
    let func = module.function(name).expect("function exists");
    let mut ctx = CanonicalContext::new();
    func.call(args, &mut ctx)
}

// ── version ──────────────────────────────────────────────────────

#[test]
fn version() {
    let result = call(&TlsModule::default(), "version", &[]);
    let obj = result.inner();
    assert_eq!(obj["library"], "stub");
}

// ── generate_self_signed ─────────────────────────────────────────

#[test]
fn generate_self_signed_single_host() {
    let result = call(&TlsModule::default(), "generate_self_signed", &[s("localhost")]);
    let obj = result.inner();
    assert!(obj["cert"].as_str().unwrap().contains("BEGIN CERTIFICATE"));
    assert_eq!(obj["fingerprint"].as_str().unwrap().len(), 64);
}

#[test]
fn generate_self_signed_multiple_hosts() {
    let result = call(
        &TlsModule::default(),
        "generate_self_signed",
        &[a(&["localhost", "example.com"])],
    );
    let obj = result.inner();
    assert!(obj["cert"].as_str().unwrap().contains("BEGIN CERTIFICATE"));
    assert_eq!(obj["fingerprint"].as_str().unwrap().len(), 64);
}

#[test]
fn generate_self_signed_no_args_errors() {
    let result = call_err(&TlsModule::default(), "generate_self_signed", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn generate_self_signed_invalid_host_type_errors() {
    let result = call_err(
        &TlsModule::default(),
        "generate_self_signed",
        &[NonFunctionValue::new(42)],
    );
    assert!(result.is_err());
}

#[test]
fn generate_self_signed_empty_array_errors() {
    let result = call_err(&TlsModule::default(), "generate_self_signed", &[a(&[])]);
    assert!(result.is_err());
}

#[test]
fn generate_self_signed_array_with_non_string_errors() {
    let hosts = NonFunctionValue::new(serde_json::json!(["localhost", 123]));
    let result = call_err(&TlsModule::default(), "generate_self_signed", &[hosts]);
    assert!(result.is_err());
}

// ── parse_cert ───────────────────────────────────────────────────

#[test]
fn parse_cert_valid() {
    let result = call(&TlsModule::default(), "parse_cert", &[s("some-cert")]);
    let obj = result.inner();
    assert_eq!(obj["count"], 1);
    assert_eq!(obj["valid"], true);
}

#[test]
fn parse_cert_empty_pem() {
    let result = call(&TlsModule::default(), "parse_cert", &[s("")]);
    let obj = result.inner();
    assert_eq!(obj["count"], 0);
    assert_eq!(obj["valid"], false);
}

#[test]
fn parse_cert_invalid_pem() {
    let result = call_err(&TlsModule::default(), "parse_cert", &[s("invalid")]);
    assert!(result.is_err());
}

#[test]
fn parse_cert_no_args_errors() {
    let result = call_err(&TlsModule::default(), "parse_cert", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn parse_cert_non_string_errors() {
    let result = call_err(&TlsModule::default(), "parse_cert", &[n(42.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

// ── fingerprint ──────────────────────────────────────────────────

#[test]
fn fingerprint_valid() {
    let result = call(&TlsModule::default(), "fingerprint", &[s("some-cert")]);
    let fp = result.as_str().unwrap();
    assert_eq!(fp.len(), 64);
    assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn fingerprint_no_cert_errors() {
    let result = call_err(&TlsModule::default(), "fingerprint", &[s("")]);
    assert!(result.is_err());
}

#[test]
fn fingerprint_no_args_errors() {
    let result = call_err(&TlsModule::default(), "fingerprint", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn fingerprint_non_string_errors() {
    let result = call_err(&TlsModule::default(), "fingerprint", &[n(42.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn fingerprint_invalid_pem() {
    let result = call_err(&TlsModule::default(), "fingerprint", &[s("invalid")]);
    assert!(result.is_err());
}

// ── client_config ────────────────────────────────────────────────

#[test]
fn client_config() {
    let result = call(
        &TlsModule::default(),
        "client_config",
        &[a(&["root"]), s("cert"), s("key")],
    );
    assert!(result.as_number().is_some());
}

#[test]
fn client_config_null_cert_key() {
    let result = call(
        &TlsModule::default(),
        "client_config",
        &[a(&["root"]), null(), null()],
    );
    assert!(result.as_number().is_some());
}

#[test]
fn client_config_no_args_errors() {
    let result = call_err(&TlsModule::default(), "client_config", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn client_config_empty_roots_errors() {
    let result = call_err(&TlsModule::default(), "client_config", &[a(&[])]);
    assert!(result.is_err());
}

#[test]
fn client_config_roots_not_array_errors() {
    let result = call_err(&TlsModule::default(), "client_config", &[s("root")]);
    assert!(result.is_err());
}

#[test]
fn client_config_roots_with_non_string_errors() {
    let roots = NonFunctionValue::new(serde_json::json!(["root", 123]));
    let result = call_err(&TlsModule::default(), "client_config", &[roots]);
    assert!(result.is_err());
}

// ── server_config ────────────────────────────────────────────────

#[test]
fn server_config() {
    let result = call(
        &TlsModule::default(),
        "server_config",
        &[s("cert"), s("key")],
    );
    assert!(result.as_number().is_some());
}

#[test]
fn server_config_empty_cert_errors() {
    let result = call_err(&TlsModule::default(), "server_config", &[s(""), s("key")]);
    assert!(result.is_err());
}

#[test]
fn server_config_no_args_errors() {
    let result = call_err(&TlsModule::default(), "server_config", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

// ── connect / accept / read / write / close ──────────────────────

#[test]
fn connect() {
    let result = call(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), n(443.0), n(1.0)],
    );
    assert!(result.as_number().is_some());
}

#[test]
fn accept() {
    let result = call(
        &TlsModule::default(),
        "accept",
        &[s("0.0.0.0"), n(8443.0), n(1.0)],
    );
    assert!(result.as_number().is_some());
}

#[test]
fn read_write_round_trip() {
    let module = TlsModule::default();
    let conn = call(
        &module,
        "connect",
        &[s("localhost"), n(443.0), n(1.0)],
    );
    let conn_id = conn.as_number().unwrap() as usize;

    call(&module, "write", &[n(conn_id as f64), s("hello")]);
    let result = call(&module, "read", &[n(conn_id as f64), n(5.0)]);
    assert_eq!(result.as_str(), Some("hello"));
}

#[test]
fn close_conn() {
    let module = TlsModule::default();
    let conn = call(
        &module,
        "connect",
        &[s("localhost"), n(443.0), n(1.0)],
    );
    let conn_id = conn.as_number().unwrap() as usize;
    call(
        &module,
        "close_conn",
        &[n(conn_id as f64)],
    );
    call(&module, "close_listener", &[n(conn_id as f64)]);
}

#[test]
fn connect_no_args_errors() {
    let result = call_err(&TlsModule::default(), "connect", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn connect_service_error() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("error"), n(443.0), n(1.0)],
    );
    assert!(result.is_err());
}

#[test]
fn accept_no_args_errors() {
    let result = call_err(&TlsModule::default(), "accept", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn accept_service_error() {
    let result = call_err(
        &TlsModule::default(),
        "accept",
        &[s("error"), n(8443.0), n(1.0)],
    );
    assert!(result.is_err());
}

#[test]
fn read_no_args_errors() {
    let result = call_err(&TlsModule::default(), "read", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn read_empty_conn() {
    let conn = call(&TlsModule::default(), "connect", &[s("localhost"), n(443.0), n(1.0)]);
    let conn_id = conn.as_number().unwrap() as usize;
    let result = call(&TlsModule::default(), "read", &[n(conn_id as f64), n(10.0)]);
    assert_eq!(result.as_str(), Some(""));
}

#[test]
fn read_service_error() {
    let result = call_err(&TlsModule::default(), "read", &[n(0.0), n(5.0)]);
    assert!(result.is_err());
}

#[test]
fn write_no_args_errors() {
    let result = call_err(&TlsModule::default(), "write", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn write_service_error() {
    let result = call_err(&TlsModule::default(), "write", &[n(0.0), s("hello")]);
    assert!(result.is_err());
}

#[test]
fn close_conn_no_args_errors() {
    let result = call_err(&TlsModule::default(), "close_conn", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn close_conn_service_error() {
    let result = call_err(&TlsModule::default(), "close_conn", &[n(0.0)]);
    assert!(result.is_err());
}

#[test]
fn close_listener_no_args_errors() {
    let result = call_err(&TlsModule::default(), "close_listener", &[]);
    assert!(matches!(result, Err(Error::ArgumentCount { .. })));
}

#[test]
fn close_listener_service_error() {
    let result = call_err(&TlsModule::default(), "close_listener", &[n(0.0)]);
    assert!(result.is_err());
}

#[test]
fn connect_port_fractional_errors() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), n(443.5), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn connect_port_out_of_range_errors() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), n(70000.0), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn connect_port_non_number_errors() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), s("abc"), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn connect_config_id_non_number_errors() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), n(443.0), s("abc")],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn connect_negative_config_id_errors() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), n(443.0), n(-1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

// ── module resolution ────────────────────────────────────────────

#[test]
fn module_name() {
    assert_eq!(TlsModule::default().name(), "tls");
}

#[test]
fn module_resolves_functions() {
    let module = TlsModule::default();
    let names = [
        "version",
        "generate_self_signed",
        "parse_cert",
        "fingerprint",
        "client_config",
        "server_config",
        "connect",
        "accept",
        "read",
        "write",
        "close_conn",
        "close_listener",
    ];
    for name in names {
        assert!(module.function(name).is_some(), "{name} missing");
    }
    assert!(module.function("nope").is_none());
}

// ── VM integration ───────────────────────────────────────────────

fn eval_expr(expr: &Expr<NonFunctionValue>) -> NonFunctionValue {
    let host = ModuleHost::new().with(TlsModule::default());
    let vm = Vm::new(host);
    let mut ctx = CanonicalContext::new();
    vm.eval(expr, &mut ctx).expect("eval succeeds")
}

#[test]
fn vm_version() {
    let expr = Expr::call("tls.version", vec![]);
    let result = eval_expr(&expr);
    assert_eq!(result.inner()["library"], "stub");
}

#[test]
fn vm_generate_self_signed() {
    let expr = Expr::call(
        "tls.generate_self_signed",
        vec![Expr::value(s("localhost"))],
    );
    let result = eval_expr(&expr);
    assert!(result.inner()["cert"].as_str().is_some());
}

#[test]
fn vm_resolves_via_module_host() {
    let host = ModuleHost::new().with(TlsModule::default());
    assert!(host.function("tls.version").is_some());
    assert!(host.function("tls.connect").is_some());
}

// ── additional type/validation errors ────────────────────────────

#[test]
fn server_config_cert_non_string_errors() {
    let result = call_err(
        &TlsModule::default(),
        "server_config",
        &[NonFunctionValue::new(42), s("key")],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn server_config_key_non_string_errors() {
    let result = call_err(
        &TlsModule::default(),
        "server_config",
        &[s("cert"), NonFunctionValue::new(42)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn client_config_key_non_string_errors() {
    let result = call_err(
        &TlsModule::default(),
        "client_config",
        &[a(&["root"]), null(), NonFunctionValue::new(42)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn client_config_cert_non_string_errors() {
    let result = call_err(
        &TlsModule::default(),
        "client_config",
        &[a(&["root"]), NonFunctionValue::new(42), s("key")],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn connect_host_non_string_errors() {
    let result = call_err(&TlsModule::default(), "connect", &[n(42.0), n(443.0), n(1.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn connect_negative_port_errors() {
    let result = call_err(
        &TlsModule::default(),
        "connect",
        &[s("localhost"), n(-1.0), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn accept_addr_non_string_errors() {
    let result = call_err(&TlsModule::default(), "accept", &[n(42.0), n(8443.0), n(1.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn accept_port_non_number_errors() {
    let result = call_err(
        &TlsModule::default(),
        "accept",
        &[s("0.0.0.0"), s("abc"), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn accept_port_fractional_errors() {
    let result = call_err(
        &TlsModule::default(),
        "accept",
        &[s("0.0.0.0"), n(8443.5), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn accept_negative_port_errors() {
    let result = call_err(
        &TlsModule::default(),
        "accept",
        &[s("0.0.0.0"), n(-1.0), n(1.0)],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn accept_config_id_non_number_errors() {
    let result = call_err(
        &TlsModule::default(),
        "accept",
        &[s("0.0.0.0"), n(8443.0), s("abc")],
    );
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn read_len_non_number_errors() {
    let result = call_err(&TlsModule::default(), "read", &[n(1.0), s("abc")]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn read_conn_id_non_number_errors() {
    let result = call_err(&TlsModule::default(), "read", &[s("abc"), n(5.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn write_conn_id_non_number_errors() {
    let result = call_err(&TlsModule::default(), "write", &[s("abc"), s("hello")]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn write_data_non_string_errors() {
    let result = call_err(&TlsModule::default(), "write", &[n(1.0), n(42.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn close_conn_non_number_errors() {
    let result = call_err(&TlsModule::default(), "close_conn", &[s("abc")]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

#[test]
fn close_listener_non_number_errors() {
    let result = call_err(&TlsModule::default(), "close_listener", &[s("abc")]);
    assert!(matches!(result, Err(Error::TypeMismatch { .. })));
}

// ── error conversion ─────────────────────────────────────────────

#[test]
fn error_into_vm_error() {
    let err = TlsError::InvalidInput {
        message: "x".to_string(),
    };
    assert!(err.into_vm_error().to_string().contains("invalid input"));
    let err = TlsError::Generation {
        message: "x".to_string(),
    };
    assert!(err
        .into_vm_error()
        .to_string()
        .contains("certificate generation failed"));
    let err = TlsError::InvalidPem {
        message: "x".to_string(),
    };
    assert!(err.into_vm_error().to_string().contains("invalid PEM"));
    let err = TlsError::Fingerprint {
        message: "x".to_string(),
    };
    assert!(err.into_vm_error().to_string().contains("fingerprint failed"));
    let err = TlsError::InvalidConfig {
        message: "x".to_string(),
    };
    assert!(err.into_vm_error().to_string().contains("invalid config"));
    let err = TlsError::Io {
        message: "x".to_string(),
    };
    assert!(err.into_vm_error().to_string().contains("tls I/O error"));
}
