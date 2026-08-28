//! Minimal, composable VM piece for the handl platform.
//!
//! The `Vm` pairs a host with the unified `handl-vm-type` value/term
//! environment, allowing the runtime to be composed from `handl-platform`.

pub use crate::value::*;

use crate::host::DefaultHost;

/// A composable handl VM that pairs a host with a type/runtime environment.
pub struct Vm {
    host: DefaultHost,
    env: Env,
}

impl Vm {
    /// Creates a new VM with an empty host and environment.
    pub fn new() -> Self {
        Self {
            host: DefaultHost::new(),
            env: Env::new(),
        }
    }

    /// Creates a new VM pre-populated with the prelude environment.
    pub fn with_prelude() -> Self {
        Self {
            host: DefaultHost::new(),
            env: prelude_env(),
        }
    }

    /// Returns a reference to the host.
    pub fn host(&self) -> &DefaultHost {
        &self.host
    }

    /// Returns a mutable reference to the host.
    pub fn host_mut(&mut self) -> &mut DefaultHost {
        &mut self.host
    }

    /// Returns a reference to the environment.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Returns a mutable reference to the environment.
    pub fn env_mut(&mut self) -> &mut Env {
        &mut self.env
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
