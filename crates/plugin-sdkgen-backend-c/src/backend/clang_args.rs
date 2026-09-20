//! libclang command-line argument construction for `parse_header`.

use std::ffi::{CString, c_char};
use std::path::Path;

/// clang's resource dir (builtin headers like `stddef.h`), probed via
/// `clang -print-resource-dir`.
fn resource_dir() -> Option<String> {
    std::process::Command::new("clang")
        .arg("-print-resource-dir")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Build the raw argv for `clang_parseTranslationUnit`:
/// `-x c -I<header dir> -I/usr/include [-resource-dir <clang dir>]
/// -target <triple> [-D<define> …]`.
pub(crate) fn clang_args(
    header: &Path,
    target: &str,
    defines: &[String],
) -> (Vec<CString>, Vec<*const c_char>) {
    // `Path::new("simple.h").parent()` is `Some("")`, not `None`.
    let header_dir = header
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut raw: Vec<CString> = Vec::new();
    raw.push(CString::new("-x").unwrap());
    raw.push(CString::new("c").unwrap());
    // Retain *all* comments in the TU, not just doxygen-style ones — real
    // headers (zlib, glibc) document declarations with plain `/* */` and
    // `//` comments, which `clang_Cursor_getRawCommentText` otherwise
    // never sees.
    raw.push(CString::new("-fparse-all-comments").unwrap());
    // `-iquote`, not `-I`: the header's directory must serve `"..."`
    // includes but must NOT join the `<...>` search path — a system
    // header directory (e.g. `/usr/include/unicode`) would otherwise
    // shadow C library headers that share a basename (`<uchar.h>` finds
    // ICU's uchar.h instead of the C11 one).
    raw.push(CString::new("-iquote").unwrap());
    raw.push(CString::new(header_dir.display().to_string()).unwrap());
    raw.push(CString::new("-I/usr/include").unwrap());
    if let Some(res) = resource_dir() {
        raw.push(CString::new("-resource-dir").unwrap());
        raw.push(CString::new(res).unwrap());
    }
    raw.push(CString::new("-target").unwrap());
    raw.push(CString::new(target).unwrap());
    for d in defines {
        raw.push(CString::new(format!("-D{d}")).unwrap());
    }
    let ptrs = raw.iter().map(|s| s.as_ptr()).collect();
    (raw, ptrs)
}
