use handl_ast::{Node, NodeMetadata};

/// object literal
#[derive(Debug, Clone)]
pub struct Object<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub entries: Vec<(String, E)>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Object<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Object node with default metadata.
    pub fn new(entries: Vec<(String, E)>) -> Self {
        Self {
            entries,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Object<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries && self.metadata == other.metadata
    }
}

impl<E, M> Node for Object<E, M>
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
            entries: self.entries,
            metadata,
        }
    }
}
