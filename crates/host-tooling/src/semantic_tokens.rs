use handl_kit_canonical::{
    Kit, SpannedToken, is_bool_kind, is_comment_kind, is_delimiter_kind, is_doc_comment_kind,
    is_identifier_kind, is_keyword_kind, is_null_kind, is_number_kind, is_operator_kind,
    is_string_kind, is_template_kind,
};
use handl_language::{Language, TokenCategory as TC};
use handl_platform::PlatformError;

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
                "method".into(),
                "constant".into(),
            ],
            token_modifiers: vec![
                "declaration".into(),
                "documentation".into(),
                "readonly".into(),
                "static".into(),
            ],
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

    fn run(&self, request: &SemanticTokensRequest)
    -> Result<SemanticTokensResponse, PlatformError>;
}

/// Trait that maps a token kind string to a semantic token type index and
/// modifier bit flags, using a [`SemanticTokenLegend`].
///
/// This is the context-unaware variant: it only sees a single token kind.
/// Context-aware classifiers should implement [`ContextClassifier`] instead.
pub trait TokenClassifier {
    /// Returns `(token_type_index, modifier_bit_flags)` for the given token kind,
    /// or `None` if the token should be skipped (e.g. whitespace, EOF).
    ///
    /// `lang` supplies the token-kind classification data
    /// (`Language::token_category`, ADR-0081).
    fn classify(
        &self,
        lang: &Language,
        kind: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)>;
}

/// Context-aware token classifier that can see the full token stream and the
/// source text, so it can disambiguate identifiers based on their role
/// (function call, type reference, parameter, property, binding declaration).
pub trait ContextClassifier {
    /// Returns `(token_type_index, modifier_bit_flags)` for the token at
    /// `index` within `tokens`, or `None` if the token should be skipped.
    fn classify(
        &self,
        lang: &Language,
        tokens: &[SpannedToken],
        index: usize,
        source: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)>;
}

/// Classifier for canonical token kinds — maps string token kinds to LSP
/// semantic token types using the legend's named indices.
///
/// Implements both [`TokenClassifier`] (context-unaware, for backward
/// compatibility with unit tests that classify a single kind in isolation)
/// and [`ContextClassifier`] (context-aware, used by [`BasicSemanticTokens`]).
pub struct CanonicalTokenClassifier;

impl CanonicalTokenClassifier {
    /// Context-unaware classification based solely on the token kind string.
    ///
    /// All identifiers map to `"variable"` here; the context-aware
    /// [`ContextClassifier`] impl refines identifiers by role.
    fn classify_simple(
        &self,
        lang: &Language,
        kind: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        let type_name = if is_number_kind(lang, kind) {
            "number"
        } else if is_string_kind(lang, kind) {
            "string"
        } else if is_comment_kind(lang, kind) {
            "comment"
        } else if is_bool_kind(lang, kind) || is_null_kind(lang, kind) {
            "constant"
        } else if is_keyword_kind(lang, kind) {
            "keyword"
        } else if is_operator_kind(lang, kind) {
            "operator"
        } else if is_delimiter_kind(lang, kind) {
            "punctuation"
        } else if is_identifier_kind(lang, kind) {
            "variable"
        } else if is_template_kind(lang, kind) {
            "string"
        } else {
            return None;
        };

        let ty = legend.token_type_index(type_name)?;

        let mods = if is_doc_comment_kind(lang, kind) {
            legend.modifier_bit("documentation").unwrap_or(0)
        } else {
            0
        };

        Some((ty, mods))
    }
}

impl TokenClassifier for CanonicalTokenClassifier {
    fn classify(
        &self,
        lang: &Language,
        kind: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        self.classify_simple(lang, kind, legend)
    }
}

impl ContextClassifier for CanonicalTokenClassifier {
    fn classify(
        &self,
        lang: &Language,
        tokens: &[SpannedToken],
        index: usize,
        source: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        let token = &tokens[index];
        let kind = token.kind.as_str();

        // Non-identifier tokens use the simple, context-unaware mapping.
        if !is_identifier_kind(lang, kind) {
            // Classify brackets/braces/parens as punctuation (not operator).
            if lang
                .token_category(kind)
                .map(|c| c.is_bracket())
                .unwrap_or(false)
            {
                let ty = legend.token_type_index("punctuation")?;
                return Some((ty, 0));
            }
            return self.classify_simple(lang, kind, legend);
        }

        let ident = token.text.as_str();

        // Helper: check if token at index has a given category.
        let is_kind = |i: usize, c: TC| -> bool {
            tokens.get(i).map(|t| lang.token_is(&t.kind, c)).unwrap_or(false)
        };

        // Property access: preceded by a dot → this is a property/member.
        // e.g. `state.acc`, `state.remaining`, `input.0`
        if index > 0 && is_kind(index - 1, TC::Dot) {
            // If followed by `(`, it's a method call: `state.f(h)`, `arr.map(...)`
            if is_kind(index + 1, TC::OpenParen) {
                let ty = legend.token_type_index("method")?;
                return Some((ty, 0));
            }
            // Otherwise it's a property access.
            let ty = legend.token_type_index("property")?;
            return Some((ty, 0));
        }

        // Type reference: the identifier `type` followed by a dot (e.g.
        // `type.str`, `type.string`). The lexer splits `type.str` into
        // separate tokens, so we check the lookahead pattern.
        if ident == "type" && is_kind(index + 1, TC::Dot) {
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

        // Function call: the next token is an opening parenthesis.
        if is_kind(index + 1, TC::OpenParen) {
            let ty = legend.token_type_index("function")?;
            return Some((ty, 0));
        }

        // Dotted function call: `ident . ident (` — the first identifier
        // is a module name in a qualified call like `crypto.sha256(...)`.
        // Classify the module as `variable`, the callee as `method`.
        if is_kind(index + 1, TC::Dot)
            && tokens
                .get(index + 2)
                .map(|t| is_identifier_kind(lang, &t.kind))
                .unwrap_or(false)
            && is_kind(index + 3, TC::OpenParen)
        {
            // The callee (index+2) is a method call on this module.
            // This identifier is the module/namespace.
            let ty = legend.token_type_index("variable")?;
            return Some((ty, 0));
        }

        // A `key:` pair inside parentheses is a parameter; inside braces it is
        // a property. We determine the enclosing bracket by scanning backwards
        // for the nearest unmatched opening delimiter, which disambiguates
        // comma-separated pairs in `(a: T, b: T)` vs `{ a: T, b: T }`.
        if is_kind(index + 1, TC::Colon) {
            match enclosing_open_delim(lang, tokens, index) {
                Some(TC::OpenParen) => {
                    let ty = legend.token_type_index("parameter")?;
                    return Some((ty, 0));
                }
                Some(TC::OpenBrace) => {
                    let ty = legend.token_type_index("property")?;
                    return Some((ty, 0));
                }
                _ => {}
            }
        }

        // Binding declaration: at the start of a line and followed by `:` or `::`
        // (and not inside a paren/brace list, which is handled above).
        if is_at_line_start(tokens, index, source)
            && (is_kind(index + 1, TC::Colon) || is_kind(index + 1, TC::Bind))
        {
            let ty = legend.token_type_index("variable")?;
            let mods = legend.modifier_bit("declaration").unwrap_or(0);
            return Some((ty, mods));
        }

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
fn enclosing_open_delim(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
) -> Option<TC> {
    let mut depth_paren: i32 = 0;
    let mut depth_brace: i32 = 0;
    let mut i = index;
    while i > 0 {
        i -= 1;
        match lang.token_category(tokens[i].kind.as_str()) {
            Some(TC::CloseParen) => depth_paren += 1,
            Some(TC::CloseBrace) => depth_brace += 1,
            Some(TC::OpenParen) => {
                if depth_paren == 0 {
                    return Some(TC::OpenParen);
                }
                depth_paren -= 1;
            }
            Some(TC::OpenBrace) => {
                if depth_brace == 0 {
                    return Some(TC::OpenBrace);
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
fn is_at_line_start(tokens: &[SpannedToken], index: usize, source: &str) -> bool {
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

/// A semantic tokens provider that uses the canonical kit lexer and a
/// context-aware token classifier.
pub struct BasicSemanticTokens {
    classifier: CanonicalTokenClassifier,
    legend: SemanticTokenLegend,
}

impl BasicSemanticTokens {
    /// Creates a new semantic tokens provider with the given classifier and legend.
    pub fn new(classifier: CanonicalTokenClassifier, legend: SemanticTokenLegend) -> Self {
        Self { classifier, legend }
    }
}

impl SemanticTokensPlugin for BasicSemanticTokens {
    fn legend(&self) -> SemanticTokenLegend {
        self.legend.clone()
    }

    fn run(
        &self,
        request: &SemanticTokensRequest,
    ) -> Result<SemanticTokensResponse, PlatformError> {
        let kit = Kit::new().map_err(|e| PlatformError::Plugin(e.to_string()))?;
        let lang = kit.lang();
        let tokens = kit
            .lex_with_spans(&request.source)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let line_starts = compute_line_starts(&request.source);

        let mut result = Vec::new();
        let mut prev_line: u32 = 0;
        let mut prev_start: u32 = 0;

        for (i, token) in tokens.iter().enumerate() {
            let (token_type, modifiers) = match ContextClassifier::classify(
                &self.classifier,
                lang,
                &tokens,
                i,
                &request.source,
                &self.legend,
            ) {
                Some(v) => v,
                None => continue,
            };

            let length = (token.span.end - token.span.start) as u32;
            let (line, start_char) = line_starts.lookup(token.span.start);

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
pub fn canonical_semantic_tokens() -> BasicSemanticTokens {
    BasicSemanticTokens::new(CanonicalTokenClassifier, SemanticTokenLegend::default())
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
