use handl_plugin_ast::{Node, NodeMetadata};

/// A single match arm: pattern, optional guard, body.
#[derive(Debug, Clone)]
pub struct MatchArm<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub pattern: E,
    pub guard: Option<E>,
    pub body: E,
    pub metadata: M,
}

/// match expression
#[derive(Debug, Clone)]
pub struct Match<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    pub subject: E,
    pub arms: Vec<MatchArm<E, M>>,
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
            arms: Vec::new(),
            metadata: M::default(),
        }
    }

    /// Creates a new Match node with arms.
    pub fn with_arms(subject: E, arms: Vec<MatchArm<E, M>>) -> Self {
        Self {
            subject,
            arms,
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

impl<E, M> PartialEq for MatchArm<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.body == other.body && self.metadata == other.metadata
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
            arms: self.arms,
            metadata,
        }
    }
}
