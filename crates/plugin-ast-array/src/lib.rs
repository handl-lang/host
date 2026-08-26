use handl_ast::{Node, NodeMetadata};

/// array literal
#[derive(Debug, Clone)]
pub struct Array<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub elements: Vec<E>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Array<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Array node with default metadata.
    pub fn new(elements: Vec<E>) -> Self {
        Self {
            elements,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Array<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements && self.metadata == other.metadata
    }
}

impl<E, M> Node for Array<E, M>
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
            elements: self.elements,
            metadata,
        }
    }
}
