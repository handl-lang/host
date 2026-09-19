//! Translation-unit cursor collection.
//!
//! `collect_cursors` is the `clang_visitChildren` callback that sorts
//! top-level declarations into functions, records, typedefs, enums, and
//! macros — filtered by `--include`/`--exclude` globs. `collect_enum`
//! expands one enum cursor into constants (macro constants come from the
//! probe pass in [`crate::backend::macro_probe`]).

use clang_sys::*;
use glob::Pattern;
use handl_plugin_sdkgen::{EnumConst, SdkgenModule};
use std::ffi::c_void;

use crate::cx::{children_of_kind, cursor_name, from_main_file, is_system_header};

/// Buckets of collected cursors plus the `include`/`exclude`/`constants`
/// filters.
#[derive(Default)]
pub(crate) struct CursorData {
    pub functions: Vec<CXCursor>,
    pub records: Vec<CXCursor>,
    pub typedefs: Vec<CXCursor>,
    pub enums: Vec<CXCursor>,
    pub macros: Vec<CXCursor>,
    /// `--include` glob(s): functions/types must match at least one.
    /// Empty = collect everything in the main file.
    pub include: Vec<Pattern>,
    /// `--exclude` glob(s): a match vetoes collection, even when an
    /// include glob also matches.
    pub exclude: Vec<Pattern>,
    /// `constants` glob(s) from `.sdkgen.yaml`: enum constants and
    /// object-like macros must match at least one. Empty = every constant
    /// reachable through the header's include closure (the full API
    /// surface a C program sees after `#include`).
    pub consts: Vec<Pattern>,
}

fn matches_any(patterns: &[Pattern], name: &str) -> bool {
    patterns.iter().any(|p| p.matches(name))
}

impl CursorData {
    /// Included and not excluded — the shared name gate.
    fn allowed(&self, name: &str) -> bool {
        (self.include.is_empty() || matches_any(&self.include, name))
            && !matches_any(&self.exclude, name)
    }
}

pub(crate) extern "C" fn collect_cursors(
    cursor: CXCursor,
    _parent: CXCursor,
    data: *mut c_void,
) -> CXChildVisitResult {
    let state = unsafe { &mut *(data as *mut CursorData) };
    let name = cursor_name(cursor);
    let allowed = state.allowed(&name);
    let excluded = matches_any(&state.exclude, &name);
    let is_main = from_main_file(cursor);
    match cursor.kind {
        CXCursor_FunctionDecl => {
            if is_main && allowed && unsafe { clang_getCursorLinkage(cursor) } == CXLinkage_External
            {
                state.functions.push(cursor);
            }
        }
        CXCursor_StructDecl | CXCursor_TypedefDecl => {
            let is_sys = is_system_header(cursor);
            // Collect type declarations from the main file (filtered by
            // include/exclude) and from system headers, which supply the
            // ABI for the selected functions even when their names do not
            // match `include`. `exclude` vetoes everywhere — a type the
            // user excluded should not exist as far as generation is
            // concerned.
            let filtering = !state.include.is_empty() || !state.exclude.is_empty();
            if !((is_main && allowed) || (is_sys && !excluded && (allowed || filtering))) {
                return CXChildVisit_Continue;
            }
            match cursor.kind {
                // Forward declarations have no layout to parse — only
                // definitions are collected.
                CXCursor_StructDecl if unsafe { clang_isCursorDefinition(cursor) } != 0 => {
                    state.records.push(cursor)
                }
                CXCursor_TypedefDecl => state.typedefs.push(cursor),
                _ => {}
            }
        }
        CXCursor_EnumDecl => {
            let is_sys = is_system_header(cursor);
            // Enums are exempt from `include` at declaration level:
            // anonymous enums have no name to match, and a named enum may
            // hold constants that do. Constants are filtered in
            // collect_enum. `exclude` still vetoes by enum name.
            let filtering =
                !state.include.is_empty() || !state.exclude.is_empty() || !state.consts.is_empty();
            if !excluded && (is_main || (is_sys && (allowed || filtering))) {
                state.enums.push(cursor);
            }
        }
        CXCursor_MacroDefinition => {
            // Object-like macros are constants — same gate as enum
            // constants. Origin gate first: compiler builtins have no
            // source location and `_`-prefixed names are the reserved
            // internal namespace — neither is API. Then `constants` wins
            // outright, else include/exclude, else everything reachable
            // through the header's include closure.
            let is_sys = is_system_header(cursor);
            let object_like = unsafe { clang_Cursor_isMacroFunctionLike(cursor) } == 0;
            let in_file = is_main || is_sys;
            let take = if !state.consts.is_empty() {
                matches_any(&state.consts, &name) && !excluded
            } else {
                !name.starts_with('_') && allowed
            };
            if object_like && in_file && take {
                state.macros.push(cursor);
            }
        }
        _ => {}
    }
    CXChildVisit_Continue
}

/// Expand one enum cursor into `EnumConst` entries on the module.
/// `consts` (config `constants:`) takes precedence; otherwise `include`
/// filters; `exclude` vetoes either way. With neither, all constants are
/// emitted (the enum's own name never gates them — anonymous enums carry
/// public constants).
pub(crate) fn collect_enum(
    cursor: CXCursor,
    module: &mut SdkgenModule,
    include: &[Pattern],
    exclude: &[Pattern],
    consts: &[Pattern],
) {
    for c in children_of_kind(cursor, CXCursor_EnumConstantDecl) {
        let name = cursor_name(c);
        if name.is_empty() {
            continue;
        }
        let keep = if !consts.is_empty() {
            matches_any(consts, &name)
        } else {
            include.is_empty() || matches_any(include, &name)
        };
        if !keep || matches_any(exclude, &name) {
            continue;
        }
        let value = unsafe { clang_getEnumConstantDeclValue(c) };
        module.enum_constants.push(EnumConst { name, value });
    }
}
