use handl_ast::{Node, NodeMetadata};
use std::marker::PhantomData;

/// variable reference
#[derive(Debug, Clone)]
pub struct Var<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub name: String,
    /// Keeps the expression type parameter in the type.
    pub _marker: PhantomData<E>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Var<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Var node with default metadata.
    pub fn new(name: String) -> Self {
        Self {
            name,
            _marker: PhantomData,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Var<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.metadata == other.metadata
    }
}

impl<E, M> Node for Var<E, M>
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
            _marker: self._marker,
            metadata,
        }
    }
}
