//! `CXString` → `String` conversions and cursor name/spelling/doc helpers.

use clang_sys::*;
use std::ffi::CStr;

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

/// Strip C comment syntax and translate Doxygen tags into plain doc lines.
fn normalize_doc(raw: &str) -> Option<String> {
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
