use handl_platform::PlatformError;

/// LSP 0-based line/character position.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// LSP range defined by two positions.
#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// A single LSP text edit.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// A single LSP completion candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: Option<i64>,
    pub insert_text: Option<String>,
    pub insert_text_format: Option<i64>,
    pub text_edit: Option<TextEdit>,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            kind: None,
            insert_text: None,
            insert_text_format: None,
            text_edit: None,
        }
    }
}

/// A source location as a half-open byte range, optionally in another file.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub start: usize,
    pub end: usize,
    /// File URI for cross-file locations; `None` means the current document.
    pub uri: Option<String>,
}

impl Location {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            uri: None,
        }
    }
}

/// A document symbol for outline / breadcrumb views.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u32,
    pub range_start: usize,
    pub range_end: usize,
    pub selection_start: usize,
    pub selection_end: usize,
}

impl DocumentSymbol {
    pub fn new(name: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            name: name.into(),
            detail: None,
            kind: 13, // SymbolKind::Variable
            range_start: start,
            range_end: end,
            selection_start: start,
            selection_end: end,
        }
    }
}

/// A minimal incremental edit.
#[derive(Debug, Clone, PartialEq)]
pub struct TextContentChange {
    /// Byte offset where the replaced range begins.
    pub start: usize,
    /// Byte offset where the replaced range ends.
    pub end: usize,
    /// Text to insert in place of the range.
    pub text: String,
}

impl TextContentChange {
    pub fn insert(pos: usize, text: impl Into<String>) -> Self {
        Self {
            start: pos,
            end: pos,
            text: text.into(),
        }
    }

    pub fn delete(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            text: String::new(),
        }
    }

    pub fn replace(start: usize, end: usize, text: impl Into<String>) -> Self {
        Self {
            start,
            end,
            text: text.into(),
        }
    }
}

// Request / response wrappers used by the LSP plugin traits.

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub source: String,
    pub pos: usize,
    /// Filesystem path of the current document (without the `file://` URI prefix).
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse(pub Vec<CompletionItem>);

#[derive(Debug, Clone)]
pub struct GotoDefRequest {
    pub source: String,
    pub pos: usize,
    /// Filesystem path of the current document (without the `file://` URI prefix).
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct GotoDefResponse(pub Option<Location>);

/// Request for `textDocument/references`.
#[derive(Debug, Clone)]
pub struct ReferencesRequest {
    /// Source text of the document.
    pub source: String,
    /// Cursor byte offset.
    pub pos: usize,
    /// Filesystem path of the current document (without the `file://` URI prefix).
    pub path: String,
    /// Whether the declaration site itself counts as a reference
    /// (`ReferenceContext.includeDeclaration`).
    pub include_declaration: bool,
}

/// Response for `textDocument/references` — byte-span locations.
#[derive(Debug, Clone)]
pub struct ReferencesResponse(pub Vec<Location>);

/// Plugin contract for `textDocument/references`.
pub trait ReferencesPlugin: Send + Sync {
    /// Return every location referencing the symbol at `request.pos`.
    fn run(&self, request: &ReferencesRequest) -> Result<ReferencesResponse, PlatformError>;
}

/// Request for `textDocument/rename`.
#[derive(Debug, Clone)]
pub struct RenameRequest {
    /// Source text of the document.
    pub source: String,
    /// Cursor byte offset.
    pub pos: usize,
    /// Filesystem path of the current document (without the `file://` URI prefix).
    pub path: String,
    /// The new name to substitute at every reference site.
    pub new_name: String,
}

/// Response for `textDocument/rename` — byte-span edits, one per
/// reference site (declaration included).
#[derive(Debug, Clone)]
pub struct RenameResponse(pub Vec<TextEdit>);

/// Plugin contract for `textDocument/rename`.
pub trait RenamePlugin: Send + Sync {
    /// Return the edits renaming the symbol at `request.pos`.
    fn run(&self, request: &RenameRequest) -> Result<RenameResponse, PlatformError>;
}

#[derive(Debug, Clone)]
pub struct DocumentSymbolsRequest(pub String);

#[derive(Debug, Clone)]
pub struct DocumentSymbolsResponse(pub Vec<DocumentSymbol>);

#[derive(Debug, Clone)]
pub struct IncrementalSyncRequest {
    pub source: String,
    pub change: TextContentChange,
}

#[derive(Debug, Clone)]
pub struct IncrementalSyncResponse(pub String);

/// Plugin contract for `textDocument/completion`.
pub trait CompletionPlugin: Send + Sync {
    fn run(&self, request: &CompletionRequest) -> Result<CompletionResponse, PlatformError>;
}

/// Plugin contract for `textDocument/definition`.
pub trait GotoDefPlugin: Send + Sync {
    fn run(&self, request: &GotoDefRequest) -> Result<GotoDefResponse, PlatformError>;
}

/// Plugin contract for `textDocument/documentSymbol`.
pub trait DocumentSymbolsPlugin: Send + Sync {
    fn run(
        &self,
        request: &DocumentSymbolsRequest,
    ) -> Result<DocumentSymbolsResponse, PlatformError>;
}

/// Plugin contract for incremental `textDocument/didChange`.
pub trait IncrementalSyncPlugin: Send + Sync {
    fn run(
        &self,
        request: &IncrementalSyncRequest,
    ) -> Result<IncrementalSyncResponse, PlatformError>;
}

/// A default incremental-sync implementation that simply replaces the
/// requested byte range.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicIncrementalSync;

impl BasicIncrementalSync {
    pub fn new() -> Self {
        Self
    }
}

impl IncrementalSyncPlugin for BasicIncrementalSync {
    fn run(
        &self,
        request: &IncrementalSyncRequest,
    ) -> Result<IncrementalSyncResponse, PlatformError> {
        let IncrementalSyncRequest { source, change } = request;
        let start = change.start.min(source.len());
        let end = change.end.min(source.len());
        let mut out = String::with_capacity(source.len() + change.text.len());
        out.push_str(&source[..start]);
        out.push_str(&change.text);
        out.push_str(&source[end..]);
        Ok(IncrementalSyncResponse(out))
    }
}

/// Request for `textDocument/hover`.
#[derive(Debug, Clone)]
pub struct HoverRequest {
    /// Source text of the document.
    pub source: String,
    /// Cursor byte offset.
    pub pos: usize,
    /// Filesystem path of the document (without the `file://` URI prefix).
    pub path: String,
}

/// Response for `textDocument/hover`.
#[derive(Debug, Clone)]
pub struct HoverResponse(pub Option<String>);

/// Plugin contract for `textDocument/hover`.
pub trait HoverPlugin: Send + Sync {
    /// Return a markdown hover string, or `None` if no hover is available.
    fn run(&self, request: &HoverRequest) -> Result<HoverResponse, PlatformError>;
}

/// Request for `textDocument/signatureHelp`.
#[derive(Debug, Clone)]
pub struct SignatureHelpRequest {
    /// Source text of the document.
    pub source: String,
    /// Cursor byte offset.
    pub pos: usize,
    /// Filesystem path of the document (without the `file://` URI prefix).
    pub path: String,
}

/// A single signature parameter.
#[derive(Debug, Clone)]
pub struct SignatureParameter {
    /// Display label for the parameter.
    pub label: String,
    /// Human-readable documentation for the parameter.
    pub documentation: String,
}

impl SignatureParameter {
    /// Create a parameter description.
    pub fn new(label: impl Into<String>, documentation: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            documentation: documentation.into(),
        }
    }
}

/// A single signature candidate.
#[derive(Debug, Clone)]
pub struct SignatureInformation {
    /// Full signature label, e.g. `comp(a, b)`.
    pub label: String,
    /// Optional human-readable documentation for the signature.
    pub documentation: Option<String>,
    /// Parameters of the signature.
    pub parameters: Vec<SignatureParameter>,
}

impl SignatureInformation {
    /// Create a signature with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            documentation: None,
            parameters: Vec::new(),
        }
    }
}

/// Response for `textDocument/signatureHelp`.
#[derive(Debug, Clone)]
pub struct SignatureHelpResponse {
    /// One or more signatures for the active call.
    pub signatures: Vec<SignatureInformation>,
    /// Zero-based index of the parameter the cursor is on.
    pub active_parameter: usize,
}

impl SignatureHelpResponse {
    /// Create a response with a single signature and active parameter.
    pub fn new(signature: SignatureInformation, active_parameter: usize) -> Self {
        Self {
            signatures: vec![signature],
            active_parameter,
        }
    }

    /// Create an empty response for when no signature help is available.
    pub fn empty() -> Self {
        Self {
            signatures: Vec::new(),
            active_parameter: 0,
        }
    }
}

/// Plugin contract for `textDocument/signatureHelp`.
pub trait SignatureHelpPlugin: Send + Sync {
    /// Return signature information for the call enclosing `request.pos`.
    fn run(&self, request: &SignatureHelpRequest) -> Result<SignatureHelpResponse, PlatformError>;
}

/// A single LSP folding range.
#[derive(Debug, Clone)]
pub struct FoldingRange {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub kind: String,
}

impl FoldingRange {
    /// Create a new folding range with the given line/character bounds and kind.
    pub fn new(
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        kind: impl Into<String>,
    ) -> Self {
        Self {
            start_line,
            start_char,
            end_line,
            end_char,
            kind: kind.into(),
        }
    }
}

/// Request for `textDocument/foldingRange`.
#[derive(Debug, Clone)]
pub struct FoldingRangeRequest {
    /// Source text of the document.
    pub source: String,
}

/// Response for `textDocument/foldingRange`.
#[derive(Debug, Clone)]
pub struct FoldingRangeResponse(pub Vec<FoldingRange>);

/// Plugin contract for `textDocument/foldingRange`.
pub trait FoldingRangePlugin: Send + Sync {
    /// Return folding ranges for the whole document.
    fn run(&self, request: &FoldingRangeRequest) -> Result<FoldingRangeResponse, PlatformError>;
}
