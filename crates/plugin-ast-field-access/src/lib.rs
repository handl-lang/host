use handl_plugin_ast::{Node, NodeMetadata};

/// field access
#[derive(Debug, Clone)]
pub struct FieldAccess<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub target: E,
    pub field: String,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> FieldAccess<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new FieldAccess node with default metadata.
    pub fn new(target: E, field: String) -> Self {
        Self {
            target,
            field,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for FieldAccess<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.field == other.field && self.metadata == other.metadata
    }
}

impl<E, M> Node for FieldAccess<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    type Value = E::Value;
    type Metadata = M;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn with_metadata(self, metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        Self {
            target: self.target,
            field: self.field,
            metadata,
        }
    }
}
