//! Source-location predicates for cursors.

use clang_sys::*;

/// Whether the cursor's declaration is in the file being parsed (as
/// opposed to a transitively included header).
pub(crate) fn from_main_file(cursor: CXCursor) -> bool {
    unsafe { clang_Location_isFromMainFile(clang_getCursorLocation(cursor)) != 0 }
}

/// Whether the cursor's declaration lives in a system header.
pub(crate) fn is_system_header(cursor: CXCursor) -> bool {
    unsafe { clang_Location_isInSystemHeader(clang_getCursorLocation(cursor)) != 0 }
}
