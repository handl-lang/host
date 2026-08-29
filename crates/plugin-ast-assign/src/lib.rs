use handl_plugin_ast::{Node, NodeMetadata};

/// assignment expression
#[derive(Debug, Clone)]
pub struct Assign<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub target: E,
    pub value: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Assign<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Assign node with default metadata.
    pub fn new(target: E, value: E) -> Self {
        Self {
            target,
            value,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Assign<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.value == other.value && self.metadata == other.metadata
    }
}

impl<E, M> Node for Assign<E, M>
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
            value: self.value,
            metadata,
        }
    }
}
