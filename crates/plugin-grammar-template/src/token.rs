use handl_plugin_lexer_template::{TemplateSegment, TemplateTokenKind};

/// Token capability required by the template grammar rule.
///
/// This trait extends [`TemplateTokenKind`] with the ability to
/// extract segments from a template token kind.
pub trait TemplateToken: TemplateTokenKind {
    /// If this token kind is a template string, returns its segments.
    fn as_template_segments(&self) -> Option<&[TemplateSegment]>;
}
