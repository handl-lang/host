mod fs;

pub use fs::{register_fs_ops, Fs, FsModule, FS_OPS};

pub mod fs_stub;

#[cfg(test)]
mod fs_tests;
