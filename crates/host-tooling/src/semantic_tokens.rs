use handl_kit_canonical::{Kit, SpannedToken, TokenClasses};
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
    fn classify(&self, kind: &str, legend: &SemanticTokenLegend) -> Option<(u32, u32)>;
}

/// Context-aware token classifier that can see the full token stream and the
/// source text, so it can disambiguate identifiers based on their role
/// (function call, type reference, parameter, property, binding declaration).
pub trait ContextClassifier {
    /// Returns `(token_type_index, modifier_bit_flags)` for the token at
    /// `index` within `tokens`, or `None` if the token should be skipped.
    fn classify(
        &self,
        tokens: &[SpannedToken],
        index: usize,
        source: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)>;
}

/// Classifier that maps token kinds to LSP semantic token types using the
/// legend's named indices, driven by the [`TokenClasses`] tables built from
/// the language's `TokenKindDef::category`/`pair` data (ADR-0044 — no
/// hardcoded kind strings).
///
/// Implements both [`TokenClassifier`] (context-unaware, for backward
/// compatibility with unit tests that classify a single kind in isolation)
/// and [`ContextClassifier`] (context-aware, used by [`BasicSemanticTokens`]).
pub struct CanonicalTokenClassifier {
    classes: TokenClasses,
}

impl CanonicalTokenClassifier {
    /// Build a classifier over the given token classification tables.
    pub fn new(classes: TokenClasses) -> Self {
        Self { classes }
    }

    /// Build a classifier from a [`handl_language::Language`]'s token data.
    pub fn for_language(lang: &handl_language::Language) -> Self {
        Self::new(TokenClasses::new(lang))
    }

    /// Build a classifier for the compiled-in canonical kit's language.
    pub fn canonical() -> Self {
        let kit = Kit::new().expect("canonical kit should assemble");
        Self::new(TokenClasses::new(kit.language()))
    }

    /// Context-unaware classification based solely on the token kind's
    /// category.
    ///
    /// All identifiers map to `"variable"` here; the context-aware
    /// [`ContextClassifier`] impl refines identifiers by role.
    fn classify_simple(&self, kind: &str, legend: &SemanticTokenLegend) -> Option<(u32, u32)> {
        let c = &self.classes;
        let type_name = if c.is_number(kind) {
            "number"
        } else if c.is_string(kind) || c.is_template(kind) {
            "string"
        } else if c.is_comment(kind) {
            "comment"
        } else if c.is_bool(kind) || c.is_null(kind) {
            "constant"
        } else if c.is_keyword(kind) {
            "keyword"
        } else if c.is_operator(kind) {
            "operator"
        } else if c.is_delimiter(kind) {
            "punctuation"
        } else if c.is_identifier(kind) {
            "variable"
        } else {
            return None;
        };

        let ty = legend.token_type_index(type_name)?;

        let mods = if c.is_doc_comment(kind) {
            legend.modifier_bit("documentation").unwrap_or(0)
        } else {
            0
        };

        Some((ty, mods))
    }
}

impl TokenClassifier for CanonicalTokenClassifier {
    fn classify(&self, kind: &str, legend: &SemanticTokenLegend) -> Option<(u32, u32)> {
        self.classify_simple(kind, legend)
    }
}

impl ContextClassifier for CanonicalTokenClassifier {
    fn classify(
        &self,
        tokens: &[SpannedToken],
        index: usize,
        source: &str,
        legend: &SemanticTokenLegend,
    ) -> Option<(u32, u32)> {
        let token = &tokens[index];
        let kind = token.kind.as_str();

        let classes = &self.classes;

        // Non-identifier tokens use the simple, context-unaware mapping
        // (brackets classify as punctuation via `is_delimiter`).
        if !classes.is_identifier(kind) {
            return self.classify_simple(kind, legend);
        }

        let ident = token.text.as_str();

        // Helper: check if the token at index spells `label` (resolved
        // against the language's token labels — e.g. `"."` → the
        // member-access token kind, however it is named).
        let is_label =
            |i: usize, l: &str| -> bool { tokens.get(i).map(|t| classes.is_label(&t.kind, l)).unwrap_or(false) };

        // Property access: preceded by a dot → this is a property/member.
        // e.g. `state.acc`, `state.remaining`, `input.0`
        if index > 0 && is_label(index - 1, ".") {
            // If followed by `(`, it's a method call: `state.f(h)`, `arr.map(...)`
            if is_label(index + 1, "(") {
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
        if ident == "type" && is_label(index + 1, ".") {
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
        if is_label(index + 1, "(") {
            let ty = legend.token_type_index("function")?;
            return Some((ty, 0));
        }

        // Dotted function call: `ident . ident (` — the first identifier
        // is a module name in a qualified call like `crypto.sha256(...)`.
        // Classify the module as `variable`, the callee as `method`.
        if is_label(index + 1, ".")
            && tokens
                .get(index + 2)
                .map(|t| classes.is_identifier(&t.kind))
                .unwrap_or(false)
            && is_label(index + 3, "(")
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
        if is_label(index + 1, ":") {
            match enclosing_open_delim(classes, tokens, index) {
                Some(open) if classes.is_label(open, "(") => {
                    let ty = legend.token_type_index("parameter")?;
                    return Some((ty, 0));
                }
                Some(open) if classes.is_label(open, "{") => {
                    let ty = legend.token_type_index("property")?;
                    return Some((ty, 0));
                }
                _ => {}
            }
        }

        // Binding declaration: at the start of a line and followed by `:` or `::`
        // (and not inside a paren/brace list, which is handled above).
        if is_at_line_start(tokens, index, source)
            && (is_label(index + 1, ":") || is_label(index + 1, "::"))
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
/// token at `index`, by scanning backwards through `tokens`, resolved via the
/// language's `open-bracket`/`close-bracket` categories and `pair` links.
///
/// Used to decide whether a `key:` pair is a parameter (inside `(...)`) or a
/// property (inside `{ ... }`). Returns `None` if the token is not inside any
/// bracket group.
fn enclosing_open_delim<'a>(
    classes: &TokenClasses,
    tokens: &'a [SpannedToken],
    index: usize,
) -> Option<&'a str> {
    // open-kind → count of seen closers expecting it.
    let mut depth: std::collections::HashMap<&str, i32> = std::collections::HashMap::new();
    let mut i = index;
    while i > 0 {
        i -= 1;
        let kind = tokens[i].kind.as_str();
        if classes.is_close_bracket(kind) {
            if let Some(open) = classes.pair_of(kind) {
                *depth.entry(open).or_insert(0) += 1;
            }
        } else if classes.is_open_bracket(kind) {
            match depth.get_mut(kind) {
                Some(d) if *d > 0 => *d -= 1,
                _ => return Some(kind),
            }
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
    BasicSemanticTokens::new(
        CanonicalTokenClassifier::canonical(),
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
