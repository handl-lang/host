mod fs;

pub use fs::{FS_OPS, Fs, FsModule, register_fs_ops};

pub mod fs_stub;

#[cfg(test)]
mod fs_tests;
