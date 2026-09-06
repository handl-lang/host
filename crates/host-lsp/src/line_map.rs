//! Fast byte-offset → `(line, character)` conversion.
//!
//! Pre-computes the byte offset of the start of every line once, then answers
//! position queries in `O(log n)` via binary search. This replaces the
//! `O(n)`-per-call `char_indices` scan that made `folding_ranges` and
//! `document_symbols` run in `O(n²)` on large files.

/// A pre-computed index of line-start byte offsets for a source string.
pub struct LineMap {
    /// Byte offset of the first character of each line. Always starts with `0`.
    line_starts: Vec<usize>,
    /// The source bytes, kept so character columns can be computed without
    /// re-slicing the original `&str`.
    bytes: Vec<u8>,
}

impl LineMap {
    /// Build a line map for `source`.
    pub fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut line_starts = Vec::with_capacity(bytes.len() / 32 + 1);
        line_starts.push(0);
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        LineMap {
            line_starts,
            bytes: bytes.to_vec(),
        }
    }

    /// Convert a byte offset into a 0-based `(line, character)` pair, where
    /// `character` counts UTF-8 code points from the start of the line.
    ///
    /// Offsets past the end of the source are clamped to the end. Offsets that
    /// land in the middle of a multi-byte sequence are walked back to the
    /// nearest preceding char boundary (matching the behaviour of the
    /// previous `offset_to_position` / `offset_to_line_char` helpers).
    pub fn position(&self, offset: usize) -> (u64, u64) {
        let mut offset = offset.min(self.bytes.len());
        // Walk back to a char boundary.
        while offset > 0 && !self.is_char_boundary(offset) {
            offset -= 1;
        }

        // Binary search for the line whose start is <= offset.
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => {
                // i is the insertion point; the line we want is i-1.
                if i == 0 { 0 } else { i - 1 }
            }
        };

        let line_start = self.line_starts[line_idx];
        // Count UTF-8 code points between line_start and offset.
        let mut col: u64 = 0;
        let mut i = line_start;
        while i < offset {
            // Advance by one UTF-8 code point.
            let len = utf8_len(self.bytes[i]);
            i += len;
            col += 1;
        }

        (line_idx as u64, col)
    }

    /// Whether `offset` is at the start of a UTF-8 code point.
    fn is_char_boundary(&self, offset: usize) -> bool {
        if offset == 0 || offset == self.bytes.len() {
            return true;
        }
        // A byte is a char boundary iff it is NOT a continuation byte
        // (0b10xxxxxx).
        self.bytes[offset] & 0xC0 != 0x80
    }

    /// Whether `offset` is the first non-whitespace character on its line.
    pub fn is_at_line_start(&self, offset: usize) -> bool {
        let offset = offset.min(self.bytes.len());
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
        };
        let line_start = self.line_starts[line_idx];
        let mut i = line_start;
        while i < offset {
            match self.bytes[i] {
                b' ' | b'\t' | b'\r' => i += 1,
                _ => return false,
            }
        }
        true
    }

    /// Byte offset of the end of the line containing `from` (the position of
    /// the `\n`, or the end of the source).
    pub fn line_end_offset(&self, from: usize) -> usize {
        let from = from.min(self.bytes.len());
        let mut end = from;
        while end < self.bytes.len() && self.bytes[end] != b'\n' {
            end += 1;
        }
        end
    }

    /// The trimmed text from `from` to the end of its line.
    pub fn rest_of_line<'a>(&'a self, source: &'a str, from: usize) -> &'a str {
        let end = self.line_end_offset(from);
        source[from..end].trim()
    }
}

/// Length in bytes of the UTF-8 code point starting with `first`.
fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xC0 {
        // Continuation byte — shouldn't be reached as a start, but be safe.
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_at_start() {
        let lm = LineMap::new("hello");
        assert_eq!(lm.position(0), (0, 0));
    }

    #[test]
    fn position_mid_first_line() {
        let lm = LineMap::new("hello");
        assert_eq!(lm.position(2), (0, 2));
    }

    #[test]
    fn position_after_newline() {
        let lm = LineMap::new("abc\ndef");
        assert_eq!(lm.position(4), (1, 0));
        assert_eq!(lm.position(6), (1, 2));
    }

    #[test]
    fn position_past_end_clamped() {
        let lm = LineMap::new("abc");
        assert_eq!(lm.position(100), (0, 3));
    }

    #[test]
    fn position_with_multibyte() {
        let lm = LineMap::new("héllo");
        // 'é' is 2 bytes; offset 1 is mid-char and should clamp to 1 (col 1).
        assert_eq!(lm.position(1), (0, 1));
        assert_eq!(lm.position(2), (0, 1));
        assert_eq!(lm.position(3), (0, 2));
    }

    #[test]
    fn is_at_line_start_detects_indent() {
        let lm = LineMap::new("  foo\n  bar");
        assert!(lm.is_at_line_start(2));
        assert!(!lm.is_at_line_start(3));
        assert!(lm.is_at_line_start(7));
    }

    #[test]
    fn line_end_offset_finds_newline() {
        let lm = LineMap::new("abc\ndef");
        assert_eq!(lm.line_end_offset(0), 3);
        assert_eq!(lm.line_end_offset(4), 7);
    }
}
