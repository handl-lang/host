//! C type → handl port + FFI marshalling descriptor mapping.
//!
//! - [`arg`] — [`map_arg_with_peek`]: one function argument, peeking at
//!   the next to collapse the C `(ptr, len)` convention into `Bytes`.
//! - [`scalar`] — [`map_arg_type`], [`map_field_type`],
//!   [`map_return_type`]: scalars, record fields, return types.
//! - [`pred`] — `CXType` kind predicates (`is_void`, `is_char`, ...).
//!
//! The parse context is reduced to `pointer_width` so this module stays
//! decoupled from [`crate::parse`].

pub(crate) mod arg;
pub(crate) mod pred;
pub(crate) mod scalar;

pub(crate) use arg::map_arg_with_peek;
pub(crate) use scalar::{is_ctypes_port, map_arg_type, map_field_type, map_return_type};

#[cfg(test)]
mod arg_test;
#[cfg(test)]
mod pred_test;
#[cfg(test)]
mod scalar_test;
