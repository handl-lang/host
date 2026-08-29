use handl_kit_canonical::{LexDelimiter, LexKind, LexLexer, LexOperator};
use handl_platform::PlatformError;
use handl_plugin_lexer::{DefaultLexError, Lexer, LexError, Token, TokenKind};

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
                "type".into(),
                "parameter".into(),
                "property".into(),
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
///
/// This is the context-unaware variant: it only sees a single token kind.
/// Context-aware classifiers should implement [`ContextClassifier`] instead.
pub trait TokenClassifier<K: TokenKind> {
    /// Returns `(token_type_index, modifier_bit_flags)` for the given token kind,
    /// or `None` if the token should be skipped (e.g. whitespace, EOF).
    fn classify(&self, kind: &K, legend: &SemanticTokenLegend) -> Option<(u32, u32)>;
}

/// Context-aware token classifier that can see the full token stream and the
/// source text, so it can disambiguate identifiers based on their role
/// (function call, type reference, parameter, property, binding declaration).
pub trait ContextClassifier<K: TokenKind> {
    /// Returns `(token_type_index, modifier_bit_flags)` for the token at
    /// `index` within `tokens`, or `None` if the token should be skipped.
    fn classify(
        &self,
        tokens: &[Token<K>],
        index: usize,
        source: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)>;
}

/// Classifier for `LexKind` — maps canonical token variants to LSP
/// semantic token types using the legend's named indices.
///
/// Implements both [`TokenClassifier`] (context-unaware, for backward
/// compatibility with unit tests that classify a single kind in isolation)
/// and [`ContextClassifier`] (context-aware, used by [`BasicSemanticTokens`]).
pub struct CanonicalTokenClassifier;

impl CanonicalTokenClassifier {
    /// Context-unaware classification based solely on the token kind.
    ///
    /// All identifiers map to `"variable"` here; the context-aware
    /// [`ContextClassifier`] impl refines identifiers by role.
    fn classify_simple(
        &self,
        kind: &LexKind,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        let type_name = match kind {
            LexKind::Number(_) => "number",
            LexKind::String(_) => "string",
            LexKind::Comment(_) => "comment",
            LexKind::DocComment(_) => "comment",
            LexKind::Bool(_) => "keyword",
            LexKind::Null => "keyword",
            LexKind::Keyword(_) => "keyword",
            LexKind::Operator(_) => "operator",
            LexKind::Delimiter(_) => "punctuation",
            LexKind::Identifier(_) => "variable",
            LexKind::Template(_) => "string",
            LexKind::Eof => return None,
        };

        let ty = legend.token_type_index(type_name)?;

        let mods = match kind {
            LexKind::DocComment(_) => {
                legend.modifier_bit("documentation").unwrap_or(0)
            }
            _ => 0,
        };

        Some((ty, mods))
    }
}

impl TokenClassifier<LexKind> for CanonicalTokenClassifier {
    fn classify(
        &self,
        kind: &LexKind,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        self.classify_simple(kind, legend)
    }
}

impl ContextClassifier<LexKind> for CanonicalTokenClassifier {
    fn classify(
        &self,
        tokens: &[Token<LexKind>],
        index: usize,
        source: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        let kind = &tokens[index].kind;

        // Non-identifier tokens use the simple, context-unaware mapping.
        if !matches!(kind, LexKind::Identifier(_)) {
            return self.classify_simple(kind, legend);
        }

        let ident = match kind {
            LexKind::Identifier(s) => s.as_str(),
            _ => unreachable!(),
        };

        // Type reference: identifiers prefixed with `type.` (e.g. type.string).
        if ident.starts_with("type.") {
            let ty = legend.token_type_index("type")?;
            return Some((ty, 0));
        }

        // PascalCase type names (e.g. Flag, Command) — starts with an
        // uppercase letter and has at least one more character.
        if ident
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
            && ident.chars().count() > 1
            && !ident.contains('.')
        {
            let ty = legend.token_type_index("type")?;
            return Some((ty, 0));
        }

        let next = tokens.get(index + 1).map(|t| &t.kind);
        let prev = if index > 0 {
            Some(&tokens[index - 1].kind)
        } else {
            None
        };

        // Function call: the next token is an opening parenthesis.
        if matches!(next, Some(LexKind::Delimiter(LexDelimiter::ParenOpen))) {
            let ty = legend.token_type_index("function")?;
            return Some((ty, 0));
        }

        // A `key:` pair inside parentheses is a parameter; inside braces it is
        // a property. We determine the enclosing bracket by scanning backwards
        // for the nearest unmatched opening delimiter, which disambiguates
        // comma-separated pairs in `(a: T, b: T)` vs `{ a: T, b: T }`.
        if matches!(next, Some(LexKind::Delimiter(LexDelimiter::Colon))) {
            match enclosing_open_delim(tokens, index) {
                Some(LexDelimiter::ParenOpen) => {
                    let ty = legend.token_type_index("parameter")?;
                    return Some((ty, 0));
                }
                Some(LexDelimiter::BraceOpen) => {
                    let ty = legend.token_type_index("property")?;
                    return Some((ty, 0));
                }
                _ => {}
            }
        }

        // Binding declaration: at the start of a line and followed by `:` or `=`
        // (and not inside a paren/brace list, which is handled above).
        if is_at_line_start(tokens, index, source)
            && matches!(
                next,
                Some(LexKind::Delimiter(LexDelimiter::Colon))
                    | Some(LexKind::Operator(LexOperator::Equal))
            )
        {
            let ty = legend.token_type_index("variable")?;
            let mods = legend.modifier_bit("declaration").unwrap_or(0);
            return Some((ty, mods));
        }

        // Suppress unused warning for `prev` while keeping it available for
        // future heuristics.
        let _ = prev;

        // Default: a plain variable reference.
        let ty = legend.token_type_index("variable")?;
        Some((ty, 0))
    }
}

/// Returns the kind of the nearest unmatched opening delimiter enclosing the
/// token at `index`, by scanning backwards through `tokens`.
///
/// Used to decide whether a `key:` pair is a parameter (inside `(...)`) or a
/// property (inside `{ ... }`). Returns `None` if the token is not inside any
/// parenthesis or brace group.
fn enclosing_open_delim(tokens: &[Token<LexKind>], index: usize) -> Option<LexDelimiter> {
    let mut depth_paren: i32 = 0;
    let mut depth_brace: i32 = 0;
    let mut i = index;
    while i > 0 {
        i -= 1;
        match &tokens[i].kind {
            LexKind::Delimiter(LexDelimiter::ParenClose) => depth_paren += 1,
            LexKind::Delimiter(LexDelimiter::BraceClose) => depth_brace += 1,
            LexKind::Delimiter(LexDelimiter::ParenOpen) => {
                if depth_paren == 0 {
                    return Some(LexDelimiter::ParenOpen);
                }
                depth_paren -= 1;
            }
            LexKind::Delimiter(LexDelimiter::BraceOpen) => {
                if depth_brace == 0 {
                    return Some(LexDelimiter::BraceOpen);
                }
                depth_brace -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Returns `true` if the token at `index` is the first token on its source line.
///
/// Compares the gap between the previous token's end and the current token's
/// start; if that gap contains a newline, the current token begins a new line.
fn is_at_line_start(tokens: &[Token<LexKind>], index: usize, source: &str) -> bool {
    if index == 0 {
        return true;
    }
    let prev_end = tokens[index - 1].span.end;
    let curr_start = tokens[index].span.start;
    if prev_end >= source.len() || curr_start > source.len() {
        return true;
    }
    source[prev_end..curr_start].contains('\n')
}

/// A semantic tokens provider that uses any lexer and a context-aware token
/// classifier.
///
/// Generic over the token kind `K`, the classifier `C`, and the lexer `L`,
/// so any kit/lexer combo can provide semantic tokens by implementing
/// [`ContextClassifier`] and [`Lexer`].
pub struct BasicSemanticTokens<K: TokenKind, C: ContextClassifier<K>, L: Lexer<K, E>, E: LexError> {
    lexer: L,
    classifier: C,
    legend: SemanticTokenLegend,
    _marker: std::marker::PhantomData<(K, E)>,
}

impl<C: ContextClassifier<LexKind>, L: Lexer<LexKind, DefaultLexError>>
    BasicSemanticTokens<LexKind, C, L, DefaultLexError>
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

impl<C: ContextClassifier<LexKind> + Send + Sync, L: Lexer<LexKind, DefaultLexError> + Send + Sync>
    SemanticTokensPlugin for BasicSemanticTokens<LexKind, C, L, DefaultLexError>
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

        for (i, token) in tokens.iter().enumerate() {
            if token.kind.is_eof() {
                continue;
            }

            let (token_type, modifiers) =
                match self.classifier.classify(&tokens, i, &request.source, &self.legend) {
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
) -> BasicSemanticTokens<LexKind, CanonicalTokenClassifier, LexLexer, DefaultLexError> {
    BasicSemanticTokens::new(
        LexLexer,
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
