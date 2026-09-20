//! `#[cfg(test)]` helpers: parse a self-contained C snippet into a
//! libclang translation unit and fish out cursors/types, so `map`/`parse`
//! functions can be unit-tested without full `parse_header` runs.
//!
//! Snippets must be self-contained — no system headers are on the include
//! path. For `size_t` and friends, `typedef` them inside the snippet.

use clang_sys::*;
use std::ffi::{CString, c_int, c_ulong, c_void};

/// A live translation unit. Owns the index + TU so cursors/types derived
/// from it stay valid for its lifetime.
pub(crate) struct Snippet {
    index: CXIndex,
    tu: CXTranslationUnit,
}

impl Drop for Snippet {
    fn drop(&mut self) {
        unsafe {
            clang_disposeTranslationUnit(self.tu);
            clang_disposeIndex(self.index);
        }
    }
}

/// Parse `src` as a C file supplied entirely in memory.
pub(crate) fn parse_snippet(src: &str) -> Snippet {
    clang_sys::load().expect("libclang load");
    let index = unsafe { clang_createIndex(0, 0) };
    assert!(!index.is_null(), "createIndex failed");
    let c_src = CString::new(src).unwrap();
    let name = CString::new("snippet.h").unwrap();
    let args: Vec<CString> = ["-x", "c", "-std=c11", "-fparse-all-comments"]
        .iter()
        .map(|s| CString::new(*s).unwrap())
        .collect();
    let argp: Vec<*const std::ffi::c_char> = args.iter().map(|s| s.as_ptr()).collect();
    let mut unsaved = CXUnsavedFile {
        Filename: name.as_ptr(),
        Contents: c_src.as_ptr(),
        Length: c_src.as_bytes().len() as c_ulong,
    };
    let tu = unsafe {
        clang_parseTranslationUnit(
            index,
            name.as_ptr(),
            argp.as_ptr(),
            argp.len() as c_int,
            &mut unsaved,
            1,
            // So MacroDefinition cursors are visible to collectors.
            CXTranslationUnit_DetailedPreprocessingRecord,
        )
    };
    assert!(!tu.is_null(), "snippet parse failed");
    Snippet { index, tu }
}

impl Snippet {
    pub(crate) fn tu(&self) -> CXTranslationUnit {
        self.tu
    }

    /// Pointer width of the snippet target in bits.
    pub(crate) fn pointer_width(&self) -> usize {
        unsafe {
            let ti = clang_getTranslationUnitTargetInfo(self.tu);
            let w = clang_TargetInfo_getPointerWidth(ti);
            clang_TargetInfo_dispose(ti);
            w as usize
        }
    }

    /// The first top-level cursor of `kind` whose name is `name`.
    /// `""` matches anonymous declarations.
    pub(crate) fn find_decl(&self, kind: CXCursorKind, name: &str) -> Option<CXCursor> {
        struct Want<'a> {
            kind: CXCursorKind,
            name: &'a str,
            found: Option<CXCursor>,
        }
        extern "C" fn visit(c: CXCursor, _p: CXCursor, d: *mut c_void) -> CXChildVisitResult {
            let w = unsafe { &mut *(d as *mut Want) };
            // An empty `name` queries for anonymous declarations, whose
            // spelling is "" or "(unnamed at ...)" depending on libclang.
            let name_matches = if w.name.is_empty() {
                crate::cx::is_anonymous_decl(c)
            } else {
                crate::cx::cursor_name(c) == w.name
            };
            if c.kind == w.kind && name_matches {
                w.found = Some(c);
                return CXChildVisit_Break;
            }
            CXChildVisit_Continue
        }
        let mut want = Want {
            kind,
            name,
            found: None,
        };
        let root = unsafe { clang_getTranslationUnitCursor(self.tu) };
        unsafe { clang_visitChildren(root, visit, &mut want as *mut _ as *mut c_void) };
        want.found
    }

    /// The `FunctionDecl` cursor for `name`.
    pub(crate) fn fn_cursor(&self, name: &str) -> CXCursor {
        self.find_decl(CXCursor_FunctionDecl, name)
            .unwrap_or_else(|| panic!("function {name} not in snippet"))
    }

    /// The `i`-th argument type of function `name`.
    pub(crate) fn fn_arg_type(&self, name: &str, i: u32) -> CXType {
        let f = self.fn_cursor(name);
        unsafe { clang_getCursorType(clang_Cursor_getArgument(f, i)) }
    }

    /// The `i`-th argument cursor of function `name`.
    pub(crate) fn fn_arg_cursor(&self, name: &str, i: u32) -> CXCursor {
        let f = self.fn_cursor(name);
        unsafe { clang_Cursor_getArgument(f, i) }
    }

    /// The return type of function `name`.
    pub(crate) fn fn_ret_type(&self, name: &str) -> CXType {
        unsafe { clang_getCursorResultType(self.fn_cursor(name)) }
    }

    /// The `StructDecl` cursor for `name`.
    pub(crate) fn record_cursor(&self, name: &str) -> CXCursor {
        self.find_decl(CXCursor_StructDecl, name)
            .unwrap_or_else(|| panic!("record {name} not in snippet"))
    }

    /// The `TypedefDecl` cursor for `name`.
    pub(crate) fn typedef_cursor(&self, name: &str) -> CXCursor {
        self.find_decl(CXCursor_TypedefDecl, name)
            .unwrap_or_else(|| panic!("typedef {name} not in snippet"))
    }
}
