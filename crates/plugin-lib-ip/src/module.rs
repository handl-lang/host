//! The `ip` handl runtime module.
//!
//! Exposes IP address, CIDR, and socket helpers callable from handl
//! expressions:
//!
//! - `ip.parse(string)` — parsed IP object
//! - `ip.format(ip)` — canonical string
//! - `ip.is_loopback(ip)`, `ip.is_multicast(ip)`, `ip.is_private(ip)`,
//!   `ip.is_link_local(ip)`, `ip.is_unspecified(ip)` — booleans
//! - `cidr.parse(string)`, `cidr.format(cidr)`, `cidr.contains(cidr, ip)`,
//!   `cidr.network(cidr)`, `cidr.broadcast(cidr)`
//! - `socket.parse(string)`, `socket.format(sock)`

use std::sync::{Arc, OnceLock};

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::HandlModule;
use serde_json::Value as JsonValue;

use crate::{Cidr, IpAddr, IpError, SocketAddr};

/// The handl source declaring the ip `op` interface — the single source
/// of truth for the ip API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const IP_HNL: &str = include_str!("../../../handl-packages/ip/ip.hnl");

// ---------------------------------------------------------------------------
// Op names per sub-module prefix
// ---------------------------------------------------------------------------

const IP_OPS: &[&str] = &[
    "ip.parse",
    "ip.format",
    "ip.is_loopback",
    "ip.is_multicast",
    "ip.is_private",
    "ip.is_link_local",
    "ip.is_unspecified",
];

const CIDR_OPS: &[&str] = &[
    "ip.cidr.parse",
    "ip.cidr.format",
    "ip.cidr.contains",
    "ip.cidr.network",
    "ip.cidr.broadcast",
];

const SOCKET_OPS: &[&str] = &["ip.socket.parse", "ip.socket.format"];

// ---------------------------------------------------------------------------
// HandlModule sub-modules (one per sub-module prefix)
// ---------------------------------------------------------------------------

/// The 3 [`HandlModule`] instances, one per ip sub-module prefix.
///
/// The ip module dispatches functions across multiple sub-module names
/// (ip, cidr, socket). Each sub-module's ops have unique local names
/// (the segment after the last `.`), so [`HandlModule::with_ops`] stores
/// them without collision. The shared [`OpRegistry`] is cloned (via
/// [`Arc::clone`]) into each sub-module.
struct IpModules {
    ip: HandlModule,
    cidr: HandlModule,
    socket: HandlModule,
}

/// Build the [`IpModules`] from the shared [`OpRegistry`] and the `.hnl`
/// source.
fn build_modules() -> IpModules {
    let registry = Arc::new(crate::ip_op_registry());
    IpModules {
        ip: HandlModule::new("ip", IP_HNL, &[]).with_ops(IP_OPS, Arc::clone(&registry)),
        cidr: HandlModule::new("cidr", IP_HNL, &[]).with_ops(CIDR_OPS, Arc::clone(&registry)),
        socket: HandlModule::new("socket", IP_HNL, &[]).with_ops(SOCKET_OPS, registry),
    }
}

/// A thread-safe lazy-initialized [`IpModules`].
static MODULES: OnceLock<IpModules> = OnceLock::new();

/// Get the shared [`IpModules`], initializing on first call.
fn modules() -> &'static IpModules {
    MODULES.get_or_init(build_modules)
}

/// The `ip` module — IP, CIDR, and socket primitives for handl expressions.
///
/// This is an **aggregator** module: it dispatches functions across
/// multiple sub-module prefixes (ip, cidr, socket). Each sub-module is
/// backed by a [`HandlModule`] with op functions dispatching to a shared
/// [`OpRegistry`]. The interface is declared in `ip.hnl` via `op`/`handl`
/// declarations; this struct delegates [`Module`] lookups to the shared
/// [`IpModules`].
#[derive(Clone, Copy)]
pub struct IpModule;

impl Module for IpModule {
    fn name(&self) -> &str {
        "ip"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        // The ip module dispatches on dotted function names like
        // "cidr.parse", "socket.format", etc. Split on the first dot to
        // get the sub-module prefix and the local function name, then
        // delegate to the appropriate HandlModule. Names without a dot
        // (e.g. "parse", "format") are delegated to the ip sub-module.
        let m = modules();
        match name.split_once('.') {
            Some((prefix, local)) => match prefix {
                "cidr" => m.cidr.function(local),
                "socket" => m.socket.function(local),
                _ => None,
            },
            None => m.ip.function(name),
        }
    }
}

fn ip_err(err: IpError) -> EvalError {
    EvalError::Other(err.to_string())
}

fn require_arg_count(args: &[serde_json::Value], expected: usize) -> Result<(), EvalError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            expected,
            args.len()
        )))
    }
}

fn require_one_string(args: &[serde_json::Value]) -> Result<String, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1,
        args.len(),
    )))?;
    arg.as_str()
        .map(|s| s.to_string())
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
}

fn bool_(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}

fn ip_to_json(ip: IpAddr) -> JsonValue {
    let (is_private, is_link_local) = match ip {
        IpAddr::V4(_) => (ip.is_private(), ip.is_link_local()),
        IpAddr::V6(_) => (false, false),
    };
    serde_json::json!({
        "version": match ip { IpAddr::V4(_) => 4, IpAddr::V6(_) => 6 },
        "address": ip.to_string(),
        "is_loopback": ip.is_loopback(),
        "is_multicast": ip.is_multicast(),
        "is_private": is_private,
        "is_link_local": is_link_local,
        "is_unspecified": ip.is_unspecified(),
    })
}

fn as_ip(value: &serde_json::Value) -> Result<IpAddr, EvalError> {
    if let Some(s) = value.as_str() {
        IpAddr::parse(s).map_err(ip_err)
    } else if let Some(obj) = value.as_object() {
        let s = obj
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "ip object with address", "object without address",
            )))?;
        IpAddr::parse(s).map_err(ip_err)
    } else {
        Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string or ip object", "non-string/non-object",
        )))
    }
}

fn as_cidr(value: &serde_json::Value) -> Result<Cidr, EvalError> {
    if let Some(s) = value.as_str() {
        Cidr::parse(s).map_err(ip_err)
    } else if let Some(obj) = value.as_object() {
        let s = obj
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "cidr object with address", "object without address",
            )))?;
        let prefix = obj
            .get("prefix")
            .and_then(|v| v.as_f64())
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "cidr object with prefix", "object without numeric prefix",
            )))?;
        if prefix < 0.0 || prefix.fract() != 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative integer prefix", "invalid prefix",
            )));
        }
        Cidr::parse(&format!("{}/{}", s, prefix as u8)).map_err(ip_err)
    } else {
        Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string or cidr object", "non-string/non-object",
        )))
    }
}

fn as_socket(value: &serde_json::Value) -> Result<SocketAddr, EvalError> {
    if let Some(s) = value.as_str() {
        SocketAddr::parse(s).map_err(ip_err)
    } else if let Some(obj) = value.as_object() {
        let ip_value = obj.get("ip").ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "socket object with ip", "object without ip",
        )))?;
        let ip = as_ip(&serde_json::Value::from(ip_value.clone()))?;
        let port = obj
            .get("port")
            .and_then(|v| v.as_f64())
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "socket object with numeric port", "object without port",
            )))?;
        if port < 0.0 || port.fract() != 0.0 || port > u16::MAX as f64 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "valid port number", "invalid port",
            )));
        }
        Ok(SocketAddr {
            ip,
            port: port as u16,
        })
    } else {
        Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string or socket object", "non-string/non-object",
        )))
    }
}

#[derive(Clone, Copy)]
pub struct IpParse;
impl Function for IpParse {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let ip = IpAddr::parse(&s).map_err(ip_err)?;
        Ok(serde_json::Value::from(ip_to_json(ip)))
    }
}

#[derive(Clone, Copy)]
pub struct IpFormat;
impl Function for IpFormat {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(serde_json::Value::from(ip.to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsLoopback;
impl Function for IpIsLoopback {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_loopback()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsMulticast;
impl Function for IpIsMulticast {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_multicast()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsPrivate;
impl Function for IpIsPrivate {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_private()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsLinkLocal;
impl Function for IpIsLinkLocal {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_link_local()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsUnspecified;
impl Function for IpIsUnspecified {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_unspecified()))
    }
}

#[derive(Clone, Copy)]
pub struct CidrParse;
impl Function for CidrParse {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let cidr = Cidr::parse(&s).map_err(ip_err)?;
        let (version, address, prefix) = match cidr {
            Cidr::V4(c) => (4, c.addr.to_string(), c.prefix),
            Cidr::V6(c) => (6, c.addr.to_string(), c.prefix),
        };
        Ok(serde_json::Value::from(serde_json::json!({
            "version": version,
            "address": address,
            "prefix": prefix as f64,
        })))
    }
}

#[derive(Clone, Copy)]
pub struct CidrFormat;
impl Function for CidrFormat {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let cidr = as_cidr(&args[0])?;
        Ok(serde_json::Value::from(cidr.to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct CidrContains;
impl Function for CidrContains {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let cidr = as_cidr(&args[0])?;
        let ip = as_ip(&args[1])?;
        Ok(bool_(cidr.contains(&ip)))
    }
}

#[derive(Clone, Copy)]
pub struct CidrNetwork;
impl Function for CidrNetwork {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let cidr = as_cidr(&args[0])?;
        Ok(serde_json::Value::from(cidr.network().to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct CidrBroadcast;
impl Function for CidrBroadcast {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let cidr = as_cidr(&args[0])?;
        Ok(serde_json::Value::from(cidr.broadcast().to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct SocketParse;
impl Function for SocketParse {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let sock = SocketAddr::parse(&s).map_err(ip_err)?;
        Ok(serde_json::Value::from(serde_json::json!({
            "ip": ip_to_json(sock.ip()),
            "port": sock.port() as f64,
        })))
    }
}

#[derive(Clone, Copy)]
pub struct SocketFormat;
impl Function for SocketFormat {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 1)?;
        let sock = as_socket(&args[0])?;
        Ok(serde_json::Value::from(sock.to_string()))
    }
}
