use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use serde_json::Value;

use super::error::TlsError;
use super::service::TlsService;
use super::stub::StubTls;

/// The `tls` runtime module.
///
/// `TlsModule` is backed by a [`TlsService`] implementation. The `stub`
/// backend is used by default and for unit tests; `handl-lib-tls-native`
/// supplies a real implementation.
pub struct TlsModule {
    version: VersionFn,
    generate_self_signed: GenerateSelfSignedFn,
    parse_cert: ParseCertFn,
    fingerprint: FingerprintFn,
    client_config: ClientConfigFn,
    server_config: ServerConfigFn,
    connect: ConnectFn,
    accept: AcceptFn,
    read: ReadFn,
    write: WriteFn,
    close_conn: CloseConnFn,
    close_listener: CloseListenerFn,
}

impl TlsModule {
    /// Creates a `TlsModule` with the given `TlsService`.
    pub fn new(service: impl TlsService + 'static) -> Self {
        let service = Arc::new(service);
        Self {
            version: VersionFn {
                service: service.clone(),
            },
            generate_self_signed: GenerateSelfSignedFn {
                service: service.clone(),
            },
            parse_cert: ParseCertFn {
                service: service.clone(),
            },
            fingerprint: FingerprintFn {
                service: service.clone(),
            },
            client_config: ClientConfigFn {
                service: service.clone(),
            },
            server_config: ServerConfigFn {
                service: service.clone(),
            },
            connect: ConnectFn {
                service: service.clone(),
            },
            accept: AcceptFn {
                service: service.clone(),
            },
            read: ReadFn {
                service: service.clone(),
            },
            write: WriteFn {
                service: service.clone(),
            },
            close_conn: CloseConnFn {
                service: service.clone(),
            },
            close_listener: CloseListenerFn { service },
        }
    }
}

impl Default for TlsModule {
    fn default() -> Self {
        Self::new(StubTls::new())
    }
}

impl Module for TlsModule {
    fn name(&self) -> &str {
        "tls"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "version" => Some(&self.version),
            "generate_self_signed" => Some(&self.generate_self_signed),
            "parse_cert" => Some(&self.parse_cert),
            "fingerprint" => Some(&self.fingerprint),
            "client_config" => Some(&self.client_config),
            "server_config" => Some(&self.server_config),
            "connect" => Some(&self.connect),
            "accept" => Some(&self.accept),
            "read" => Some(&self.read),
            "write" => Some(&self.write),
            "close_conn" => Some(&self.close_conn),
            "close_listener" => Some(&self.close_listener),
            _ => None,
        }
    }
}

fn require_arg<'a>(
    args: &'a [serde_json::Value],
    idx: usize,
) -> Result<&'a serde_json::Value, EvalError> {
    args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))
}

fn require_string_at(args: &[serde_json::Value], idx: usize) -> Result<&str, EvalError> {
    require_arg(args, idx)?
        .as_str()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
}

fn require_number_at(args: &[serde_json::Value], idx: usize) -> Result<f64, EvalError> {
    require_arg(args, idx)?
        .as_f64()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))
}

fn require_usize_at(args: &[serde_json::Value], idx: usize) -> Result<usize, EvalError> {
    let n = require_number_at(args, idx)?;
    if n < 0.0 || n.trunc() != n {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "non-negative integer", "non-integer number",
        )));
    }
    Ok(n as usize)
}

fn require_u16_at(args: &[serde_json::Value], idx: usize) -> Result<u16, EvalError> {
    let n = require_number_at(args, idx)?;
    if n < 0.0 || n > u16::MAX as f64 || n.trunc() != n {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "port number (0-65535)", "invalid port",
        )));
    }
    Ok(n as u16)
}

fn extract_hosts(value: &serde_json::Value) -> Result<Vec<String>, TlsError> {
    if let Some(s) = value.as_str() {
        return Ok(vec![s.to_string()]);
    }
    if let Some(arr) = value.as_array() {
        let mut hosts = Vec::with_capacity(arr.len());
        for item in arr {
            let s = item.as_str().ok_or_else(|| TlsError::InvalidInput {
                message: "hosts must be strings".to_string(),
            })?;
            hosts.push(s.to_string());
        }
        return Ok(hosts);
    }
    Err(TlsError::InvalidInput {
        message: "expected a string or an array of strings".to_string(),
    })
}

fn extract_string_array(value: &serde_json::Value) -> Result<Vec<String>, TlsError> {
    let arr = value.as_array().ok_or_else(|| TlsError::InvalidInput {
        message: "expected an array of strings".to_string(),
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_str().ok_or_else(|| TlsError::InvalidInput {
            message: "array items must be strings".to_string(),
        })?;
        out.push(s.to_string());
    }
    Ok(out)
}

fn optional_string_at(args: &[serde_json::Value], idx: usize) -> Result<Option<&str>, EvalError> {
    match args.get(idx) {
        None => Ok(None),
        Some(v) if v.is_null() => Ok(None),
        Some(v) => v
            .as_str()
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "string or null", "non-string non-null",
            )))
            .map(Some),
    }
}

// ── Function structs ─────────────────────────────────────────────

macro_rules! no_arg_fn {
    ($name:ident, $method:ident) => {
        struct $name {
            service: Arc<dyn TlsService>,
        }

        impl Function for $name {
            fn call(
                &self,
                _args: &[serde_json::Value],
                _ctx: &mut EvalContext,
            ) -> Result<serde_json::Value, EvalError> {
                let value = self.service.$method();
                Ok(serde_json::Value::from(value))
            }
        }
    };
}

no_arg_fn!(VersionFn, version);

struct GenerateSelfSignedFn {
    service: Arc<dyn TlsService>,
}

impl Function for GenerateSelfSignedFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let hosts = extract_hosts(require_arg(args, 0)?)?;
        let value = self.service.generate_self_signed(&hosts)?;
        Ok(serde_json::Value::from(value))
    }
}

struct ParseCertFn {
    service: Arc<dyn TlsService>,
}

impl Function for ParseCertFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let pem = require_string_at(args, 0)?;
        let value = self.service.parse_cert(pem)?;
        Ok(serde_json::Value::from(value))
    }
}

struct FingerprintFn {
    service: Arc<dyn TlsService>,
}

impl Function for FingerprintFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let pem = require_string_at(args, 0)?;
        let fp = self.service.fingerprint(pem)?;
        Ok(serde_json::Value::from(fp))
    }
}

struct ClientConfigFn {
    service: Arc<dyn TlsService>,
}

impl Function for ClientConfigFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let roots = extract_string_array(require_arg(args, 0)?)?;
        let cert = optional_string_at(args, 1)?;
        let key = optional_string_at(args, 2)?;
        let id = self.service.client_config(&roots, cert, key)?;
        Ok(serde_json::Value::from(id))
    }
}

struct ServerConfigFn {
    service: Arc<dyn TlsService>,
}

impl Function for ServerConfigFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let cert = require_string_at(args, 0)?;
        let key = require_string_at(args, 1)?;
        let id = self.service.server_config(cert, key)?;
        Ok(serde_json::Value::from(id))
    }
}

struct ConnectFn {
    service: Arc<dyn TlsService>,
}

impl Function for ConnectFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let host = require_string_at(args, 0)?;
        let port = require_u16_at(args, 1)?;
        let config_id = require_usize_at(args, 2)?;
        let id = self.service.connect(host, port, config_id)?;
        Ok(serde_json::Value::from(id))
    }
}

struct AcceptFn {
    service: Arc<dyn TlsService>,
}

impl Function for AcceptFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let addr = require_string_at(args, 0)?;
        let port = require_u16_at(args, 1)?;
        let config_id = require_usize_at(args, 2)?;
        let id = self.service.accept(addr, port, config_id)?;
        Ok(serde_json::Value::from(id))
    }
}

struct ReadFn {
    service: Arc<dyn TlsService>,
}

impl Function for ReadFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let conn_id = require_usize_at(args, 0)?;
        let len = require_usize_at(args, 1)?;
        let data = self.service.read(conn_id, len)?;
        Ok(serde_json::Value::from(data))
    }
}

struct WriteFn {
    service: Arc<dyn TlsService>,
}

impl Function for WriteFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let conn_id = require_usize_at(args, 0)?;
        let data = require_string_at(args, 1)?;
        self.service.write(conn_id, data)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct CloseConnFn {
    service: Arc<dyn TlsService>,
}

impl Function for CloseConnFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let conn_id = require_usize_at(args, 0)?;
        self.service.close_conn(conn_id)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct CloseListenerFn {
    service: Arc<dyn TlsService>,
}

impl Function for CloseListenerFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let listener_id = require_usize_at(args, 0)?;
        self.service.close_listener(listener_id)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `tls.hnl` — the TLS interface surface.
pub const TLS_OPS: &[&str] = &[
    "tls.version",
    "tls.generate_self_signed",
    "tls.parse_cert",
    "tls.fingerprint",
    "tls.client_config",
    "tls.server_config",
    "tls.connect",
    "tls.accept",
    "tls.read",
    "tls.write",
    "tls.close_conn",
    "tls.close_listener",
];

/// Build an [`OpRegistry`] for the TLS ops, registering Rust handls that
/// delegate to the given [`TlsService`] under their op names.
///
/// This is the shared registry builder used by both the interface crate
/// (with [`StubTls`] for testing) and the `handl-lib-tls-native` impl crate
/// (with `NativeTls` backed by rustls). Each op name maps to a `Function`
/// struct that parses its JSON arguments and dispatches to the service.
pub fn tls_op_registry(service: impl TlsService + 'static) -> OpRegistry {
    let service: Arc<dyn TlsService> = Arc::new(service);
    let mut registry = OpRegistry::new();
    registry.register("tls.version", Arc::new(VersionFn { service: service.clone() }));
    registry.register(
        "tls.generate_self_signed",
        Arc::new(GenerateSelfSignedFn { service: service.clone() }),
    );
    registry.register("tls.parse_cert", Arc::new(ParseCertFn { service: service.clone() }));
    registry.register("tls.fingerprint", Arc::new(FingerprintFn { service: service.clone() }));
    registry.register("tls.client_config", Arc::new(ClientConfigFn { service: service.clone() }));
    registry.register("tls.server_config", Arc::new(ServerConfigFn { service: service.clone() }));
    registry.register("tls.connect", Arc::new(ConnectFn { service: service.clone() }));
    registry.register("tls.accept", Arc::new(AcceptFn { service: service.clone() }));
    registry.register("tls.read", Arc::new(ReadFn { service: service.clone() }));
    registry.register("tls.write", Arc::new(WriteFn { service: service.clone() }));
    registry.register("tls.close_conn", Arc::new(CloseConnFn { service: service.clone() }));
    registry.register("tls.close_listener", Arc::new(CloseListenerFn { service }));
    registry
}
