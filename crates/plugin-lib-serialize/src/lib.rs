//! # handl-lib-serialize
//!
//! Format-agnostic serialization framework.
//!
//! Provides `Encoder`, `Decoder`, `serialize`, and `deserialize` abstractions
//! shared by the `handl-lib-*` format crates.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The serialize interface is declared in
//! `kit-stdlib/data/serialize/serialize.hnl`. The [`Encoder`], [`Decoder`],
//! [`Serializer`], and [`SerializerRegistry`] traits remain in Rust — they
//! are implemented by concrete format crates (e.g. `plugin-lib-json`,
//! `plugin-lib-toml`, `plugin-lib-yaml`). The [`SerializeLibraryPlugin`]
//! wraps the generic [`HandlLibraryPlugin`] with the `.hnl` source,
//! eliminating the hand-rolled `Plugin` trait scaffolding.

pub mod error;
pub mod serializer;
pub mod traits;

pub use error::SerializeError;
pub use serializer::{Serializer, SerializerRegistry};
pub use traits::{Decoder, Encoder};

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the serialize interface — the single source
/// of truth for the serialize API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const SERIALIZE_HNL: &str = include_str!("../../../kit-stdlib/data/serialize/serialize.hnl");

/// Serialize a value using the provided encoder.
pub fn serialize<T, E>(value: &T, encoder: &E) -> Result<E::Output, SerializeError>
where
    E: Encoder<T>,
{
    encoder.encode(value)
}

/// Deserialize input using the provided decoder.
pub fn deserialize<I, T, D>(input: &I, decoder: &D) -> Result<T, SerializeError>
where
    I: ?Sized,
    D: Decoder<I, T>,
{
    decoder.decode(input)
}

// ---------------------------------------------------------------------------
// SerializeLibraryPlugin (wraps HandlLibraryPlugin)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `serialize` interface via the
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The serialize interface is declared in `serialize.hnl`; the trait
/// definitions remain in Rust. The generic loader handles module
/// registration.
pub struct SerializeLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl SerializeLibraryPlugin {
    /// Create a new `SerializeLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "serialize",
                "lib.serialize",
                "lib-serialize",
                SERIALIZE_HNL,
                &[],
            ),
        }
    }
}

impl Default for SerializeLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for SerializeLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for serialize.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`SerializeLibraryPlugin`].
pub fn serialize_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new_with_caps(
        "serialize",
        "lib.serialize",
        "lib-serialize",
        SERIALIZE_HNL,
        &[],
    )
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`SerializeLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_serialize::LibraryPlugin`.
pub use SerializeLibraryPlugin as LibraryPlugin;
