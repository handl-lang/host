//! `CXString` → `String` conversions and cursor name/spelling/doc helpers.

use clang_sys::*;
use std::collections::HashMap;
use std::ffi::{CStr, c_uint};
use std::path::PathBuf;

/// Convert a libclang `CXString` to an owned `String`, disposing the
/// `CXString` after copying.
pub(crate) fn cx_string(str: CXString) -> String {
    unsafe {
        let ptr = clang_getCString(str);
        let s = if ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        };
        clang_disposeString(str);
        s
    }
}

/// The spelling (name) of a cursor — empty for anonymous declarations.
pub(crate) fn cursor_name(cursor: CXCursor) -> String {
    unsafe { cx_string(clang_getCursorSpelling(cursor)) }
}

/// The spelling of a type, e.g. `const struct sockaddr *`.
pub(crate) fn type_spelling(ty: CXType) -> String {
    unsafe { cx_string(clang_getTypeSpelling(ty)) }
}

/// Whether a declaration cursor has no usable name. libclang spells
/// anonymous tags either as `""` or `"(unnamed at file:line:col)"`
/// depending on version — a typedef'd anonymous struct (`typedef struct
/// {...} foo_t`) gets the typedef name and is *not* anonymous here.
pub(crate) fn is_anonymous_decl(cursor: CXCursor) -> bool {
    let name = cursor_name(cursor);
    name.is_empty() || name.contains("(unnamed") || name.contains("(anonymous")
}

/// The declared name of a record type (`struct point` → `"point"`).
pub(crate) fn record_name(ty: CXType) -> Result<String, String> {
    let decl = unsafe { clang_getTypeDeclaration(ty) };
    if is_anonymous_decl(decl) {
        return Err(format!("unnamed record: {}", type_spelling(ty)));
    }
    Ok(cursor_name(decl))
}

/// The doc comment attached to a cursor, normalized to plain text:
/// comment markers (`/**`, `*`, `//`) stripped and common Doxygen tags
/// translated. `None` when absent or empty after normalization.
pub(crate) fn doc_comment(cursor: CXCursor) -> Option<String> {
    let raw = unsafe { cx_string(clang_Cursor_getRawCommentText(cursor)) };
    normalize_doc(&raw)
}

/// Doc for a `#define` constant. libclang does not reliably attach raw
/// comments to `MacroDefinition` cursors (host-version dependent), so
/// when the cursor carries none the source text is scanned directly: a
/// trailing comment on the definition line wins, else a contiguous
/// comment block ending on the line above. `files` caches file contents
/// across the (many) macros a header defines.
pub(crate) fn macro_doc(
    cursor: CXCursor,
    files: &mut HashMap<PathBuf, Vec<String>>,
) -> Option<String> {
    if let Some(doc) = doc_comment(cursor) {
        return Some(doc);
    }
    let loc = unsafe { clang_getCursorLocation(cursor) };
    let mut file: CXFile = std::ptr::null_mut();
    let mut line: c_uint = 0;
    unsafe {
        clang_getFileLocation(
            loc,
            &mut file,
            &mut line,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if file.is_null() || line == 0 {
        return None;
    }
    let path = PathBuf::from(unsafe { cx_string(clang_getFileName(file)) });
    let lines = files.entry(path.clone()).or_insert_with(|| {
        std::fs::read_to_string(&path)
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    });
    lexical_doc(lines, line as usize - 1)
}

/// Extract a doc for the item defined on `def_line` (0-based): a
/// trailing comment on that line, else the contiguous comment block
/// ending on the line directly above.
fn lexical_doc(lines: &[String], def_line: usize) -> Option<String> {
    if let Some(raw) = trailing_comment(lines.get(def_line)?) {
        return normalize_doc(&raw);
    }
    let mut block: Vec<&str> = Vec::new();
    let mut i = def_line;
    while i > 0 {
        let l = lines[i - 1].trim();
        if l.starts_with("//") || l.starts_with("/*") || l.starts_with('*') || l.ends_with("*/") {
            block.push(lines[i - 1].as_str());
            i -= 1;
        } else {
            break;
        }
    }
    block.reverse();
    normalize_doc(&block.join("\n"))
}

/// The comment text on a source line — from `/*` or `//` to the end of
/// the comment (or line). The earliest opener wins so `/* a // b */`
/// yields the block, not the nested `//`.
fn trailing_comment(line: &str) -> Option<String> {
    let start = [line.find("//"), line.find("/*")]
        .into_iter()
        .flatten()
        .min()?;
    Some(line[start..].to_string())
}

/// Strip C comment syntax and translate Doxygen tags into plain doc lines.
pub(crate) fn normalize_doc(raw: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let mut l = line.trim();
        for marker in ["/**", "/*!", "///<", "///", "//", "/*"] {
            if let Some(rest) = l.strip_prefix(marker) {
                l = rest;
                break;
            }
        }
        l = l.trim_end_matches("*/").trim_start_matches('*').trim();
        if !l.is_empty() {
            lines.push(translate_tag(l));
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Translate one line's leading `@tag` / `\tag` Doxygen command; unknown
/// tags are kept verbatim.
fn translate_tag(l: &str) -> String {
    let Some(rest) = l.strip_prefix('@').or_else(|| l.strip_prefix('\\')) else {
        return l.to_string();
    };
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let (tag, body) = (&rest[..end], rest[end..].trim());
    // `@param[out]` / `@retval{x}` — brackets glued to the tag are dropped.
    let tag = tag.split('[').next().unwrap_or(tag);
    match tag {
        "brief" | "short" | "description" => body.to_string(),
        "param" | "arg" | "argument" => {
            let mut b = body;
            if let Some(inner) = b.strip_prefix('[') {
                if let Some(i) = inner.find(']') {
                    b = inner[i + 1..].trim();
                }
            }
            match b.find(char::is_whitespace) {
                Some(i) => format!("{}: {}", &b[..i], b[i..].trim()),
                None => format!("{b}:"),
            }
        }
        "return" | "returns" | "retval" => format!("Returns: {body}"),
        "note" | "remark" | "remarks" => format!("Note: {body}"),
        "warning" => format!("Warning: {body}"),
        "sa" | "see" => format!("See: {body}"),
        _ => l.to_string(),
    }
}
