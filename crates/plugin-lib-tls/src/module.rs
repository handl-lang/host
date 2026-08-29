use std::sync::Arc;

use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};
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

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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
    args: &'a [NonFunctionValue],
    idx: usize,
) -> Result<&'a NonFunctionValue, Error> {
    args.get(idx).ok_or(Error::ArgumentCount {
        expected: idx + 1,
        got: args.len(),
    })
}

fn require_string_at(args: &[NonFunctionValue], idx: usize) -> Result<&str, Error> {
    require_arg(args, idx)?.as_str().ok_or(Error::TypeMismatch {
        expected: "string",
        got: "non-string",
    })
}

fn require_number_at(args: &[NonFunctionValue], idx: usize) -> Result<f64, Error> {
    require_arg(args, idx)?
        .as_number()
        .ok_or(Error::TypeMismatch {
            expected: "number",
            got: "non-number",
        })
}

fn require_usize_at(args: &[NonFunctionValue], idx: usize) -> Result<usize, Error> {
    let n = require_number_at(args, idx)?;
    if n < 0.0 || n.trunc() != n {
        return Err(Error::TypeMismatch {
            expected: "non-negative integer",
            got: "non-integer number",
        });
    }
    Ok(n as usize)
}

fn require_u16_at(args: &[NonFunctionValue], idx: usize) -> Result<u16, Error> {
    let n = require_number_at(args, idx)?;
    if n < 0.0 || n > u16::MAX as f64 || n.trunc() != n {
        return Err(Error::TypeMismatch {
            expected: "port number (0-65535)",
            got: "invalid port",
        });
    }
    Ok(n as u16)
}

fn extract_hosts(value: &NonFunctionValue) -> Result<Vec<String>, TlsError> {
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

fn extract_string_array(value: &NonFunctionValue) -> Result<Vec<String>, TlsError> {
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

fn optional_string_at(args: &[NonFunctionValue], idx: usize) -> Result<Option<&str>, Error> {
    match args.get(idx) {
        None => Ok(None),
        Some(v) if v.is_null() => Ok(None),
        Some(v) => v
            .as_str()
            .ok_or(Error::TypeMismatch {
                expected: "string or null",
                got: "non-string non-null",
            })
            .map(Some),
    }
}

// ── Function structs ─────────────────────────────────────────────

macro_rules! no_arg_fn {
    ($name:ident, $method:ident) => {
        struct $name {
            service: Arc<dyn TlsService>,
        }

        impl Function<NonFunctionValue> for $name {
            fn call(
                &self,
                _args: &[NonFunctionValue],
                _ctx: &mut dyn Context<NonFunctionValue>,
            ) -> Result<NonFunctionValue, Error> {
                let value = self.service.$method();
                Ok(NonFunctionValue::new(value))
            }
        }
    };
}

no_arg_fn!(VersionFn, version);

struct GenerateSelfSignedFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for GenerateSelfSignedFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let hosts = extract_hosts(require_arg(args, 0)?)?;
        let value = self.service.generate_self_signed(&hosts)?;
        Ok(NonFunctionValue::new(value))
    }
}

struct ParseCertFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for ParseCertFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let pem = require_string_at(args, 0)?;
        let value = self.service.parse_cert(pem)?;
        Ok(NonFunctionValue::new(value))
    }
}

struct FingerprintFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for FingerprintFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let pem = require_string_at(args, 0)?;
        let fp = self.service.fingerprint(pem)?;
        Ok(NonFunctionValue::new(fp))
    }
}

struct ClientConfigFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for ClientConfigFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let roots = extract_string_array(require_arg(args, 0)?)?;
        let cert = optional_string_at(args, 1)?;
        let key = optional_string_at(args, 2)?;
        let id = self.service.client_config(&roots, cert, key)?;
        Ok(NonFunctionValue::new(id))
    }
}

struct ServerConfigFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for ServerConfigFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let cert = require_string_at(args, 0)?;
        let key = require_string_at(args, 1)?;
        let id = self.service.server_config(cert, key)?;
        Ok(NonFunctionValue::new(id))
    }
}

struct ConnectFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for ConnectFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let host = require_string_at(args, 0)?;
        let port = require_u16_at(args, 1)?;
        let config_id = require_usize_at(args, 2)?;
        let id = self.service.connect(host, port, config_id)?;
        Ok(NonFunctionValue::new(id))
    }
}

struct AcceptFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for AcceptFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let addr = require_string_at(args, 0)?;
        let port = require_u16_at(args, 1)?;
        let config_id = require_usize_at(args, 2)?;
        let id = self.service.accept(addr, port, config_id)?;
        Ok(NonFunctionValue::new(id))
    }
}

struct ReadFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for ReadFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let conn_id = require_usize_at(args, 0)?;
        let len = require_usize_at(args, 1)?;
        let data = self.service.read(conn_id, len)?;
        Ok(NonFunctionValue::new(data))
    }
}

struct WriteFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for WriteFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let conn_id = require_usize_at(args, 0)?;
        let data = require_string_at(args, 1)?;
        self.service.write(conn_id, data)?;
        Ok(NonFunctionValue::new(Value::Null))
    }
}

struct CloseConnFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for CloseConnFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let conn_id = require_usize_at(args, 0)?;
        self.service.close_conn(conn_id)?;
        Ok(NonFunctionValue::new(Value::Null))
    }
}

struct CloseListenerFn {
    service: Arc<dyn TlsService>,
}

impl Function<NonFunctionValue> for CloseListenerFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let listener_id = require_usize_at(args, 0)?;
        self.service.close_listener(listener_id)?;
        Ok(NonFunctionValue::new(Value::Null))
    }
}
