//! # handl-lib-archive-tar
//!
//! Tar implementation of the [`handl_plugin_lib_archive`] [`Archive`] trait using [`tar`].

mod tar_archive;
mod module;

#[cfg(test)]
mod tar_tests;

pub use tar_archive::TarArchive;
pub use module::TarModule;

pub mod plugin;
pub use plugin::LibraryPlugin;
