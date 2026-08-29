//! # handl-plugin-lib-utils
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
//! - [`StrModule`] — `str.upper`, `str.lower`, `str.trim`,
//!   `str.trim_start`, `str.trim_end`, `str.split`, `str.join`,
//!   `str.replace`, `str.replace_first`, `str.contains`,
//!   `str.starts_with`, `str.ends_with`, `str.slice`, `str.reverse`,
//!   `str.repeat`, `str.find`, `str.count`, `str.char_at`,
//!   `str.pad_left`, `str.pad_right`, `str.length`, `str.concat`,
//!   `str.strip_prefix`, `str.strip_suffix`, `str.lines`, `str.rfind`,
//!   `str.capitalize`, `str.title_case`, `str.snake_case`, `str.camel_case`,
//!   `str.pascal_case`, `str.kebab_case`, `str.split_at`, `str.trim_matches`,
//!   `str.is_empty`, `str.equals`, `str.is_alpha`,
//!   `str.is_numeric`, `str.is_alphanumeric`, `str.levenshtein`,
//!   `str.similarity`, `str.split_whitespace`, `str.truncate`,
//!   `str.slugify`, `str.word_wrap`
//! - [`BoolModule`] — `bool.and`, `bool.or`, `bool.not`, `bool.xor`,
//!   `bool.nand`, `bool.nor`, `bool.implies`, `bool.eq`
//! - [`CmpModule`] — `cmp.eq`, `cmp.ne`, `cmp.lt`, `cmp.le`, `cmp.gt`,
//!   `cmp.ge`, `cmp.compare`, `cmp.min`, `cmp.max`, `cmp.clamp`,
//!   `cmp.between`, `cmp.in_range`, `cmp.is_sorted`, `cmp.is_sorted_desc`,
//!   `cmp.sort`, `cmp.sort_desc`
//! - [`FnModule`] — `fn.pipe`, `fn.flow`, `fn.compose`, `fn.fanout`,
//!   `fn.choice`, `fn.apply`, `fn.map`, `fn.filter`, `fn.reduce`,
//!   `fn.tap`, `fn.constantly`, `fn.flip`, `fn.partial`, `fn.when`,
//!   `fn.unless`, `fn.converge`
//! - [`TypeModule`] — `type.to_string`, `type.to_number`, `type.to_bool`,
//!   `type.type_of`, `type.identity`, `type.default`, `type.coalesce`
//! - [`OptionModule`] — `option.some`, `option.none`, `option.is_some`,
//!   `option.is_none`, `option.unwrap`, `option.unwrap_or`, `option.map`,
//!   `option.flat_map`, `option.filter`, `option.or`, `option.and`, `option.xor`
//! - [`ResultModule`] — `result.ok`, `result.err`, `result.is_ok`,
//!   `result.is_err`
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_vm_common::ModuleHost;
//! use handl_plugin_lib_utils::{MathModule, StrModule, BoolModule, CmpModule,
//!     FnModule, TypeModule, OptionModule};
//!
//! let host = ModuleHost::new()
//!     .with(MathModule)
//!     .with(StrModule)
//!     .with(BoolModule)
//!     .with(CmpModule)
//!     .with(FnModule)
//!     .with(TypeModule)
//!     .with(OptionModule);
//! ```
//!
//! This crate is interface-agnostic — usable from CLI, MCP, or any consumer
//! that evaluates handl expressions.

mod bool;
mod cmp;
mod encoding;
mod r#fn;
mod math;
mod mood;
mod option;
mod parse;
mod result;
mod serialize;
pub(crate) mod shared;
mod r#str;
mod r#type;

pub use bool::BoolModule;
pub use cmp::CmpModule;
pub use encoding::EncodingModule;
pub use r#fn::FnModule;
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
pub use result::ResultModule;
pub use serialize::SerializeModule;
pub use r#str::StrModule;
pub use r#type::TypeModule;
