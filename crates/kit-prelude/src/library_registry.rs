//! The prelude kit's library-module table (ADR-0128 §0.4).
//!
//! Moved out of `plugin-ext-kit`: a kit manifest's `modules:` names
//! resolve through a [`LibraryRegistry`] *the kit supplies*. The table is
//! composed, not monolithic — `module-runtime` lives here (the shared host
//! every kit registers first) and each `kit-*` crate contributes its own
//! rows: `kit-stdlib` the `hello`/`stdlib`/`cli` manifests' libraries,
//! `kit-server` the `config`/`server` manifests'. Hosts wire
//! [`prelude_library_registry`] (or a subset) into `BasicKitLoaderPlugin`;
//! other kits supply their own.

use std::sync::Arc;

use handl_kit_data::{LibraryPlugin, LibraryRegistry};

/// Populate `reg` with the prelude kit's own rows (the shared
/// `module-runtime` host), then delegate to each linked kit's
/// `populate_libraries` for the `stdlib`/`cli`/`config`/`server` manifests'
/// `modules:` sets.
pub fn populate(reg: &mut LibraryRegistry) {
    reg.register("module-runtime", || {
        Box::new(handl_plugin_module::ModuleRuntime::default()) as LibraryPlugin
    });
    handl_kit_stdlib::populate_libraries(reg);
    handl_kit_server::populate_libraries(reg);
}

/// The prelude library registry, shared and ready to inject into a
/// `BasicKitLoaderPlugin`.
pub fn prelude_library_registry() -> Arc<LibraryRegistry> {
    let mut reg = LibraryRegistry::new();
    populate(&mut reg);
    Arc::new(reg)
}
