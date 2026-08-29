use handl_plugin_ast::{Node, NodeMetadata};

/// postfix index expression
#[derive(Debug, Clone)]
pub struct Index<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub target: E,
    pub index: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Index<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Index node with default metadata.
    pub fn new(target: E, index: E) -> Self {
        Self {
            target, index,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Index<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.index == other.index && self.metadata == other.metadata
    }
}

impl<E, M> Node for Index<E, M>
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
            target: self.target, index: self.index,
            metadata,
        }
    }
}
