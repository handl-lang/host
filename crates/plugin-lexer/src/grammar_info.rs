use serde::{Deserialize, Serialize};

/// A description of a kit's lexical grammar, extracted from its token plugins.
///
/// This is the bridge between the dynamic plugin system and static grammar
/// consumers (TextMate grammars, tree-sitter configs, etc.). Any kit that
/// wants to generate editor grammars produces this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarInfo {
    /// Unique scope name, e.g. `"source.handl"`.
    pub scope_name: String,
    /// File extensions, e.g. `["hnl"]`.
    pub file_extensions: Vec<String>,
    /// Keyword strings, e.g. `["let", "const", "match", ...]`.
    pub keywords: Vec<String>,
    /// Boolean literal strings, e.g. `["true", "false"]`.
    pub booleans: Vec<String>,
    /// Null literal strings, e.g. `["null"]`.
    pub nulls: Vec<String>,
    /// Operator strings, e.g. `["|>", "->", "=", "+", ...]`.
    pub operators: Vec<String>,
    /// Single-character delimiter strings, e.g. `["(", ")", "{", ...]`.
    pub delimiters: Vec<String>,
    /// Line comment prefix, e.g. `"//"`.
    pub line_comment_start: Option<String>,
    /// Block comment start/end, e.g. `("/*", "*/")`.
    pub block_comment: Option<(String, String)>,
    /// Doc line comment prefix, e.g. `"///"`.
    pub doc_line_comment_start: Option<String>,
    /// Doc block comment start/end, e.g. `("/**", "*/")`.
    pub doc_block_comment: Option<(String, String)>,
    /// String delimiter characters, e.g. `['"', '\'']`.
    pub string_delimiters: Vec<char>,
    /// Regex pattern for number literals, e.g. `r"-?\d+(\.\d+)?([eE][+-]?\d+)?"`.
    pub number_regex: String,
    /// Regex pattern for identifiers, e.g. `r"[a-zA-Z_][a-zA-Z0-9_]*"`.
    pub identifier_regex: String,
}

/// Trait that any kit can implement to expose its lexical grammar metadata.
/// This decouples grammar generation from any specific kit implementation.
pub trait GrammarInfoProvider {
    fn grammar_info(&self) -> GrammarInfo;
}

/// Trait for kit-level metadata that is not derivable from the lexer config.
///
/// This includes the TextMate scope name and file extensions, which are
/// editorial conventions rather than lexical properties. Kits implement this
/// to provide values for [`GrammarInfo::scope_name`] and
/// [`GrammarInfo::file_extensions`].
pub trait KitGrammarMetadata {
    /// Returns the TextMate scope name, e.g. `"source.handl"`.
    fn scope_name(&self) -> &str;

    /// Returns the file extensions, e.g. `["hnl"]`.
    fn file_extensions(&self) -> Vec<String>;
}
