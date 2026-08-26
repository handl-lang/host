//! Canonical `Call` AST-node plugin.
//!
//! Provides the `Call` concrete node shape. The node implements
//! `handl_ast::Node`, the base capability trait from the generic AST crate.

use handl_ast::{Node, NodeMetadata};

/// A canonical function-call expression node: `name(arg1, arg2, ...)`.
///
/// `E` is the kit's expression node type; it may be a single node type
/// such as the canonical `Expr` that contains `Call` as one shape.
#[derive(Debug, Clone)]
pub struct Call<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// Name of the function being called.
    pub name: String,
    /// Argument expressions.
    pub args: Vec<E>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Call<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new call node with default metadata.
    pub fn new<S>(name: S, args: Vec<E>) -> Self
    where
        S: Into<String>,
    {
        Self {
            name: name.into(),
            args,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Call<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.args == other.args && self.metadata == other.metadata
    }
}

impl<E, M> Node for Call<E, M>
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
            args: self.args,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Leaf(String);

    impl Node for Leaf {
        type Value = String;
        type Metadata = ();

        fn metadata(&self) -> &Self::Metadata {
            &()
        }

        fn with_metadata(self, _metadata: Self::Metadata) -> Self
        where
            Self: Sized,
        {
            self
        }
    }

    #[test]
    fn constructs_call() {
        let c = Call::<Leaf, ()>::new("foo", vec![Leaf("a".into())]);
        assert_eq!(c.name, "foo");
        assert_eq!(c.args.len(), 1);
        assert_eq!(c.args[0], Leaf("a".into()));
    }

    #[test]
    fn node_metadata_round_trip() {
        let c = Call::<Leaf, ()>::new("bar", vec![]);
        let c = c.with_metadata(());
        assert_eq!(c.metadata(), &());
    }
}
