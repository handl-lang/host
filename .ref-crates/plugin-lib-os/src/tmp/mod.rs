mod tmp;

pub use tmp::{TMP_OPS, Tmp, TmpModule, register_tmp_ops};

pub mod tmp_stub;

#[cfg(test)]
mod tmp_tests;
