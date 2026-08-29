use handl_plugin_ast::{Node, NodeMetadata};

/// A single handler clause in a `handle` expression.
#[derive(Debug, Clone)]
pub struct HandlerArm<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub op_name: String,
    pub params: Vec<String>,
    pub continuation_name: String,
    pub body: E,
    pub metadata: M,
}

/// handle expression
#[derive(Debug, Clone)]
pub struct Handle<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub expr: E,
    pub handlers: Vec<HandlerArm<E, M>>,
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
            handlers: Vec::new(),
            metadata: M::default(),
        }
    }

    /// Creates a new Handle node with handlers.
    pub fn with_handlers(expr: E, handlers: Vec<HandlerArm<E, M>>) -> Self {
        Self {
            expr,
            handlers,
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

impl<E, M> PartialEq for HandlerArm<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.op_name == other.op_name && self.body == other.body && self.metadata == other.metadata
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
            handlers: self.handlers,
            metadata,
        }
    }
}
