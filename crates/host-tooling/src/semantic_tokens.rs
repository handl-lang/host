use handl_kit_canonical::{CanonicalKind, Kit};
use handl_platform::PlatformError;
use handl_plugin_lexer::TokenKind;

/// LSP semantic token types (indices match the LSP protocol order).
pub const TOKEN_TYPES: &[&str] = &[
    "keyword",
    "number",
    "string",
    "comment",
    "operator",
    "punctuation",
    "variable",
    "function",
];

/// LSP semantic token modifiers (bit flags).
pub const TOKEN_MODIFIERS: &[&str] = &["declaration", "documentation"];

/// A single semantic token: delta-encoded line, start char, length, type, modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticToken {
    /// Line offset relative to the previous token (delta line).
    pub delta_line: u32,
    /// Character offset relative to the previous token on the same line (delta start).
    pub delta_start: u32,
    /// Token length in characters.
    pub length: u32,
    /// Token type index into [`TOKEN_TYPES`].
    pub token_type: u32,
    /// Token modifier bit flags.
    pub token_modifiers: u32,
}

/// Request for semantic tokens of a document.
#[derive(Debug, Clone)]
pub struct SemanticTokensRequest {
    pub source: String,
}

/// Response containing the semantic tokens (delta-encoded array).
#[derive(Debug, Clone)]
pub struct SemanticTokensResponse(pub Vec<SemanticToken>);

/// Plugin contract for `textDocument/semanticTokens`.
pub trait SemanticTokensPlugin: Send + Sync {
    fn run(&self, request: &SemanticTokensRequest)
    -> Result<SemanticTokensResponse, PlatformError>;
}

/// A semantic tokens provider that uses the canonical kit's lexer.
///
/// Lexes the source, maps each `CanonicalKind` to an LSP token type, and
/// produces delta-encoded tokens as specified by the LSP protocol.
pub struct BasicSemanticTokens {
    kit: Kit,
}

impl BasicSemanticTokens {
    pub fn new(kit: Kit) -> Self {
        Self { kit }
    }
}

impl SemanticTokensPlugin for BasicSemanticTokens {
    fn run(
        &self,
        request: &SemanticTokensRequest,
    ) -> Result<SemanticTokensResponse, PlatformError> {
        let tokens = self
            .kit
            .lex(&request.source)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let mut result = Vec::new();
        let mut prev_line: u32 = 0;
        let mut prev_start: u32 = 0;

        for token in &tokens {
            if token.kind.is_eof() {
                continue;
            }

            let (token_type, modifiers) = map_kind(&token.kind);
            let length = (token.span.end - token.span.start) as u32;

            // Convert byte offsets to line:character.
            let (line, start_char) = byte_offset_to_line_char(&request.source, token.span.start);

            let delta_line = line - prev_line;
            let delta_start = if delta_line == 0 {
                start_char - prev_start
            } else {
                start_char
            };

            result.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers: modifiers,
            });

            prev_line = line;
            prev_start = start_char;
        }

        Ok(SemanticTokensResponse(result))
    }
}

fn map_kind(kind: &CanonicalKind) -> (u32, u32) {
    let ty = match kind {
        CanonicalKind::Number(_) => 1,     // number
        CanonicalKind::String(_) => 2,     // string
        CanonicalKind::Comment(_) => 3,    // comment
        CanonicalKind::DocComment(_) => 3, // comment (with documentation modifier)
        CanonicalKind::Bool(_) => 0,       // keyword
        CanonicalKind::Null => 0,          // keyword
        CanonicalKind::Keyword(_) => 0,    // keyword
        CanonicalKind::Operator(_) => 4,   // operator
        CanonicalKind::Delimiter(_) => 5,  // punctuation
        CanonicalKind::Identifier(_) => 6, // variable
        CanonicalKind::Template(_) => 2,   // string
        CanonicalKind::Eof => 6,           // shouldn't reach here
    };

    let mods = match kind {
        CanonicalKind::DocComment(_) => 0b10, // documentation modifier (bit 1)
        _ => 0,
    };

    (ty, mods)
}

fn byte_offset_to_line_char(source: &str, byte_offset: usize) -> (u32, u32) {
    let mut line: u32 = 0;
    let mut last_line_start: usize = 0;

    for (i, ch) in source.char_indices() {
        if i >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            last_line_start = i + 1;
        }
    }

    // Character offset within the line (not byte offset).
    let char_offset = source[last_line_start..byte_offset.min(source.len())]
        .chars()
        .count() as u32;

    (line, char_offset)
}
