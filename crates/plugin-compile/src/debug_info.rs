/// Debug information formatter for a compiled target.
pub trait DebugInfo: std::fmt::Debug {
    /// Format this debug info as a string.
    fn format(&self) -> String;
}

/// Placeholder debug-info implementation (ADR-0114 Phase 2).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Null;

impl DebugInfo for Null {
    fn format(&self) -> String {
        String::new()
    }
}
