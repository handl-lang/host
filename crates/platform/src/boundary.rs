//! Boundary types describing isolation and deployment units.

/// Kinds of runtime boundaries a plugin or host may target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryKind {
    /// A single language module.
    Module,
    /// A package of related modules.
    Package,
    /// A separate operating-system process.
    Process,
    /// A worker thread or isolate.
    Worker,
    /// A network server.
    Server,
    /// A network client.
    Client,
    /// An RPC interface.
    Rpc,
    /// A deployed service.
    Deployment,
}

/// A named boundary of a particular kind.
#[derive(Debug, Clone)]
pub struct Boundary {
    /// The kind of boundary.
    pub kind: BoundaryKind,
    /// The human-readable boundary name.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_kinds_exhausted() {
        let b = Boundary {
            kind: BoundaryKind::Process,
            name: "worker-pool".into(),
        };
        assert_eq!(b.kind, BoundaryKind::Process);
        assert_eq!(b.name, "worker-pool");
    }
}
