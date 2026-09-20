//! Source-location predicates for cursors.

use clang_sys::*;

/// Resolve a cursor's location through macro expansion.
///
/// Declarations whose names are produced by renaming macros — ICU's
/// `urename.h` (`#define ucol_open ucol_open_78`), glibc's `__REDIRECT` —
/// report a *spelling* location inside the macro body or clang's
/// `<scratch space>`, which is neither the main file nor a system
/// header. The *expansion* location points back at the file that wrote
/// the name. For declarations not produced by a macro the expansion
/// location is the spelling location, so this is a no-op for ordinary
/// headers.
fn expansion_location(cursor: CXCursor) -> CXSourceLocation {
    unsafe {
        let loc = clang_getCursorLocation(cursor);
        let mut file: CXFile = std::ptr::null_mut();
        let mut line = 0;
        let mut column = 0;
        clang_getExpansionLocation(
            loc,
            &mut file,
            &mut line,
            &mut column,
            std::ptr::null_mut(),
        );
        if file.is_null() {
            return loc;
        }
        clang_getLocation(
            clang_Cursor_getTranslationUnit(cursor),
            file,
            line,
            column,
        )
    }
}

/// Whether the cursor's declaration is in the file being parsed (as
/// opposed to a transitively included header).
pub(crate) fn from_main_file(cursor: CXCursor) -> bool {
    unsafe { clang_Location_isFromMainFile(expansion_location(cursor)) != 0 }
}

/// Whether the cursor's declaration lives in a system header.
pub(crate) fn is_system_header(cursor: CXCursor) -> bool {
    unsafe { clang_Location_isInSystemHeader(expansion_location(cursor)) != 0 }
}
