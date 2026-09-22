//! Translation-unit diagnostic collection and the error/fatal gate.

use clang_sys::*;
use handl_plugin_sdkgen::SdkgenError;

use crate::cx::cx_string;

/// Collect libclang diagnostics of severity ≥ warning. Returns `Err` —
/// with every message joined — when any error/fatal diagnostic exists.
pub(crate) fn check_tu(tu: CXTranslationUnit) -> Result<(), SdkgenError> {
    let mut fatal = false;
    let mut msgs = Vec::new();
    let num = unsafe { clang_getNumDiagnostics(tu) };
    for i in 0..num {
        let diag = unsafe { clang_getDiagnostic(tu, i) };
        let sev = unsafe { clang_getDiagnosticSeverity(diag) };
        if sev == CXDiagnostic_Error || sev == CXDiagnostic_Fatal {
            fatal = true;
        }
        if sev >= CXDiagnostic_Warning {
            let sev_label = match sev {
                CXDiagnostic_Warning => "warning",
                CXDiagnostic_Error => "error",
                CXDiagnostic_Fatal => "fatal",
                _ => "note",
            };
            let msg = cx_string(unsafe { clang_getDiagnosticSpelling(diag) });
            if !msg.is_empty() {
                msgs.push(format!("[{sev_label}] {msg}"));
            }
        }
        unsafe { clang_disposeDiagnostic(diag) };
    }
    if fatal {
        Err(SdkgenError(format!(
            "libclang reported parse errors:\n{}",
            msgs.join("\n")
        )))
    } else {
        Ok(())
    }
}
