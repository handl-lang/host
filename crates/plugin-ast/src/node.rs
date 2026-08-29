use std::any::Any;
use std::fmt::Debug;

use crate::NodeMetadata;

/// The single generic AST node abstraction.
///
/// Every concrete node type in a plugin or kit implements `Node`.
/// A node may also implement zero or more capability traits (`Expr`,
/// `Stmt`, `Pattern`, `Type`, `Field`) depending on its shape.
pub trait Node: Debug + Clone + PartialEq {
    /// The runtime value type stored in this node (literals, constants, ...).
    type Value;

    /// The metadata attached to this node.
    type Metadata: NodeMetadata;

    /// Returns the metadata attached to this node.
    fn metadata(&self) -> &Self::Metadata;

    /// Returns this node with the given metadata.
    fn with_metadata(self, metadata: Self::Metadata) -> Self
    where
        Self: Sized;
}

/// Object-safe node trait for dynamic composition.
pub trait DynNode: Debug + Send + Sync + 'static {
    fn metadata(&self) -> &dyn Any;
    fn kind(&self) -> &str;
    fn dyn_clone(&self) -> Box<dyn DynNode>;
    fn as_any(&self) -> &dyn Any;
}

impl<T> DynNode for T
where
    T: Node + Send + Sync + 'static,
    T::Metadata: 'static,
{
    fn metadata(&self) -> &dyn Any {
        Node::metadata(self)
    }

    fn kind(&self) -> &str {
        std::any::type_name::<T>()
    }

    fn dyn_clone(&self) -> Box<dyn DynNode> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
