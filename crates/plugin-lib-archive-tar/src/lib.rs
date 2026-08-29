//! # handl-lib-archive-tar
//!
//! Tar implementation of the [`handl_plugin_lib_archive`] [`Archive`] trait using [`tar`].

mod module;
mod tar_archive;

#[cfg(test)]
mod tar_tests;

pub use module::TarModule;
pub use tar_archive::TarArchive;

pub mod plugin;
pub use plugin::LibraryPlugin;
