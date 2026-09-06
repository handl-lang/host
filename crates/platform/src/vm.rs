//! Minimal, composable VM piece for the handl platform.
//!
//! The `Vm` is generic over the host and environment types so that the
//! concrete `DefaultHost` and `Env` are no longer hard-coded.  Any
//! `CallableLookup` implementation can be supplied instead.

/// A composable handl VM that pairs a host with a runtime environment.
pub struct Vm<CallableLookup, Env> {
    host: CallableLookup,
    env: Env,
}

impl<CallableLookup, Env> Vm<CallableLookup, Env> {
    /// Creates a new VM with a default host and environment.
    pub fn new() -> Self
    where
        CallableLookup: Default,
        Env: Default,
    {
        Self {
            host: CallableLookup::default(),
            env: Env::default(),
        }
    }

    /// Returns a reference to the host.
    pub fn host(&self) -> &CallableLookup {
        &self.host
    }

    /// Returns a mutable reference to the host.
    pub fn host_mut(&mut self) -> &mut CallableLookup {
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

impl<CallableLookup, Env> Default for Vm<CallableLookup, Env>
where
    CallableLookup: Default,
    Env: Default,
{
    fn default() -> Self {
        Self::new()
    }
}
