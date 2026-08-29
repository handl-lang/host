use std::ffi::{c_char, CStr, CString};
use std::ptr::null;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Request {
    capability: String,
    payload: serde_json::Value,
}

/// Exposes the C ABI expected by `handl-plugin-loader-native`.
#[unsafe(no_mangle)]
pub extern "C" fn handl_plugin_run(request_json: *const c_char) -> *const c_char {
    if request_json.is_null() {
        return null();
    }

    let input = unsafe { CStr::from_ptr(request_json) };
    let request: Request = match serde_json::from_slice(input.to_bytes()) {
        Ok(r) => r,
        Err(_) => return null(),
    };

    let response = if request.capability == "echo" {
        request.payload
    } else {
        serde_json::json!({ "error": "unknown capability" })
    };

    let s = match serde_json::to_string(&response) {
        Ok(s) => s,
        Err(_) => return null(),
    };

    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => null(),
    }
}
