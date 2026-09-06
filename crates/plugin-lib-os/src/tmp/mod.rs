mod tmp;

pub use tmp::{register_tmp_ops, Tmp, TmpModule, TMP_OPS};

pub mod tmp_stub;

#[cfg(test)]
mod tmp_tests;
