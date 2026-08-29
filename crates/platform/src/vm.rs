//! Minimal, composable VM piece for the handl platform.
//!
//! The `Vm` is generic over the host and environment types so that the
//! concrete `DefaultHost` and `Env` are no longer hard-coded.  A
//! `RuntimeHost` (or any other `Host` implementation) can be supplied
//! instead.

pub use crate::value::*;

/// A composable handl VM that pairs a host with a runtime environment.
pub struct Vm<Host, Env> {
    host: Host,
    env: Env,
}

impl<Host, Env> Vm<Host, Env> {
    /// Creates a new VM with a default host and environment.
    pub fn new() -> Self
    where
        Host: Default,
        Env: Default,
    {
        Self {
            host: Host::default(),
            env: Env::default(),
        }
    }

    /// Returns a reference to the host.
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Returns a mutable reference to the host.
    pub fn host_mut(&mut self) -> &mut Host {
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

impl<Host, Env> Default for Vm<Host, Env>
where
    Host: Default,
    Env: Default,
{
    fn default() -> Self {
        Self::new()
    }
}
