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

use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;

use crate::{Cidr, IpAddr, IpError, SocketAddr};

/// The `ip` module — IP, CIDR, and socket primitives for handl expressions.
#[derive(Clone, Copy)]
pub struct IpModule;

impl Module for IpModule {
    fn name(&self) -> &str {
        "ip"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "parse" => Some(&IpParse),
            "format" => Some(&IpFormat),
            "is_loopback" => Some(&IpIsLoopback),
            "is_multicast" => Some(&IpIsMulticast),
            "is_private" => Some(&IpIsPrivate),
            "is_link_local" => Some(&IpIsLinkLocal),
            "is_unspecified" => Some(&IpIsUnspecified),
            "cidr.parse" => Some(&CidrParse),
            "cidr.format" => Some(&CidrFormat),
            "cidr.contains" => Some(&CidrContains),
            "cidr.network" => Some(&CidrNetwork),
            "cidr.broadcast" => Some(&CidrBroadcast),
            "socket.parse" => Some(&SocketParse),
            "socket.format" => Some(&SocketFormat),
            _ => None,
        }
    }
}

fn ip_err(err: IpError) -> Error {
    Error::PrimitiveError(err.to_string())
}

fn require_arg_count(args: &[NonFunctionValue], expected: usize) -> Result<(), Error> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(Error::ArgumentCount {
            expected,
            got: args.len(),
        })
    }
}

fn require_one_string(args: &[NonFunctionValue]) -> Result<String, Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: args.len(),
    })?;
    arg.as_str()
        .map(|s| s.to_string())
        .ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })
}

fn bool_(value: bool) -> NonFunctionValue {
    NonFunctionValue::new(value)
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

fn as_ip(value: &NonFunctionValue) -> Result<IpAddr, Error> {
    if let Some(s) = value.as_str() {
        IpAddr::parse(s).map_err(ip_err)
    } else if let Some(obj) = value.as_object() {
        let s = obj
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or(Error::TypeMismatch {
                expected: "ip object with address",
                got: "object without address",
            })?;
        IpAddr::parse(s).map_err(ip_err)
    } else {
        Err(Error::TypeMismatch {
            expected: "string or ip object",
            got: "non-string/non-object",
        })
    }
}

fn as_cidr(value: &NonFunctionValue) -> Result<Cidr, Error> {
    if let Some(s) = value.as_str() {
        Cidr::parse(s).map_err(ip_err)
    } else if let Some(obj) = value.as_object() {
        let s = obj
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or(Error::TypeMismatch {
                expected: "cidr object with address",
                got: "object without address",
            })?;
        let prefix = obj
            .get("prefix")
            .and_then(|v| v.as_f64())
            .ok_or(Error::TypeMismatch {
                expected: "cidr object with prefix",
                got: "object without numeric prefix",
            })?;
        if prefix < 0.0 || prefix.fract() != 0.0 {
            return Err(Error::TypeMismatch {
                expected: "non-negative integer prefix",
                got: "invalid prefix",
            });
        }
        Cidr::parse(&format!("{}/{}", s, prefix as u8)).map_err(ip_err)
    } else {
        Err(Error::TypeMismatch {
            expected: "string or cidr object",
            got: "non-string/non-object",
        })
    }
}

fn as_socket(value: &NonFunctionValue) -> Result<SocketAddr, Error> {
    if let Some(s) = value.as_str() {
        SocketAddr::parse(s).map_err(ip_err)
    } else if let Some(obj) = value.as_object() {
        let ip_value = obj.get("ip").ok_or(Error::TypeMismatch {
            expected: "socket object with ip",
            got: "object without ip",
        })?;
        let ip = as_ip(&NonFunctionValue::new(ip_value.clone()))?;
        let port = obj
            .get("port")
            .and_then(|v| v.as_f64())
            .ok_or(Error::TypeMismatch {
                expected: "socket object with numeric port",
                got: "object without port",
            })?;
        if port < 0.0 || port.fract() != 0.0 || port > u16::MAX as f64 {
            return Err(Error::TypeMismatch {
                expected: "valid port number",
                got: "invalid port",
            });
        }
        Ok(SocketAddr {
            ip,
            port: port as u16,
        })
    } else {
        Err(Error::TypeMismatch {
            expected: "string or socket object",
            got: "non-string/non-object",
        })
    }
}

#[derive(Clone, Copy)]
pub struct IpParse;
impl Function<NonFunctionValue> for IpParse {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let ip = IpAddr::parse(&s).map_err(ip_err)?;
        Ok(NonFunctionValue::new(ip_to_json(ip)))
    }
}

#[derive(Clone, Copy)]
pub struct IpFormat;
impl Function<NonFunctionValue> for IpFormat {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(NonFunctionValue::new(ip.to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsLoopback;
impl Function<NonFunctionValue> for IpIsLoopback {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_loopback()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsMulticast;
impl Function<NonFunctionValue> for IpIsMulticast {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_multicast()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsPrivate;
impl Function<NonFunctionValue> for IpIsPrivate {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_private()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsLinkLocal;
impl Function<NonFunctionValue> for IpIsLinkLocal {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_link_local()))
    }
}

#[derive(Clone, Copy)]
pub struct IpIsUnspecified;
impl Function<NonFunctionValue> for IpIsUnspecified {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let ip = as_ip(&args[0])?;
        Ok(bool_(ip.is_unspecified()))
    }
}

#[derive(Clone, Copy)]
pub struct CidrParse;
impl Function<NonFunctionValue> for CidrParse {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let cidr = Cidr::parse(&s).map_err(ip_err)?;
        let (version, address, prefix) = match cidr {
            Cidr::V4(c) => (4, c.addr.to_string(), c.prefix),
            Cidr::V6(c) => (6, c.addr.to_string(), c.prefix),
        };
        Ok(NonFunctionValue::new(serde_json::json!({
            "version": version,
            "address": address,
            "prefix": prefix as f64,
        })))
    }
}

#[derive(Clone, Copy)]
pub struct CidrFormat;
impl Function<NonFunctionValue> for CidrFormat {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let cidr = as_cidr(&args[0])?;
        Ok(NonFunctionValue::new(cidr.to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct CidrContains;
impl Function<NonFunctionValue> for CidrContains {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 2)?;
        let cidr = as_cidr(&args[0])?;
        let ip = as_ip(&args[1])?;
        Ok(bool_(cidr.contains(&ip)))
    }
}

#[derive(Clone, Copy)]
pub struct CidrNetwork;
impl Function<NonFunctionValue> for CidrNetwork {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let cidr = as_cidr(&args[0])?;
        Ok(NonFunctionValue::new(cidr.network().to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct CidrBroadcast;
impl Function<NonFunctionValue> for CidrBroadcast {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let cidr = as_cidr(&args[0])?;
        Ok(NonFunctionValue::new(cidr.broadcast().to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct SocketParse;
impl Function<NonFunctionValue> for SocketParse {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let sock = SocketAddr::parse(&s).map_err(ip_err)?;
        Ok(NonFunctionValue::new(serde_json::json!({
            "ip": ip_to_json(sock.ip()),
            "port": sock.port() as f64,
        })))
    }
}

#[derive(Clone, Copy)]
pub struct SocketFormat;
impl Function<NonFunctionValue> for SocketFormat {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 1)?;
        let sock = as_socket(&args[0])?;
        Ok(NonFunctionValue::new(sock.to_string()))
    }
}
