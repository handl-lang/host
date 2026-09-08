//! # handl-lib-utils
//!
//! Standard library modules for the handl runtime.
//!
//! Provides concrete [`Module`](handl_plugin_vm_common::Module) implementations with useful functions for
//! expressions and interpolation:
//!
//! - [`MathModule`] — `math.abs`, `math.floor`, `math.ceil`, `math.round`,
//!   `math.min`, `math.max`, `math.sqrt`, `math.pow`, `math.sign`, `math.trunc`,
//!   `math.clamp`, `math.log`, `math.log10`, `math.log2`, `math.exp`, `math.sin`,
//!   `math.cos`, `math.tan`, `math.asin`, `math.acos`, `math.atan`, `math.atan2`,
//!   `math.hypot`, `math.mod`, `math.cbrt`, `math.degrees`, `math.radians`,
//!   `math.round_to`, `math.pi`, `math.e`
//! - `str.*` — the `str` contract is implemented by `str-handl`
//!   (`handl-packages/str-handl/str-handl.hnl`), loaded as a
//!   [`HandlModule`](handl_plugin_lib_handl::HandlModule) over the `char` floor.
//! - [`BoolModule`] — `bool.and`, `bool.or`, `bool.not`, `bool.xor`,
//!   `bool.nand`, `bool.nor`, `bool.implies`, `bool.eq`
//! - [`CombinatorsModule`] — `combinators.pipe`, `combinators.flow`, `combinators.compose`, `combinators.fanout`,
//!   `combinators.choice`, `combinators.apply`, `combinators.map`, `combinators.filter`,
//!   `combinators.reduce`, `combinators.tap`, `combinators.constantly`, `combinators.flip`,
//!   `combinators.partial`, `combinators.when`, `combinators.unless`, `combinators.converge`
//! - [`TypeModule`] — `type.to_string`, `type.to_number`, `type.to_bool`,
//!   `type.type_of`, `type.identity`, `type.default`, `type.coalesce`
//! - [`OptionModule`] — `option.some`, `option.none`, `option.is_some`,
//!   `option.is_none`, `option.unwrap`, `option.unwrap_or`, `option.map`,
//!   `option.flat_map`, `option.filter`, `option.or`, `option.and`, `option.xor`
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_vm_common::ModuleRuntime;
//! use handl_plugin_lib_utils::{MathModule, BoolModule,
//!     CombinatorsModule, TypeModule, OptionModule};
//!
//! let host = ModuleRuntime::new()
//!     .with(MathModule)
//!     .with(BoolModule)
//!     .with(CombinatorsModule)
//!     .with(TypeModule)
//!     .with(OptionModule);
//! ```
//!
//! This crate is interface-agnostic — usable from CLI, MCP, or any consumer
//! that evaluates handl expressions.
//!
//! ## ADR-0077 Phase 4: op/handl migration
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The aggregated utils interface is declared in
//! `handl-packages/utils/utils.hnl` via `---` doc comments and `cap`
//! declarations. The [`UtilsLibraryPlugin`] wraps a generic
//! [`HandlLibraryPlugin`] for the interface declaration (id, name,
//! capabilities) while contributing multiple [`ModuleRule`]s — one per
//! sub-module — into the [`ModuleRuntime`]. The Rust `Module`/`Function`
//! impls are preserved for the native-backed sub-modules; `str` is now
//! provided by the pure-handl `str-handl` implementation.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    Module, PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{HandlLibraryPlugin, ModuleResolver, build_str_handl_module};
use handl_plugin_module::{ModuleRule, ModuleRuntime};
use std::sync::Arc;

mod bool;
mod encoding;
mod r#fn;
pub mod function_value;
mod math;
mod mood;
mod option;
mod parse;
mod serialize;
pub mod shared;
mod r#type;

pub use bool::BoolModule;
pub use encoding::EncodingModule;
pub use r#fn::CombinatorsModule;
pub use handl_plugin_lib_base64::Base64Module;
pub use handl_plugin_lib_hex::HexModule;
pub use handl_plugin_lib_json::JsonModule;
pub use handl_plugin_lib_toml::TomlModule;
pub use handl_plugin_lib_url_rfc::UrlRfcModule;
pub use handl_plugin_lib_url_whatwg::UrlWhatwgModule;
pub use handl_plugin_lib_yaml::YamlModule;
pub use math::MathModule;
pub use mood::MoodModule;
pub use option::OptionModule;
pub use parse::ParseModule;
pub use serialize::SerializeModule;
pub use r#type::TypeModule;

/// The handl source declaring the aggregated utils interface — the single
/// source of truth for the utils API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// The `.hnl` file contains `---` doc comments describing every sub-module
/// and its ops, plus `cap` declarations for the utils-owned modules. The
/// re-exported crates (base64, hex, json, toml, yaml, url-rfc, url-whatwg)
/// each have their own `.hnl` files declaring their own op interfaces.
const UTILS_HNL: &str = include_str!("../../../handl-packages/utils/utils.hnl");

// ---------------------------------------------------------------------------
// UtilsLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers all the utils sub-modules into the
/// [`ModuleRuntime`] and declares the aggregated interface via a
/// [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// Because `plugin-lib-utils` is an **aggregator** crate that registers
/// multiple modules (bool, fn, math, mood, option, parse, serialize,
/// typeof, encoding, plus re-exported base64/hex/json/toml/yaml/url
/// modules), it cannot use a single [`HandlLibraryPlugin`] `contribute`
/// (which registers exactly one module). Instead, the [`HandlLibraryPlugin`]
/// inner is used for the interface declaration — id, name, and capabilities
/// parsed from `utils.hnl` — while the custom [`Plugin::contribute`]
/// implementation registers one [`ModuleRule`] per sub-module.
///
/// The Rust `Module`/`Function` impls are preserved for the native-backed
/// sub-modules; `str` is provided by the pure-handl `str-handl` module.
pub struct UtilsLibraryPlugin {
    inner: HandlLibraryPlugin,
}

/// Resolver used by the `str-handl` module to find its `char` backend.
struct CharResolver;

impl ModuleResolver for CharResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        if name == "char" {
            Some(Arc::new(CharModule))
        } else {
            None
        }
    }
}

impl UtilsLibraryPlugin {
    /// Create a new `UtilsLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "utils",
                "lib.utils",
                "lib-utils",
                UTILS_HNL,
                &[],
            ),
        }
    }
}

impl Default for UtilsLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for UtilsLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        // Utils-owned modules (hand-rolled Module impls).
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(MathModule))?;
        // `str` is implemented by pure-handl `str-handl` over the `char` floor.
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(build_str_handl_module(&CharResolver)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(BoolModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(CombinatorsModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TypeModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(OptionModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(MoodModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(EncodingModule::new()))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(ParseModule::new()))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(SerializeModule::new()))?;
        // Re-exported modules from other crates.
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(Base64Module))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(HexModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(JsonModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TomlModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(YamlModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(UrlRfcModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(UrlWhatwgModule))?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`UtilsLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_utils::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use UtilsLibraryPlugin as LibraryPlugin;
