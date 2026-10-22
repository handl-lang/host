//! `handl-plugin-ext-typecheck` — the `typecheck` capability adapter.
//!
//! Exposes the canonical bootstrap typecheck (`hnl typecheck` / `hnl check`,
//! ADR-0088 / ADR-0105) through the `plugin-ext-*` capability seam so a host
//! can dispatch it uniformly alongside `fmt` and `lint`.
//!
//! The implementation is [`plugin::TypecheckPlugin`], backed by
//! [`handl_kit_prelude::Kit::typecheck_with_prelude`] — the *same*
//! entry point `host-cli` uses, so the capability path and the direct call
//! cannot drift.
//!
//! # Why there is no `language` payload (unlike `lint`)
//!
//! ADR-0124's language handoff exists because a capability plugin and its
//! caller can assemble *different* languages while both calling them
//! "canonical" — which is exactly what happened to `lint` (`host-cli` used
//! `load_canonical_language()`, the plugin's default was `lint_language()`).
//! `typecheck` has no such hazard: both sides obtain their language from the
//! single assembly path [`Kit::new`] (`handl_kit_data_prelude::assemble`),
//! which is deterministic. Two `Kit`s built from the same canonical data are
//! therefore identical, so a language payload would carry no information.
//!
//! If a non-canonical `Language` ever needs checking through this capability,
//! the extension point is a `language` payload field deserialized into
//! [`handl_language::Language`] and handed to
//! `hnl_engine_typecheck::check_module_with_prelude` (which *does*
//! take `&Language`); that returns diagnostics rather than a synthesized type,
//! so it would be an additive `status` variant, not a replacement.

pub mod plugin;

pub use plugin::{TYPECHECK_CAPABILITY, TypecheckPlugin};
