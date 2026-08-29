use handl_plugin_ast::{Node, NodeMetadata};

/// pipe expression
#[derive(Debug, Clone)]
pub struct Pipe<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub left: E,
    pub right: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Pipe<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Pipe node with default metadata.
    pub fn new(left: E, right: E) -> Self {
        Self {
            left, right,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Pipe<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.left == other.left && self.right == other.right && self.metadata == other.metadata
    }
}

impl<E, M> Node for Pipe<E, M>
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
            left: self.left, right: self.right,
            metadata,
        }
    }
}
