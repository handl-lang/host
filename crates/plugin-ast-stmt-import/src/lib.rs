use handl_plugin_ast::{Node, NodeMetadata};

/// An `import` statement: `import source`.
#[derive(Debug, Clone)]
pub struct Import<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// The imported source expression.
    pub source: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Import<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new `Import` node with default metadata.
    pub fn new(source: E) -> Self {
        Self {
            source,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Import<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.metadata == other.metadata
    }
}

impl<E, M> Node for Import<E, M>
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
            source: self.source,
            metadata,
        }
    }
}
