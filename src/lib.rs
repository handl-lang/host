//! # handl-lib-parse
//!
//! Generic, tree-shakeable parsing framework for the handl runtime.
//!
//! Mirrors `handl-lib-serialize` but for one-to-one string-to-value parsing.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The parse interface is declared in
//! `kit-stdlib/data/parse/parse.hnl`. The [`Parser`] and [`DynParser`]
//! traits remain in Rust — they are implemented by concrete parser crates
//! (e.g. `plugin-lib-url-rfc`, `plugin-lib-url-whatwg`). The
//! [`ParseLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with
//! the `.hnl` source, eliminating the hand-rolled `Plugin` trait
//! scaffolding.

use serde_json::Value;

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the parse interface — the single source of
/// truth for the parse API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const PARSE_HNL: &str = include_str!("../../../kit-stdlib/data/parse/parse.hnl");

/// Shared error for all parser failures.
#[derive(Debug, thiserror::Error)]
#[error("parse error: {reason}")]
pub struct ParseError {
    /// Human-readable description of the failure.
    pub reason: String,
}

impl ParseError {
    /// Create a new parse error.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// A parser that converts an `input` of type `I` into an output of type `O`.
pub trait Parser<I: ?Sized, O> {
    /// Parse `input` into `O`.
    fn parse(&self, input: &I) -> Result<O, ParseError>;
}

/// Parse `input` using the given `parser`.
pub fn parse<I: ?Sized, O, P: Parser<I, O>>(input: &I, parser: P) -> Result<O, ParseError> {
    parser.parse(input)
}

/// A parser that converts a string into a `serde_json::Value`.
pub trait DynParser: Send + Sync {
    /// Parse `input` into a `Value`.
    fn parse(&self, input: &str) -> Result<Value, ParseError>;
}

// ---------------------------------------------------------------------------
// ParseLibraryPlugin (wraps HandlLibraryPlugin)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `parse` interface via the
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The parse interface is declared in `parse.hnl`; the trait definitions
/// remain in Rust. The generic loader handles module registration.
pub struct ParseLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl ParseLibraryPlugin {
    /// Create a new `ParseLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "parse",
                "lib.parse",
                "lib-parse",
                PARSE_HNL,
                &[],
            ),
        }
    }
}

impl Default for ParseLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for ParseLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for parse.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`ParseLibraryPlugin`].
pub fn parse_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new_with_caps("parse", "lib.parse", "lib-parse", PARSE_HNL, &[])
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`ParseLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_parse::LibraryPlugin`.
pub use ParseLibraryPlugin as LibraryPlugin;
