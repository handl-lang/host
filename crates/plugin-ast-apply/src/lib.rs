use handl_plugin_ast::{Node, NodeMetadata};

/// function application
#[derive(Debug, Clone)]
pub struct Apply<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub target: E,
    pub args: Vec<E>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Apply<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Apply node with default metadata.
    pub fn new(target: E, args: Vec<E>) -> Self {
        Self {
            target, args,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Apply<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.args == other.args && self.metadata == other.metadata
    }
}

impl<E, M> Node for Apply<E, M>
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
            target: self.target, args: self.args,
            metadata,
        }
    }
}
