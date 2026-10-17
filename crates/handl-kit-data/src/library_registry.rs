//! Registry mapping library plugin names to zero-argument constructors.
//!
//! This is the library-plugin counterpart to [`KitRegistry`]: it lets a
//! kit manifest name a library plugin (e.g. `json`, `os-std`) without
//! hardcoding the Rust constructor call. The registry itself is generic and
//! dependency-free of any concrete `plugin-lib-*` crate; downstream code
//! populates it with the constructors it needs.
//!
//! [`KitRegistry`]: crate::KitRegistry

use std::collections::HashMap;

use handl_platform::Plugin;

/// A boxed, thread-safe library plugin instance.
pub type LibraryPlugin = Box<dyn Plugin + Send + Sync>;

/// A registry of zero-argument constructors for library plugins.
///
/// Constructed empty and populated by a downstream crate that depends on the
/// concrete plugin crates (for example `handl-plugin-ext-kit`).
#[derive(Default)]
pub struct LibraryRegistry {
    constructors: HashMap<String, Box<dyn Fn() -> LibraryPlugin + Send + Sync + 'static>>,
}

impl LibraryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            constructors: HashMap::new(),
        }
    }

    /// Register a zero-argument constructor under `name`.
    pub fn register<F>(&mut self, name: &str, ctor: F)
    where
        F: Fn() -> LibraryPlugin + Send + Sync + 'static,
    {
        self.constructors
            .insert(name.to_string(), Box::new(ctor) as Box<_>);
    }

    /// Resolve `name` to a freshly constructed library plugin.
    pub fn resolve(&self, name: &str) -> Option<LibraryPlugin> {
        self.constructors.get(name).map(|ctor| ctor())
    }

    /// Returns `true` if a constructor for `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.constructors.contains_key(name)
    }

    /// Iterate over registered names in an arbitrary order.
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.constructors.keys().map(|s| s.as_str())
    }
}
