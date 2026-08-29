use handl_plugin_ast::{Node, NodeMetadata};

/// A `module` declaration: `module name` or `module name = value`.
#[derive(Debug, Clone)]
pub struct Module<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// The module name.
    pub name: String,
    /// An optional module body expression.
    pub value: Option<E>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Module<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new `Module` node with default metadata.
    pub fn new(name: String, value: Option<E>) -> Self {
        Self {
            name,
            value,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Module<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value && self.metadata == other.metadata
    }
}

impl<E, M> Node for Module<E, M>
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
            value: self.value,
            metadata,
        }
    }
}
