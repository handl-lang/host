/// A segment of a template string.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateSegment {
    /// A literal text segment.
    Text(String),
    /// An interpolation expression — the raw text between `${` and `}`.
    Expr(String),
}
