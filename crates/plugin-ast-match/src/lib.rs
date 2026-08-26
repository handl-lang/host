use handl_ast::{Node, NodeMetadata};

/// match expression
#[derive(Debug, Clone)]
pub struct Match<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub subject: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Match<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Match node with default metadata.
    pub fn new(subject: E) -> Self {
        Self {
            subject,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Match<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.subject == other.subject && self.metadata == other.metadata
    }
}

impl<E, M> Node for Match<E, M>
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
            subject: self.subject,
            metadata,
        }
    }
}
