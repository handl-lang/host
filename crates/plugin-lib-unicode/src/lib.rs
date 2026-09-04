//! # handl-lib-unicode
//!
//! Unicode primitives for the handl language toolchain and runtime.
//!
//! Provides a focused, compiler-friendly API over Unicode data:
//!
//! - [`decode`] — validate and decode UTF-8/UTF-16/UTF-32 byte streams into
//!   `char` iterators, with byte offsets for diagnostic positions.
//! - [`ident`] — identifier validation using `XID_Start` / `XID_Continue`
//!   (UAX #31), suitable for lexers and parsers.
//!
//! Also exposes the `unicode` handl runtime module via [`UnicodeModule`].
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The unicode interface is declared in
//! `handl-packages/kit-stdlib/unicode/unicode.hnl` via `op` declarations, and the
//! Rust unicode logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`UnicodeLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry.
//!
//! The [`UnicodeModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

pub mod decode;
pub mod ident;
pub mod module;

pub use decode::{DecodeError, Utf8Iterator, decode_utf8};
pub use ident::{is_identifier, is_identifier_continue, is_identifier_start};
pub use module::UnicodeModule;

/// The handl source declaring the unicode `op` interface — the single source
/// of truth for the unicode API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const UNICODE_HNL: &str = include_str!("../../../handl-packages/kit-stdlib/unicode/unicode.hnl");

/// The op names declared in `unicode.hnl`.
const UNICODE_OPS: &[&str] = &[
    "unicode.is_valid_identifier",
    "unicode.is_xid_start",
    "unicode.is_xid_continue",
    "unicode.code_points",
    "unicode.code_point_at",
    "unicode.decode",
];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for unicode, registering the Rust handls under
/// their op names.
pub fn unicode_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("unicode.is_valid_identifier", Arc::new(module::IsValidIdentifier));
    registry.register("unicode.is_xid_start", Arc::new(module::IsXidStart));
    registry.register("unicode.is_xid_continue", Arc::new(module::IsXidContinue));
    registry.register("unicode.code_points", Arc::new(module::CodePoints));
    registry.register("unicode.code_point_at", Arc::new(module::CodePointAt));
    registry.register("unicode.decode", Arc::new(module::Decode));
    registry
}

// ---------------------------------------------------------------------------
// UnicodeLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `unicode` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct UnicodeLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl UnicodeLibraryPlugin {
    /// Create a new `UnicodeLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(unicode_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "unicode",
                "lib.unicode",
                "lib-unicode",
                UNICODE_HNL,
                &[],
            )
            .with_ops(UNICODE_OPS, registry),
        }
    }
}

impl Default for UnicodeLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for UnicodeLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for unicode with the op registry.
pub fn unicode_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(unicode_op_registry());
    HandlLibraryPlugin::new_with_caps("unicode", "lib.unicode", "lib-unicode", UNICODE_HNL, &[])
        .with_ops(UNICODE_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`UnicodeLibraryPlugin`].
pub use UnicodeLibraryPlugin as LibraryPlugin;
