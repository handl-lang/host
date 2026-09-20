//! # handl-lib-encoding
//!
//! Encoding abstractions for the handl runtime.
//!
//! Provides `Encoder`, `Decoder`, `Encoding`, and `EncodingRegistry`
//! abstractions shared by the `handl-lib-*` codec crates. The generic
//! [`Encoder`]/[`Decoder`] traits support typed codecs (e.g.
//! `Decoder<Bytes, Frame>` for a WebSocket frame codec); the concrete
//! [`Encoding`] trait covers the `Value`-to-`Value` codecs such as
//! `base64` and `hex`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The encoding interface is declared in
//! `handl-packages/encoding/encoding.hnl`. The [`Encoder`], [`Decoder`],
//! [`Encoding`], and [`EncodingRegistry`] traits remain in Rust — they
//! are implemented by concrete encoding crates (e.g. base64, hex). The
//! [`EncodingLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with
//! the `.hnl` source, eliminating the hand-rolled `Plugin` trait
//! scaffolding.

pub mod encoding;
pub mod error;
pub mod traits;

pub use encoding::{Encoding, EncodingRegistry};
pub use error::EncodingError;
pub use traits::{Decoder, Encoder};

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the encoding interface — the single source
/// of truth for the encoding API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const ENCODING_HNL: &str = include_str!("../../../handl-packages/encoding/encoding.hnl");

/// Encode a value using the provided encoder.
pub fn encode<T, E>(value: &T, encoder: &E) -> Result<E::Output, EncodingError>
where
    E: Encoder<T>,
{
    encoder.encode(value)
}

/// Decode input using the provided decoder.
pub fn decode<I, T, D>(input: &I, decoder: &D) -> Result<T, EncodingError>
where
    I: ?Sized,
    D: Decoder<I, T>,
{
    decoder.decode(input)
}

// ---------------------------------------------------------------------------
// EncodingLibraryPlugin (wraps HandlLibraryPlugin)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `encoding` interface via the
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The encoding interface is declared in `encoding.hnl`; the trait
/// definitions remain in Rust. The generic loader handles module
/// registration.
pub struct EncodingLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl EncodingLibraryPlugin {
    /// Create a new `EncodingLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "encoding",
                "lib.encoding",
                "lib-encoding",
                ENCODING_HNL,
                &[],
            ),
        }
    }
}

impl Default for EncodingLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for EncodingLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for encoding.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`EncodingLibraryPlugin`].
pub fn encoding_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new_with_caps(
        "encoding",
        "lib.encoding",
        "lib-encoding",
        ENCODING_HNL,
        &[],
    )
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`EncodingLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_encoding::LibraryPlugin`.
pub use EncodingLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::Plugin;

    #[test]
    fn plugin_id_and_name() {
        let plugin = EncodingLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.encoding");
        assert_eq!(plugin.name(), "lib-encoding");
    }

    #[test]
    fn plugin_has_capabilities() {
        let plugin = EncodingLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        let names: Vec<&str> = caps.iter().map(|c| c.as_str()).collect();
        assert!(names.contains(&"encoding.encode"));
        assert!(names.contains(&"encoding.decode"));
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = encoding_handl_plugin();
        assert_eq!(plugin.id(), "lib.encoding");
        assert_eq!(plugin.name(), "lib-encoding");
    }
}
