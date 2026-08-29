use handl_plugin_ast::{Node, NodeMetadata};

/// let binding
#[derive(Debug, Clone)]
pub struct Let<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub name: String,
    pub value: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Let<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Let node with default metadata.
    pub fn new(name: String, value: E) -> Self {
        Self {
            name,
            value,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Let<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value && self.metadata == other.metadata
    }
}

impl<E, M> Node for Let<E, M>
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
            name: self.name,
            value: self.value,
            metadata,
        }
    }
}
