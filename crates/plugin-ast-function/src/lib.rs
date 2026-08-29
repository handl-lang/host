use handl_plugin_ast::{Node, NodeMetadata};

/// function literal
#[derive(Debug, Clone)]
pub struct Function<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub params: Vec<String>,
    pub body: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Function<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new Function node with default metadata.
    pub fn new(params: Vec<String>, body: E) -> Self {
        Self {
            params, body,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Function<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params && self.body == other.body && self.metadata == other.metadata
    }
}

impl<E, M> Node for Function<E, M>
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
            params: self.params, body: self.body,
            metadata,
        }
    }
}
