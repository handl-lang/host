//! # handl-kit-data-config
//!
//! The **data-driven config kit** — extends [`handl-kit-data-canonical`] via
//! the `extends` chain (ADR-0044 §1, §5), adding pure-library capability
//! declarations.
//!
//! This demonstrates the `extends` chain: the parent (`kit-data-canonical`) is
//! assembled first, then this kit's fragments (pure-library
//! [`CapabilityDecl`](handl_language::CapabilityDecl)s) merge on top. No new
//! grammar or eval fragments are added — this models how a config kit curates
//! *capabilities* (the pure stdlib surface) without changing the language's
//! parse/eval behavior. Per ADR-0039, `kit-config` curates the pure
//! (side-effect-free) library plugins; this data-driven equivalent declares
//! those capabilities as data fragments.
//!
//! The existing `kit-config` crate is **not** modified or deleted — it remains
//! as the pre-migration reference.

#![warn(missing_docs)]

use handl_emitters::Emitter;
use handl_kit_data::{DataKit, DataKitManifest, EmitterRegistry, FragmentSourceEntry};
use handl_kit_data_canonical;
use handl_language::{CapabilityDecl, LangError, Language};

/// The kit name under which this kit registers itself.
pub const KIT_NAME: &str = "kit-data-config";

/// The pure-library capability ids curated by this kit (mirroring
/// `kit-config`'s pure stdlib surface: math, str, array, set, bool, cmp, obj,
/// fn, type, crypto, datetime, intl, kv — no OS, no network, no RNG).
pub const PURE_CAPABILITY_IDS: &[&str] = &[
    "lib.math",
    "lib.str",
    "lib.array",
    "lib.set",
    "lib.bool",
    "lib.cmp",
    "lib.obj",
    "lib.fn",
    "lib.type",
    "lib.crypto",
    "lib.datetime",
    "lib.intl",
    "lib.kv",
];

/// An emitter that produces the pure-library capability declarations.
///
/// This models how a kit curates capabilities as data fragments: the emitter
/// emits [`CapabilityDecl`]s (which use union/idempotent merge semantics), so
/// merging the same capability twice is a no-op.
#[derive(Debug, Default, Clone, Copy)]
pub struct PureCapsEmitter;

impl PureCapsEmitter {
    /// Create a new pure-caps emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for PureCapsEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the pure-library capability declarations as a [`Language`].
///
/// # Errors
///
/// Returns [`LangError`] if a capability merge fails (should not happen —
/// `CapabilityDecl` uses union semantics).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    for cap in PURE_CAPABILITY_IDS {
        lang.caps
            .merge(CapabilityDecl::new(*cap).with_description("pure library capability"))?;
    }
    Ok(lang)
}

/// Build the [`DataKitManifest`] for the data-driven config kit.
///
/// Extends [`handl_kit_data_canonical::KIT_NAME`] and adds a single
/// `pure-caps` fragment source (the [`PureCapsEmitter`]). The runtime config
/// is inherited from the parent (Pratt + tree-walk).
pub fn manifest() -> DataKitManifest {
    DataKitManifest::new()
        .with_extends(handl_kit_data_canonical::KIT_NAME)
        .with_fragment(FragmentSourceEntry::emitter("pure-caps", "pure-caps"))
        .with_runtime(
            handl_runtime::RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval("tree-walk"),
        )
}

/// Build an [`EmitterRegistry`] pre-populated with the canonical kit's
/// emitters/manifest plus this kit's `pure-caps` emitter and manifest.
pub fn registry() -> EmitterRegistry {
    let mut reg = handl_kit_data_canonical::registry();
    reg.register_emitter("pure-caps", || Box::new(PureCapsEmitter::new()));
    reg.register_kit(KIT_NAME, manifest());
    reg
}

/// Assemble the data-driven config kit: build the registry (canonical + this
/// kit) and assemble via the `extends` chain.
///
/// # Errors
///
/// Returns [`LangError`] if assembly fails.
pub fn assemble() -> Result<DataKit, LangError> {
    let reg = registry();
    DataKit::assemble(&manifest(), &reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_extends_canonical() {
        let m = manifest();
        assert_eq!(
            m.extends.as_deref(),
            Some(handl_kit_data_canonical::KIT_NAME)
        );
        assert_eq!(m.fragments.len(), 1);
        assert_eq!(m.fragments[0].name, "pure-caps");
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let m = manifest();
        let j = serde_json::to_string(&m).unwrap();
        let back: DataKitManifest = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn pure_caps_emitter_produces_all_capabilities() {
        let lang = emit().unwrap();
        assert_eq!(lang.caps.len(), PURE_CAPABILITY_IDS.len());
        for cap in PURE_CAPABILITY_IDS {
            assert!(lang.caps.get_by_id(cap).is_some(), "missing {}", cap);
        }
    }

    #[test]
    fn assemble_extends_canonical_with_caps() {
        let kit = assemble().unwrap();
        // The canonical language fragments are present (inherited via extends).
        assert!(kit.lang().tokens.len() > 0);
        assert!(kit.lang().grammar.len() > 0);
        assert!(kit.lang().eval.len() > 0);
        // The canonical kit now declares foundation capabilities (lex, parse,
        // textmate) via the `tokens` and `textmate` emitters; config adds the
        // pure-library capabilities on top. Capabilities are union/idempotent,
        // so the total is the canonical caps plus the pure caps (no overlap).
        let canonical = handl_kit_data_canonical::assemble().unwrap();
        let canonical_caps = canonical.lang().caps.len();
        assert!(canonical_caps > 0);
        assert_eq!(
            kit.lang().caps.len(),
            canonical_caps + PURE_CAPABILITY_IDS.len()
        );
        for cap in PURE_CAPABILITY_IDS {
            assert!(kit.lang().caps.get_by_id(cap).is_some());
        }
    }

    #[test]
    fn assemble_caps_are_union_with_parent() {
        // The canonical kit declares foundation capabilities (lex, parse,
        // textmate); config adds the pure-library capabilities on top.
        // Capabilities are union/idempotent, so the parent's caps are kept and
        // the pure caps merge alongside them (no overlap → sum).
        let canonical = handl_kit_data_canonical::assemble().unwrap();
        let canonical_caps = canonical.lang().caps.len();
        assert!(canonical_caps > 0);
        let config = assemble().unwrap();
        assert_eq!(
            config.lang().caps.len(),
            canonical_caps + PURE_CAPABILITY_IDS.len()
        );
    }

    #[test]
    fn registry_resolves_pure_caps_emitter() {
        let reg = registry();
        assert!(reg.resolve_emitter("pure-caps").is_some());
    }

    #[test]
    fn registry_resolves_config_kit_manifest() {
        let reg = registry();
        let m = reg.resolve_kit(KIT_NAME).unwrap();
        assert_eq!(
            m.extends.as_deref(),
            Some(handl_kit_data_canonical::KIT_NAME)
        );
    }
}
