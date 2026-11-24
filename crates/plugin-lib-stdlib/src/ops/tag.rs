//! `impl:` table for the `tag` module (P-013 T-151) — host `Function`
//! impls re-exported from `handl-plugin-lib-tag` (the crate that owns the
//! `__tag__` inspection semantics).
//!
//! `tag` was historically *not* a `packages::MODULES` row — it resolved
//! through the resolver fallback (kit-canonical registers
//! `TagModule`/`TagLibraryPlugin` directly). T-151 makes it a table row
//! driven by `lib-tag/atoms.yaml`, so the spec — not `TAG_OPS` +
//! `tag_op_registry()` — is the source of truth for the module's op
//! surface. The impl structs stay in `plugin-lib-tag` (they are `pub`
//! there); this table only keys them.

use std::sync::Arc;

use handl_platform::Function;

/// The `impl:` table `lib-tag/atoms.yaml` (`module: tag`) keys into —
/// `impl` name → `Function` impl. The atoms spec (4 `tag.*` atoms with
/// sigs) plus this table fully determine the module's
/// `handl_plugin_lib_handl::OpRegistry`;
/// [`crate::atoms::registry_from_atoms`] does the merge.
pub fn tag_impls() -> Vec<(&'static str, Arc<dyn Function + Send + Sync>)> {
    vec![
        ("tag_untag", Arc::new(handl_plugin_lib_tag::Untag)),
        ("tag_tag_of", Arc::new(handl_plugin_lib_tag::TagOf)),
        ("tag_is_tag", Arc::new(handl_plugin_lib_tag::IsTag)),
        ("tag_match_type", Arc::new(handl_plugin_lib_tag::MatchType)),
    ]
}
