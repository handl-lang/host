use handl_platform::PlatformError;

/// A single LSP completion candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub label: String,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

/// A source location as a half-open byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub start: usize,
    pub end: usize,
}

impl Location {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A document symbol for outline / breadcrumb views.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSymbol {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

impl DocumentSymbol {
    pub fn new(name: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            name: name.into(),
            start,
            end,
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
}

#[derive(Debug, Clone)]
pub struct CompletionResponse(pub Vec<CompletionItem>);

#[derive(Debug, Clone)]
pub struct GotoDefRequest {
    pub source: String,
    pub pos: usize,
}

#[derive(Debug, Clone)]
pub struct GotoDefResponse(pub Option<Location>);

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
    fn run(&self, request: &DocumentSymbolsRequest) -> Result<DocumentSymbolsResponse, PlatformError>;
}

/// Plugin contract for incremental `textDocument/didChange`.
pub trait IncrementalSyncPlugin: Send + Sync {
    fn run(&self, request: &IncrementalSyncRequest) -> Result<IncrementalSyncResponse, PlatformError>;
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
    fn run(&self, request: &IncrementalSyncRequest) -> Result<IncrementalSyncResponse, PlatformError> {
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
