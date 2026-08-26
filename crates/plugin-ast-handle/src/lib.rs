use handl_ast::{Node, NodeMetadata};

/// handle expression
#[derive(Debug, Clone)]
pub struct Handle<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub expr: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Handle<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Handle node with default metadata.
    pub fn new(expr: E) -> Self {
        Self {
            expr,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Handle<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.expr == other.expr && self.metadata == other.metadata
    }
}

impl<E, M> Node for Handle<E, M>
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
            expr: self.expr,
            metadata,
        }
    }
}
