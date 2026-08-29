/// A runtime value carried by a kit node.
#[derive(Debug, Clone, PartialEq)]
pub enum KitValue {
    /// A numeric value.
    Number(f64),
    /// A string literal.
    String(String),
    /// A boolean value.
    Bool(bool),
    /// The `null` value.
    Null,
}
