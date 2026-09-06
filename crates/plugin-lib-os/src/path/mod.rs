mod path;

pub use path::{register_path_ops, Path, PathModule, PATH_OPS};

pub mod path_stub;

#[cfg(test)]
mod path_tests;
