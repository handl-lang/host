//! Cursor → `SdkgenModule` IR lowering.
//!
//! - [`record`] — struct/union declarations → `Record` (fields, layout).
//! - [`typedef`] — typedef declarations → `Typedef`.
//! - [`function`] — function declarations → `Function`, including
//!   `(ptr, len)` pair collapse, `out_struct` resolution, and the
//!   opaque-handle fallback for unknown records.

use handl_plugin_sdkgen::Record;
use std::cell::RefCell;
use std::collections::HashMap;

pub(crate) mod function;
pub(crate) mod record;
pub(crate) mod typedef;

pub(crate) use function::parse_function;
pub(crate) use record::parse_record;
pub(crate) use typedef::parse_typedef;

/// Per-translation-unit parse state shared by the lowering functions.
pub(crate) struct ParseCtx {
    /// Pointer width in bits — decides `i32` vs `i64` for `size_t`.
    pub pointer_width: usize,
    /// Records parsed so far — consulted by `out_struct_descriptor`.
    /// `RefCell` because `parse_function` reads while `parse_record`
    /// fills.
    pub records: RefCell<HashMap<String, Record>>,
}

#[cfg(test)]
mod function_test;
#[cfg(test)]
mod record_test;
#[cfg(test)]
mod typedef_test;
