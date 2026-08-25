use std::collections::HashMap;

use handl_platform::PlatformError;
use handl_vm_common::ModuleHost;

/// A composable standard library that can install its functions into a VM host.
///
/// Each `Library` has a name (used for `handl.config.json` and `--lib`) and
/// an `install` method that registers its modules/functions into a `ModuleHost`.
pub trait Library: Send + Sync {
    /// The library name, e.g. `hello` or `collections`.
    fn name(&self) -> &str;

    /// Install this library's modules/functions into the given `ModuleHost`.
    fn install(&self, host: ModuleHost) -> ModuleHost;
}

/// A registry that maps library names to `Library` factories.
#[derive(Default)]
pub struct LibRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn Library> + Send + Sync + 'static>>,
}

impl LibRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Registers a `Library` factory under `name`.
    pub fn register<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn() -> Box<dyn Library> + Send + Sync + 'static,
    {
        self.factories.insert(name.into(), Box::new(factory));
    }

    /// Resolves `name` to a new `Library` instance.
    pub fn resolve(&self, name: &str) -> Option<Box<dyn Library>> {
        self.factories.get(name).map(|f| f())
    }

    /// Returns the names of all registered libraries.
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.factories.keys().map(String::as_str)
    }

    /// Returns an error for an unknown library name.
    pub fn unknown(name: &str) -> PlatformError {
        PlatformError::Plugin(format!("unknown library: {name}"))
    }
}
