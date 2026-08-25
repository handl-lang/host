#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryKind {
    Module,
    Package,
    Process,
    Worker,
    Server,
    Client,
    Rpc,
    Deployment,
}

#[derive(Debug, Clone)]
pub struct Boundary {
    pub kind: BoundaryKind,
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
