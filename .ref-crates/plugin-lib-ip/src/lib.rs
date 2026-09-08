//! # handl-lib-ip
//!
//! IP address, CIDR, and socket utilities for the handl runtime.
//!
//! Provides ergonomic, zero-allocation-by-default wrappers around IPv4,
//! IPv6, CIDR prefixes, and socket addresses, with strict validation and
//! lossless round-trip parsing.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The ip interface is declared in
//! `handl-packages/ip/ip.hnl` via `op` declarations, and the Rust
//! IP/CIDR/socket logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`IpLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`IpModule`] re-export is preserved for backward compatibility.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_lib_ip::{IpAddr, Ipv4, Ipv6, Cidr, SocketAddr};
//!
//! let v4 = Ipv4::parse("192.168.1.1")?;
//! let v6 = Ipv6::parse("::1")?;
//! let cidr = Cidr::parse("10.0.0.0/8")?;
//! let sock = SocketAddr::parse("[::1]:8080")?;
//! ```

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

mod addr;
mod cidr;
mod error;
mod module;
mod socket;

#[cfg(test)]
mod tests;

pub use addr::{IpAddr, Ipv4, Ipv6};
pub use cidr::{Cidr, Cidr4, Cidr6};
pub use error::IpError;
pub use module::{
    CidrBroadcast, CidrContains, CidrFormat, CidrNetwork, CidrParse, IpFormat, IpIsLinkLocal,
    IpIsLoopback, IpIsMulticast, IpIsPrivate, IpIsUnspecified, IpModule, IpParse, SocketFormat,
    SocketParse,
};
pub use socket::SocketAddr;

/// The handl source declaring the ip `op` interface — the single source
/// of truth for the ip API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const IP_HNL: &str = include_str!("../../../handl-packages/ip/ip.hnl");

/// The op names declared in `ip.hnl`.
const IP_OPS: &[&str] = &[
    "ip.parse",
    "ip.format",
    "ip.is_loopback",
    "ip.is_multicast",
    "ip.is_private",
    "ip.is_link_local",
    "ip.is_unspecified",
    "ip.cidr.parse",
    "ip.cidr.format",
    "ip.cidr.contains",
    "ip.cidr.network",
    "ip.cidr.broadcast",
    "ip.socket.parse",
    "ip.socket.format",
];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for ip, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"ip.parse"` → [`IpParse`] (no runtime checker; axioms trusted)
/// - `"ip.format"` → [`IpFormat`] (no runtime checker; axioms trusted)
/// - `"ip.is_loopback"` → [`IpIsLoopback`] (no runtime checker; axioms trusted)
/// - `"ip.is_multicast"` → [`IpIsMulticast`] (no runtime checker; axioms trusted)
/// - `"ip.is_private"` → [`IpIsPrivate`] (no runtime checker; axioms trusted)
/// - `"ip.is_link_local"` → [`IpIsLinkLocal`] (no runtime checker; axioms trusted)
/// - `"ip.is_unspecified"` → [`IpIsUnspecified`] (no runtime checker; axioms trusted)
/// - `"ip.cidr.parse"` → [`CidrParse`] (no runtime checker; axioms trusted)
/// - `"ip.cidr.format"` → [`CidrFormat`] (no runtime checker; axioms trusted)
/// - `"ip.cidr.contains"` → [`CidrContains`] (no runtime checker; axioms trusted)
/// - `"ip.cidr.network"` → [`CidrNetwork`] (no runtime checker; axioms trusted)
/// - `"ip.cidr.broadcast"` → [`CidrBroadcast`] (no runtime checker; axioms trusted)
/// - `"ip.socket.parse"` → [`SocketParse`] (no runtime checker; axioms trusted)
/// - `"ip.socket.format"` → [`SocketFormat`] (no runtime checker; axioms trusted)
pub fn ip_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("ip.parse", Arc::new(IpParse));
    registry.register("ip.format", Arc::new(IpFormat));
    registry.register("ip.is_loopback", Arc::new(IpIsLoopback));
    registry.register("ip.is_multicast", Arc::new(IpIsMulticast));
    registry.register("ip.is_private", Arc::new(IpIsPrivate));
    registry.register("ip.is_link_local", Arc::new(IpIsLinkLocal));
    registry.register("ip.is_unspecified", Arc::new(IpIsUnspecified));
    registry.register("ip.cidr.parse", Arc::new(CidrParse));
    registry.register("ip.cidr.format", Arc::new(CidrFormat));
    registry.register("ip.cidr.contains", Arc::new(CidrContains));
    registry.register("ip.cidr.network", Arc::new(CidrNetwork));
    registry.register("ip.cidr.broadcast", Arc::new(CidrBroadcast));
    registry.register("ip.socket.parse", Arc::new(SocketParse));
    registry.register("ip.socket.format", Arc::new(SocketFormat));
    registry
}

// ---------------------------------------------------------------------------
// IpLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `ip` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The ip interface is declared in `ip.hnl` via `op`/`handl`
/// declarations; the Rust IP/CIDR/socket handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct IpLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl IpLibraryPlugin {
    /// Create a new `IpLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(ip_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("ip", "lib.ip", "lib-ip", IP_HNL, &[])
                .with_ops(IP_OPS, registry),
        }
    }
}

impl Default for IpLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for IpLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for ip with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`IpLibraryPlugin`].
pub fn ip_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(ip_op_registry());
    HandlLibraryPlugin::new_with_caps("ip", "lib.ip", "lib-ip", IP_HNL, &[])
        .with_ops(IP_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`IpLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_ip::LibraryPlugin`.
pub use IpLibraryPlugin as LibraryPlugin;
