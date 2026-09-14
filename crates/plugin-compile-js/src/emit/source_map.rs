//! Source Map V3 builder for JavaScript output.

use handl_language::Span;

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A single source-to-generated position mapping.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mapping {
    /// 0-indexed source line.
    pub source_line: u32,
    /// 0-indexed source column.
    pub source_col: u32,
    /// 0-indexed generated line.
    pub generated_line: u32,
    /// 0-indexed generated column.
    pub generated_col: u32,
    /// Optional symbol name associated with this mapping.
    pub name: Option<String>,
}

/// Builder for a Source Map V3 JSON object.
#[derive(Debug, Default, Clone)]
pub struct SourceMapBuilder {
    source_name: String,
    source: String,
    file: String,
    mappings: Vec<Mapping>,
    names: Vec<String>,
}

impl SourceMapBuilder {
    /// Create a new builder for the given source and output file.
    pub fn new(
        source_name: impl Into<String>,
        source: impl Into<String>,
        file: impl Into<String>,
    ) -> Self {
        Self {
            source_name: source_name.into(),
            source: source.into(),
            file: file.into(),
            mappings: Vec::new(),
            names: Vec::new(),
        }
    }

    /// Add a mapping from a generated position to a source position.
    pub fn add_mapping(
        &mut self,
        generated_line: u32,
        generated_col: u32,
        source_line: u32,
        source_col: u32,
        name: Option<String>,
    ) {
        if let Some(n) = name.as_ref() && !self.names.contains(n) {
            self.names.push(n.clone());
        }

        self.mappings.push(Mapping {
            source_line,
            source_col,
            generated_line,
            generated_col,
            name,
        });
    }

    /// Build the V3 source map JSON string.
    pub fn build(&self) -> String {
        let mut sorted = self.mappings.clone();
        sorted.sort_by(|a, b| {
            a.generated_line
                .cmp(&b.generated_line)
                .then_with(|| a.generated_col.cmp(&b.generated_col))
        });

        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut prev_gen_col: i64 = 0;
        let mut prev_source: i64 = 0;
        let mut prev_source_line: i64 = 0;
        let mut prev_source_col: i64 = 0;
        let mut prev_name: i64 = 0;

        for mapping in sorted {
            let line = mapping.generated_line as usize;
            while groups.len() <= line {
                groups.push(Vec::new());
            }

            let group = &mut groups[line];
            let gen_col_delta = mapping.generated_col as i64 - prev_gen_col;
            let source_delta = 0 - prev_source;
            let source_line_delta = mapping.source_line as i64 - prev_source_line;
            let source_col_delta = mapping.source_col as i64 - prev_source_col;

            let mut segment = String::new();
            segment.push_str(&encode_vlq(gen_col_delta));
            segment.push_str(&encode_vlq(source_delta));
            segment.push_str(&encode_vlq(source_line_delta));
            segment.push_str(&encode_vlq(source_col_delta));

            if let Some(_name) = &mapping.name {
                let name_idx = self.names.iter().position(|n| n == _name).unwrap_or(0) as i64;
                let name_delta = name_idx - prev_name;
                segment.push_str(&encode_vlq(name_delta));
                prev_name = name_idx;
            }

            if !group.is_empty() {
                group.push(",".to_string());
            }
            group.push(segment);

            prev_gen_col = mapping.generated_col as i64;
            prev_source = 0;
            prev_source_line = mapping.source_line as i64;
            prev_source_col = mapping.source_col as i64;
        }

        let mappings = groups
            .into_iter()
            .map(|g| g.concat())
            .collect::<Vec<_>>()
            .join(";");

        serde_json::json!({
            "version": 3,
            "file": self.file,
            "sources": [self.source_name],
            "sourcesContent": [self.source],
            "names": self.names,
            "mappings": mappings,
        })
        .to_string()
    }
}

/// Convert a byte offset into the source into a 0-indexed `(line, column)` pair.
pub(crate) fn byte_offset_to_line_col(source: &str, mut offset: usize) -> (u32, u32) {
    if offset > source.len() {
        offset = source.len();
    }
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let col = prefix.rsplit('\n').next().unwrap_or("").chars().count() as u32;
    (line, col)
}

/// Convert a source `Span` to a source-map `(line, column)` pair.
pub(crate) fn span_to_line_col(source: &str, span: Span) -> (u32, u32) {
    byte_offset_to_line_col(source, span.start)
}

fn encode_vlq(value: i64) -> String {
    let mut vlq = if value < 0 {
        ((-value) as u64) << 1 | 1
    } else {
        (value as u64) << 1
    };

    let mut out = String::new();
    loop {
        let mut digit = (vlq & 0b11111) as usize;
        vlq >>= 5;
        if vlq > 0 {
            digit |= 0b100000;
        }
        out.push(BASE64[digit] as char);
        if vlq == 0 {
            break;
        }
    }
    out
}
