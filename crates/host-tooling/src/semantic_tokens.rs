use handl_kit_canonical::{CanonicalKind, CanonicalLexer};
use handl_platform::PlatformError;
use handl_plugin_lexer::{DefaultLexError, Lexer, LexError, TokenKind};

/// LSP semantic token type names and modifier names, in protocol order.
///
/// The index of each entry is the numeric token type (or modifier bit position)
/// used in the delta-encoded `data` array of the LSP response.
#[derive(Debug, Clone)]
pub struct SemanticTokenLegend {
    pub token_types: Vec<String>,
    pub token_modifiers: Vec<String>,
}

impl SemanticTokenLegend {
    /// Returns the index of a token type by name, or `None` if not found.
    pub fn token_type_index(&self, name: &str) -> Option<u32> {
        self.token_types
            .iter()
            .position(|t| t == name)
            .map(|i| i as u32)
    }

    /// Returns the bit-flag for a modifier by name, or `None` if not found.
    pub fn modifier_bit(&self, name: &str) -> Option<u32> {
        self.token_modifiers
            .iter()
            .position(|m| m == name)
            .map(|i| 1u32 << i)
    }
}

impl Default for SemanticTokenLegend {
    fn default() -> Self {
        Self {
            token_types: vec![
                "keyword".into(),
                "number".into(),
                "string".into(),
                "comment".into(),
                "operator".into(),
                "punctuation".into(),
                "variable".into(),
                "function".into(),
            ],
            token_modifiers: vec!["declaration".into(), "documentation".into()],
        }
    }
}

/// A single semantic token: delta-encoded line, start char, length, type, modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticToken {
    /// Line offset relative to the previous token (delta line).
    pub delta_line: u32,
    /// Character offset relative to the previous token on the same line (delta start).
    pub delta_start: u32,
    /// Token length in characters.
    pub length: u32,
    /// Token type index into the legend's `token_types`.
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
    /// Returns the legend describing token types and modifiers.
    fn legend(&self) -> SemanticTokenLegend;

    fn run(
        &self,
        request: &SemanticTokensRequest,
    ) -> Result<SemanticTokensResponse, PlatformError>;
}

/// Trait that maps a kit's token kind to a semantic token type index and
/// modifier bit flags, using a [`SemanticTokenLegend`].
///
/// This decouples the classification logic from any specific token kind enum.
/// Each kit implements this to define how its tokens map to LSP categories.
pub trait TokenClassifier<K: TokenKind> {
    /// Returns `(token_type_index, modifier_bit_flags)` for the given token kind,
    /// or `None` if the token should be skipped (e.g. whitespace, EOF).
    fn classify(&self, kind: &K, legend: &SemanticTokenLegend) -> Option<(u32, u32)>;
}

/// Classifier for `CanonicalKind` — maps canonical token variants to LSP
/// semantic token types using the legend's named indices.
pub struct CanonicalTokenClassifier;

impl TokenClassifier<CanonicalKind> for CanonicalTokenClassifier {
    fn classify(
        &self,
        kind: &CanonicalKind,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        let type_name = match kind {
            CanonicalKind::Number(_) => "number",
            CanonicalKind::String(_) => "string",
            CanonicalKind::Comment(_) => "comment",
            CanonicalKind::DocComment(_) => "comment",
            CanonicalKind::Bool(_) => "keyword",
            CanonicalKind::Null => "keyword",
            CanonicalKind::Keyword(_) => "keyword",
            CanonicalKind::Operator(_) => "operator",
            CanonicalKind::Delimiter(_) => "punctuation",
            CanonicalKind::Identifier(_) => "variable",
            CanonicalKind::Template(_) => "string",
            CanonicalKind::Eof => return None,
        };

        let ty = legend.token_type_index(type_name)?;

        let mods = match kind {
            CanonicalKind::DocComment(_) => {
                legend.modifier_bit("documentation").unwrap_or(0)
            }
            _ => 0,
        };

        Some((ty, mods))
    }
}

/// A semantic tokens provider that uses any lexer and a token classifier.
///
/// Generic over the token kind `K`, the classifier `C`, and the lexer `L`,
/// so any kit/lexer combo can provide semantic tokens by implementing
/// [`TokenClassifier`] and [`Lexer`].
pub struct BasicSemanticTokens<K: TokenKind, C: TokenClassifier<K>, L: Lexer<K, E>, E: LexError> {
    lexer: L,
    classifier: C,
    legend: SemanticTokenLegend,
    _marker: std::marker::PhantomData<(K, E)>,
}

impl<C: TokenClassifier<CanonicalKind>, L: Lexer<CanonicalKind, DefaultLexError>>
    BasicSemanticTokens<CanonicalKind, C, L, DefaultLexError>
{
    pub fn new(lexer: L, classifier: C, legend: SemanticTokenLegend) -> Self {
        Self {
            lexer,
            classifier,
            legend,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C: TokenClassifier<CanonicalKind> + Send + Sync, L: Lexer<CanonicalKind, DefaultLexError> + Send + Sync>
    SemanticTokensPlugin for BasicSemanticTokens<CanonicalKind, C, L, DefaultLexError>
{
    fn legend(&self) -> SemanticTokenLegend {
        self.legend.clone()
    }

    fn run(
        &self,
        request: &SemanticTokensRequest,
    ) -> Result<SemanticTokensResponse, PlatformError> {
        let tokens = self
            .lexer
            .lex(&request.source)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let line_starts = compute_line_starts(&request.source);

        let mut result = Vec::new();
        let mut prev_line: u32 = 0;
        let mut prev_start: u32 = 0;

        for token in &tokens {
            if token.kind.is_eof() {
                continue;
            }

            let (token_type, modifiers) =
                match self.classifier.classify(&token.kind, &self.legend) {
                    Some(v) => v,
                    None => continue,
                };

            let length = (token.span.end - token.span.start) as u32;
            let (line, start_char) =
                line_starts.lookup(token.span.start);

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

/// Convenience constructor for the default canonical semantic tokens provider.
pub fn canonical_semantic_tokens(
) -> BasicSemanticTokens<CanonicalKind, CanonicalTokenClassifier, CanonicalLexer, DefaultLexError> {
    BasicSemanticTokens::new(
        CanonicalLexer,
        CanonicalTokenClassifier,
        SemanticTokenLegend::default(),
    )
}

/// Precomputed line start offsets for O(log n) line:column lookup.
struct LineStarts {
    /// Byte offset of the start of each line (line 0 starts at offset 0).
    starts: Vec<usize>,
    source: String,
}

impl LineStarts {
    fn lookup(&self, byte_offset: usize) -> (u32, u32) {
        let line = self.starts.partition_point(|&s| s <= byte_offset) - 1;
        let line_start = self.starts[line];
        let char_offset = self.source[line_start..byte_offset.min(self.source.len())]
            .chars()
            .count() as u32;
        (line as u32, char_offset)
    }
}

fn compute_line_starts(source: &str) -> LineStarts {
    let mut starts = vec![0usize];
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(i + 1);
        }
    }
    LineStarts {
        starts,
        source: source.to_string(),
    }
}
