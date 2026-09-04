//! # handl-lib-encoding
//!
//! Encoding abstractions for the handl runtime.
//!
//! Provides the `Encoding` trait and a registry for codecs such as
//! `base64` and `hex`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The encoding interface is declared in
//! `handl-packages/encoding/encoding.hnl`. The [`Encoding`] trait and
//! [`EncodingRegistry`] remain in Rust — they are implemented by concrete
//! encoding crates (e.g. `plugin-lib-base64`, `plugin-lib-hex`). The
//! [`EncodingLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with
//! the `.hnl` source, eliminating the hand-rolled `Plugin` trait
//! scaffolding.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the encoding interface — the single source
/// of truth for the encoding API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const ENCODING_HNL: &str = include_str!("../../../handl-packages/encoding/encoding.hnl");

/// Errors that can occur when encoding or decoding.
#[derive(Debug, Error)]
pub enum EncodingError {
    #[error("encoding error: {0}")]
    Message(String),
}

/// An encoding that can turn values into strings and back again.
pub trait Encoding: Send + Sync {
    /// Encode `value` according to this encoding.
    fn encode(&self, value: &Value) -> Result<Value, EncodingError>;

    /// Decode `value` from this encoding.
    fn decode(&self, value: &Value) -> Result<Value, EncodingError>;
}

/// A registry of named encodings.
#[derive(Default)]
pub struct EncodingRegistry {
    encodings: HashMap<String, Arc<dyn Encoding>>,
}

impl EncodingRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            encodings: HashMap::new(),
        }
    }

    /// Registers an encoding under a name.
    pub fn with(mut self, name: impl Into<String>, encoding: impl Encoding + 'static) -> Self {
        self.encodings.insert(name.into(), Arc::new(encoding));
        self
    }

    /// Returns the encoding registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&dyn Encoding> {
        self.encodings.get(name).map(|e| e.as_ref())
    }
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
