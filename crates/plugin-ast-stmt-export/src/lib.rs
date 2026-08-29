use handl_plugin_ast::{Node, NodeMetadata};

/// An `export` statement: `export value`.
#[derive(Debug, Clone)]
pub struct Export<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// The exported value expression.
    pub value: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Export<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new `Export` node with default metadata.
    pub fn new(value: E) -> Self {
        Self {
            value,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Export<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.metadata == other.metadata
    }
}

impl<E, M> Node for Export<E, M>
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
            value: self.value,
            metadata,
        }
    }
}
