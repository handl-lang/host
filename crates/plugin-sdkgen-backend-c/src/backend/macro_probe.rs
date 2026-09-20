//! Object-like `#define` constant evaluation via a probe translation
//! unit.
//!
//! libclang cannot evaluate `MacroDefinition` cursors directly, so we
//! reparse a synthesized probe file: `#include "header"` plus
//! `enum { __hnl_sdkgen_<NAME> = (<NAME>) };` per macro. libclang folds
//! the expressions — including references to other macros and enum
//! constants (`#define AF_INET PF_INET`) — and macros that are not
//! integer constants (empty header guards, string literals, odd syntax)
//! produce no evaluable constant and are dropped.

use clang_sys::*;
use handl_plugin_sdkgen::EnumConst;
use std::ffi::{CString, c_char, c_int, c_ulong};
use std::path::Path;

use crate::cx::{children_of_kind, cursor_name, first_child};

/// Prefix on probe enum constants that carry macro names.
const PROBE_PREFIX: &str = "__hnl_sdkgen_";

/// Evaluate `names` (object-like macros) to integer constants.
/// `args` are the same clang args used for the main parse. Anything that
/// fails to fold is silently skipped.
pub(crate) fn eval_macros(
    index: CXIndex,
    args: &[*const c_char],
    names: &[String],
    header: &Path,
) -> Vec<EnumConst> {
    if names.is_empty() {
        return Vec::new();
    }
    let abs = header
        .canonicalize()
        .unwrap_or_else(|_| header.to_path_buf());
    let mut src = format!("#include \"{}\"\n", abs.display());
    for n in names {
        src.push_str(&format!("enum {{ {PROBE_PREFIX}{n} = ({n}) }};\n"));
    }
    let Ok(c_src) = CString::new(src) else {
        return Vec::new();
    };
    let probe_name = abs.with_file_name("__hnl_sdkgen_probe.c");
    let Ok(c_name) = CString::new(probe_name.to_string_lossy().as_bytes()) else {
        return Vec::new();
    };
    // The probe file is supplied as an unsaved buffer — nothing is written
    // to disk.
    let mut unsaved = CXUnsavedFile {
        Filename: c_name.as_ptr(),
        Contents: c_src.as_ptr(),
        Length: c_src.as_bytes().len() as c_ulong,
    };
    let tu = unsafe {
        clang_parseTranslationUnit(
            index,
            c_name.as_ptr(),
            args.as_ptr(),
            args.len() as c_int,
            &mut unsaved,
            1,
            0,
        )
    };
    if tu.is_null() {
        return Vec::new();
    }

    let root = unsafe { clang_getTranslationUnitCursor(tu) };
    let mut out = Vec::new();
    for d in children_of_kind(root, CXCursor_EnumDecl) {
        for c in children_of_kind(d, CXCursor_EnumConstantDecl) {
            // Error recovery still produces a constant cursor for failed
            // expressions (`#define GUARD` → `()`). Evaluate each
            // initializer: only a clean integer fold keeps it.
            let Some(value) = eval_int_constant(c) else {
                continue;
            };
            let name = cursor_name(c);
            if let Some(macro_name) = name.strip_prefix(PROBE_PREFIX) {
                out.push(EnumConst {
                    name: macro_name.to_string(),
                    value,
                    // The probe TU holds synthesized `enum { … }` — the
                    // macro's own comment lives on the main TU's cursor,
                    // which is where the caller fills this in.
                    doc: None,
                });
            }
        }
    }
    unsafe { clang_disposeTranslationUnit(tu) };
    out
}

/// Evaluate an `EnumConstantDecl`'s initializer expression; `None` when it
/// does not fold to an integer.
fn eval_int_constant(cursor: CXCursor) -> Option<i64> {
    let expr = first_child(cursor)?;
    let result = unsafe { clang_Cursor_Evaluate(expr) };
    if result.is_null() {
        return None;
    }
    let value = if unsafe { clang_EvalResult_getKind(result) } == CXEval_Int {
        Some(unsafe { clang_EvalResult_getAsLongLong(result) })
    } else {
        None
    };
    unsafe { clang_EvalResult_dispose(result) };
    value
}
