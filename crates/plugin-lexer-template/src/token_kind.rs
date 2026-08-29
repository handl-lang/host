use handl_plugin_lexer::{TokenKind};

use crate::segment::TemplateSegment;

/// A token kind that carries template string segments.
///
/// Dialects that want template string support should add a variant
/// to their token kind enum and implement this trait.
pub trait TemplateTokenKind: TokenKind {
    /// Constructs a template token kind from the given segments.
    fn template_kind(segments: Vec<TemplateSegment>) -> Self;
}
