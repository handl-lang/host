//! DWARF debug-info builder trait and placeholder.

#![allow(unsafe_code)]

use std::fmt::Debug;

/// Builds a DWARF-style line table.
pub trait DebugInfoBuilder: Debug {
    /// Record a source line/column entry.
    fn add_line_table(&mut self, file: &str, line: u32, column: u32);

    /// Finalize and return the line table bytes.
    fn finish(&self) -> Vec<u8>;
}

/// A minimal DWARF line table builder.
#[derive(Debug, Default)]
pub struct DwarfLineTable {
    entries: Vec<(String, u32, u32)>,
}

impl DwarfLineTable {
    /// Create a new empty line table builder.
    pub fn new() -> Self {
        Self::default()
    }
}

impl DebugInfoBuilder for DwarfLineTable {
    fn add_line_table(&mut self, file: &str, line: u32, column: u32) {
        self.entries.push((file.to_string(), line, column));
    }

    fn finish(&self) -> Vec<u8> {
        #[cfg(feature = "inkwell")]
        {
            // Real DWARF line table construction: emit a compact binary
            // representation of the collected (file, line, column) entries.
            let mut out = Vec::new();
            for (file, line, column) in &self.entries {
                let file_bytes = file.as_bytes();
                out.extend_from_slice(&(file_bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(file_bytes);
                out.extend_from_slice(&line.to_le_bytes());
                out.extend_from_slice(&column.to_le_bytes());
            }
            out
        }

        #[cfg(not(feature = "inkwell"))]
        b"<DwarfLineTable inkwell disabled>".to_vec()
    }
}
