//! # handl-plugin-ffi — Generic FFI Dispatcher (ADR-0083 §4)
//!
//! One Rust crate provides the generic FFI dispatcher. It is the **only** new
//! Rust per ADR-0083. The dispatcher:
//!
//! 1. Loads libraries (cached by name — `dlopen` once, reuse).
//! 2. Looks up symbols (cached by library + symbol — `dlsym` once).
//! 3. Marshals args per the descriptor (handl `Value` → C types).
//! 4. Calls the symbol (`unsafe extern "C"`).
//! 5. Unmarshals the return (C type → handl `Value`).
//!
//! The dispatcher implements [`handl_platform::Function`] (the same trait
//! `OpFunction` dispatches to), so it plugs into the existing
//! [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) with no engine changes.
//! An `FfiOpFunction` is just a `Function` whose `call` does the
//! marshal/call/unmarshal dance.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::sync::{Arc, Mutex};

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use libloading::Library;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// MarshalType (ADR-0083 §2)
// ---------------------------------------------------------------------------

/// A C marshalling type descriptor (ADR-0083 §2).
///
/// Each descriptor maps a handl value to a C ABI type for FFI dispatch.
/// The fixed, small set covers the vast majority of C ABIs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarshalType {
    /// `int32_t` — handl `Number` truncated to i32, pass by value.
    I32,
    /// `int64_t` — handl `Number` truncated to i64, pass by value.
    I64,
    /// `double` — handl `Number` pass by value.
    F64,
    /// `const char*` — handl `String` null-terminated, pass pointer.
    CStr,
    /// `void* + size_t` — handl `Array<Byte>` as (ptr, len) pair, two C args.
    Buf,
    /// `void*` — opaque `Handle` pass-through raw pointer.
    Ptr,
    /// `void` — no return value (return type only).
    Void,
    /// `void*` — opaque `Handle` wrapped in a resource table entry.
    Handle,
}

impl MarshalType {
    /// Parse a descriptor string ("i32", "cstr", etc.) into a [`MarshalType`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "f64" => Some(Self::F64),
            "cstr" => Some(Self::CStr),
            "buf" => Some(Self::Buf),
            "ptr" => Some(Self::Ptr),
            "void" => Some(Self::Void),
            "handle" => Some(Self::Handle),
            _ => None,
        }
    }

    /// The number of C arguments this descriptor consumes.
    ///
    /// `buf` consumes two (ptr + len); all others consume one.
    pub fn c_arg_count(&self) -> usize {
        match self {
            Self::Buf => 2,
            _ => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// FfiBinding
// ---------------------------------------------------------------------------

/// An FFI binding specification: library, symbol, arg types, return type.
///
/// Corresponds to the `ffi("library", "symbol", [arg_types] -> return_type)`
/// declaration form in handl source (ADR-0083 §1).
#[derive(Debug, Clone)]
pub struct FfiBinding {
    /// The library name — resolved by the dynamic loader (`dlopen`).
    /// The special name `"self"` uses `Library::this()` (the current
    /// process's symbol table), which is useful for testing.
    pub library: String,
    /// The C symbol name (`dlsym`).
    pub symbol: String,
    /// Argument marshalling types — one per handl argument.
    pub arg_types: Vec<MarshalType>,
    /// Return marshalling type.
    pub return_type: MarshalType,
}

// ---------------------------------------------------------------------------
// LibraryCache
// ---------------------------------------------------------------------------

/// A cached library lookup table (ADR-0083 §4: "dlopen once, reuse").
///
/// Libraries are loaded once via `dlopen` (or `Library::this()` for the
/// special `"self"` name) and cached for subsequent lookups. The cache is
/// shared across all [`FfiDispatcher`] instances that use the same
/// [`FfiLibraryCache`].
pub(crate) struct LibraryCache {
    libs: HashMap<String, Library>,
}

impl Default for LibraryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryCache {
    fn new() -> Self {
        Self {
            libs: HashMap::new(),
        }
    }

    /// Get or load a library by name. The special name `"self"` loads the
    /// current process's symbol table via `Library::this()`.
    fn get_or_load(&mut self, name: &str) -> Result<&Library, EvalError> {
        // HashMap entry API doesn't work well with Library (not Clone), so we
        // check then insert.
        if !self.libs.contains_key(name) {
            let lib = if name == "self" {
                // Load the current process's symbol table. On Unix this is
                // safe (Library::this() is not unsafe); on Windows it's also
                // safe. We only look up symbols that actually exist (dlsym
                // verifies).
                #[cfg(unix)]
                {
                    libloading::os::unix::Library::this().into()
                }
                #[cfg(windows)]
                {
                    libloading::os::windows::Library::this().into()
                }
                #[cfg(not(any(unix, windows)))]
                {
                    return Err(EvalError::Other(
                        "ffi: 'self' library not supported on this platform".to_string(),
                    ));
                }
            } else {
                // SAFETY: Library::new calls dlopen which is unsafe because it
                // loads arbitrary native code. The caller (binding author) is
                // responsible for ensuring the library is safe to load.
                unsafe { Library::new(name) }.map_err(|e| {
                    EvalError::Other(format!("ffi: failed to load library '{}': {}", name, e))
                })?
            };
            self.libs.insert(name.to_string(), lib);
        }
        Ok(self.libs.get(name).expect("just inserted"))
    }
}

// ---------------------------------------------------------------------------
// FfiDispatcher
// ---------------------------------------------------------------------------

/// The FFI dispatcher (ADR-0083 §4).
///
/// Caches `dlopen`'d libraries and `dlsym`'d symbols. Implements [`Function`]
/// so it plugs into [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) via
/// [`OpFunction`](handl_plugin_lib_handl::OpFunction).
///
/// On `call`, the dispatcher:
/// 1. Validates the argument count matches the binding's arg types.
/// 2. Marshals each handl `Value` to C types per `arg_types`.
/// 3. Loads the library (cached) and looks up the symbol (cached).
/// 4. Calls the raw C function via `unsafe extern "C"`.
/// 5. Unmarshals the return per `return_type`.
pub struct FfiDispatcher {
    binding: FfiBinding,
    cache: Arc<Mutex<LibraryCache>>,
}

impl FfiDispatcher {
    /// Create a new `FfiDispatcher` with a fresh library cache.
    pub fn new(binding: FfiBinding) -> Self {
        Self {
            binding,
            cache: Arc::new(Mutex::new(LibraryCache::new())),
        }
    }

    /// Create a new `FfiDispatcher` sharing an existing library cache.
    ///
    /// Use [`FfiLibraryCache::dispatcher`] for the common case.
    pub(crate) fn with_cache(binding: FfiBinding, cache: Arc<Mutex<LibraryCache>>) -> Self {
        Self { binding, cache }
    }
}

impl Function for FfiDispatcher {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        // 1. Validate arg count.
        let expected = self.binding.arg_types.len();
        if args.len() != expected {
            return Err(EvalError::Other(format!(
                "ffi '{}': expected {} args, got {}",
                self.binding.symbol,
                expected,
                args.len()
            )));
        }

        // 2. Marshal args.
        let marshalled = marshal_args(&self.binding.arg_types, args)?;

        // 3. Load library + look up symbol.
        let symbol_bytes = format!("{}\0", self.binding.symbol);
        let raw_ptr = {
            let mut cache = self
                .cache
                .lock()
                .map_err(|e| EvalError::Other(format!("ffi: cache lock poisoned: {}", e)))?;
            let lib = cache.get_or_load(&self.binding.library)?;
            // SAFETY: the symbol name is null-terminated and the library is
            // a valid loaded handle. The returned pointer is a raw C symbol
            // address (or an error if not found).
            let sym: libloading::Symbol<*mut c_void> =
                unsafe { lib.get(symbol_bytes.as_bytes()) }.map_err(|e| {
                    EvalError::Other(format!(
                        "ffi: symbol '{}' not found in '{}': {}",
                        self.binding.symbol, self.binding.library, e
                    ))
                })?;
            *sym
        };

        // 4. Call the raw C function + 5. Unmarshal return.
        call_and_unmarshal(raw_ptr, &marshalled, &self.binding.return_type, &self.binding.symbol)
    }
}

// ---------------------------------------------------------------------------
// Marshalling
// ---------------------------------------------------------------------------

/// A marshalled C argument value.
enum MarshalledArg {
    I32(i32),
    I64(i64),
    F64(f64),
    CStr(CString),
    Buf(Vec<u8>),
    Ptr(*mut c_void),
}

/// Marshal handl `Value` args to C types per the descriptor list.
///
/// Returns a `Vec<MarshalledArg>` aligned with `arg_types`. The caller must
/// keep the `CString` and `Vec<u8>` alive until after the C call (their
/// pointers are passed to the C function).
fn marshal_args(
    arg_types: &[MarshalType],
    args: &[Value],
) -> Result<Vec<MarshalledArg>, EvalError> {
    arg_types
        .iter()
        .zip(args.iter())
        .map(|(ty, val)| marshal_one(ty, val))
        .collect()
}

/// Marshal a single handl `Value` to a C type per the descriptor.
fn marshal_one(ty: &MarshalType, val: &Value) -> Result<MarshalledArg, EvalError> {
    match ty {
        MarshalType::I32 => {
            let n = val
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("ffi i32: expected number".to_string()))?;
            Ok(MarshalledArg::I32(n as i32))
        }
        MarshalType::I64 => {
            let n = val
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("ffi i64: expected number".to_string()))?;
            Ok(MarshalledArg::I64(n))
        }
        MarshalType::F64 => {
            let n = val
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("ffi f64: expected number".to_string()))?;
            Ok(MarshalledArg::F64(n))
        }
        MarshalType::CStr => {
            let s = val
                .as_str()
                .ok_or_else(|| EvalError::TypeMismatch("ffi cstr: expected string".to_string()))?;
            let c = CString::new(s).map_err(|e| {
                EvalError::Other(format!("ffi cstr: string contains null byte: {}", e))
            })?;
            Ok(MarshalledArg::CStr(c))
        }
        MarshalType::Buf => {
            let arr = val
                .as_array()
                .ok_or_else(|| EvalError::TypeMismatch("ffi buf: expected array".to_string()))?;
            let bytes: Vec<u8> = arr
                .iter()
                .map(|b| {
                    b.as_i64()
                        .map(|n| n as u8)
                        .ok_or_else(|| EvalError::TypeMismatch("ffi buf: expected byte array".to_string()))
                })
                .collect::<Result<Vec<u8>, EvalError>>()?;
            Ok(MarshalledArg::Buf(bytes))
        }
        MarshalType::Ptr => {
            let n = val
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("ffi ptr: expected number (handle)".to_string()))?;
            Ok(MarshalledArg::Ptr(n as *mut c_void))
        }
        MarshalType::Handle => {
            let n = val
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("ffi handle: expected number (handle)".to_string()))?;
            Ok(MarshalledArg::Ptr(n as *mut c_void))
        }
        MarshalType::Void => {
            Err(EvalError::TypeMismatch("ffi: void is not a valid arg type".to_string()))
        }
    }
}

/// Call the raw C function pointer and unmarshal the return value.
///
/// Since Rust doesn't support variadic C calls with dynamic types, we match
/// on the total C arg count and the specific type combination. For the
/// common cases (0–4 C args), we transmute the raw symbol pointer to the
/// right `extern "C" fn(...)` type and call it.
///
/// # Safety
///
/// `raw_ptr` must be a valid C function pointer matching the marshalled
/// argument types and return type. The marshalled args must outlive the call
/// (their pointers are dereferenced by the C function).
fn call_and_unmarshal(
    raw_ptr: *mut c_void,
    args: &[MarshalledArg],
    return_type: &MarshalType,
    symbol: &str,
) -> Result<Value, EvalError> {
    // Compute the total number of C arguments (buf = 2).
    let c_arg_count: usize = args
        .iter()
        .map(|a| match a {
            MarshalledArg::Buf(_) => 2,
            _ => 1,
        })
        .sum();

    // SAFETY: raw_ptr is a valid C function pointer obtained from dlsym.
    // The caller guarantees the types match. We transmute to the correct
    // function pointer type and call it with the marshalled arguments.
    unsafe {
        match c_arg_count {
            0 => call_0_args(raw_ptr, return_type, symbol),
            1 => call_1_arg(raw_ptr, &args[0], return_type, symbol),
            2 => call_2_args(raw_ptr, args, return_type, symbol),
            3 => call_3_args(raw_ptr, args, return_type, symbol),
            4 => call_4_args(raw_ptr, args, return_type, symbol),
            n => Err(EvalError::Other(format!(
                "ffi '{}': unsupported C arg count {} (max 4)",
                symbol, n
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Typed call helpers (0–4 C args)
// ---------------------------------------------------------------------------

/// Extract the raw C value from a marshalled arg for passing to a C function.
///
/// Returns `(i32_val, i64_val, f64_val, ptr_val)` — the caller selects the
/// appropriate field based on the function pointer type.
fn extract_c_value(arg: &MarshalledArg) -> (i32, i64, f64, *mut c_void) {
    match arg {
        MarshalledArg::I32(v) => (*v, 0, 0.0, std::ptr::null_mut()),
        MarshalledArg::I64(v) => (0, *v, 0.0, std::ptr::null_mut()),
        MarshalledArg::F64(v) => (0, 0, *v, std::ptr::null_mut()),
        MarshalledArg::CStr(c) => (0, 0, 0.0, c.as_ptr() as *mut c_void),
        MarshalledArg::Buf(b) => (0, 0, 0.0, b.as_ptr() as *mut c_void),
        MarshalledArg::Ptr(p) => (0, 0, 0.0, *p),
    }
}

/// Call a C function with 0 arguments.
unsafe fn call_0_args(
    raw_ptr: *mut c_void,
    return_type: &MarshalType,
    symbol: &str,
) -> Result<Value, EvalError> {
    // SAFETY: raw_ptr is a valid C function pointer. We transmute to the
    // correct return type and call it.
    unsafe {
        match return_type {
            MarshalType::I32 => {
                let f: extern "C" fn() -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f() as i64))
            }
            MarshalType::I64 => {
                let f: extern "C" fn() -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f()))
            }
            MarshalType::F64 => {
                let f: extern "C" fn() -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f()))
            }
            MarshalType::CStr => {
                let f: extern "C" fn() -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f();
                if ptr.is_null() {
                    return Ok(Value::Null);
                }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr
                    .to_str()
                    .map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            MarshalType::Ptr | MarshalType::Handle => {
                let f: extern "C" fn() -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f() as i64))
            }
            MarshalType::Void => {
                let f: extern "C" fn() = std::mem::transmute(raw_ptr);
                f();
                Ok(Value::Null)
            }
            MarshalType::Buf => {
                Err(EvalError::Other("ffi: buf is not a valid return type".to_string()))
            }
        }
    }
}

/// Call a C function with 1 C argument.
unsafe fn call_1_arg(
    raw_ptr: *mut c_void,
    arg: &MarshalledArg,
    return_type: &MarshalType,
    symbol: &str,
) -> Result<Value, EvalError> {
    let (a_i32, a_i64, a_f64, a_ptr) = extract_c_value(arg);
    // SAFETY: raw_ptr is a valid C function pointer matching the arg type.
    unsafe {
        match (arg, return_type) {
            // i32 arg
            (MarshalledArg::I32(_), MarshalType::I32) => {
                let f: extern "C" fn(i32) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i32) as i64))
            }
            (MarshalledArg::I32(_), MarshalType::I64) => {
                let f: extern "C" fn(i32) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i32)))
            }
            (MarshalledArg::I32(_), MarshalType::F64) => {
                let f: extern "C" fn(i32) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i32)))
            }
            (MarshalledArg::I32(_), MarshalType::Void) => {
                let f: extern "C" fn(i32) = std::mem::transmute(raw_ptr);
                f(a_i32);
                Ok(Value::Null)
            }
            (MarshalledArg::I32(_), MarshalType::Ptr) | (MarshalledArg::I32(_), MarshalType::Handle) => {
                let f: extern "C" fn(i32) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i32) as i64))
            }
            (MarshalledArg::I32(_), MarshalType::CStr) => {
                let f: extern "C" fn(i32) -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f(a_i32);
                if ptr.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            // i64 arg
            (MarshalledArg::I64(_), MarshalType::I32) => {
                let f: extern "C" fn(i64) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i64) as i64))
            }
            (MarshalledArg::I64(_), MarshalType::I64) => {
                let f: extern "C" fn(i64) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i64)))
            }
            (MarshalledArg::I64(_), MarshalType::F64) => {
                let f: extern "C" fn(i64) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i64)))
            }
            (MarshalledArg::I64(_), MarshalType::Void) => {
                let f: extern "C" fn(i64) = std::mem::transmute(raw_ptr);
                f(a_i64);
                Ok(Value::Null)
            }
            (MarshalledArg::I64(_), MarshalType::Ptr) | (MarshalledArg::I64(_), MarshalType::Handle) => {
                let f: extern "C" fn(i64) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_i64) as i64))
            }
            (MarshalledArg::I64(_), MarshalType::CStr) => {
                let f: extern "C" fn(i64) -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f(a_i64);
                if ptr.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            // f64 arg
            (MarshalledArg::F64(_), MarshalType::I32) => {
                let f: extern "C" fn(f64) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_f64) as i64))
            }
            (MarshalledArg::F64(_), MarshalType::I64) => {
                let f: extern "C" fn(f64) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_f64)))
            }
            (MarshalledArg::F64(_), MarshalType::F64) => {
                let f: extern "C" fn(f64) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_f64)))
            }
            (MarshalledArg::F64(_), MarshalType::Void) => {
                let f: extern "C" fn(f64) = std::mem::transmute(raw_ptr);
                f(a_f64);
                Ok(Value::Null)
            }
            (MarshalledArg::F64(_), MarshalType::Ptr) | (MarshalledArg::F64(_), MarshalType::Handle) => {
                let f: extern "C" fn(f64) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_f64) as i64))
            }
            (MarshalledArg::F64(_), MarshalType::CStr) => {
                let f: extern "C" fn(f64) -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f(a_f64);
                if ptr.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            // cstr arg
            (MarshalledArg::CStr(_), MarshalType::I32) => {
                let f: extern "C" fn(*const c_char) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr as *const c_char) as i64))
            }
            (MarshalledArg::CStr(_), MarshalType::I64) => {
                let f: extern "C" fn(*const c_char) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr as *const c_char)))
            }
            (MarshalledArg::CStr(_), MarshalType::F64) => {
                let f: extern "C" fn(*const c_char) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr as *const c_char)))
            }
            (MarshalledArg::CStr(_), MarshalType::Void) => {
                let f: extern "C" fn(*const c_char) = std::mem::transmute(raw_ptr);
                f(a_ptr as *const c_char);
                Ok(Value::Null)
            }
            (MarshalledArg::CStr(_), MarshalType::Ptr) | (MarshalledArg::CStr(_), MarshalType::Handle) => {
                let f: extern "C" fn(*const c_char) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr as *const c_char) as i64))
            }
            (MarshalledArg::CStr(_), MarshalType::CStr) => {
                let f: extern "C" fn(*const c_char) -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f(a_ptr as *const c_char);
                if ptr.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            // ptr/handle arg
            (MarshalledArg::Ptr(_), MarshalType::I32) => {
                let f: extern "C" fn(*mut c_void) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr) as i64))
            }
            (MarshalledArg::Ptr(_), MarshalType::I64) => {
                let f: extern "C" fn(*mut c_void) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr)))
            }
            (MarshalledArg::Ptr(_), MarshalType::F64) => {
                let f: extern "C" fn(*mut c_void) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr)))
            }
            (MarshalledArg::Ptr(_), MarshalType::Void) => {
                let f: extern "C" fn(*mut c_void) = std::mem::transmute(raw_ptr);
                f(a_ptr);
                Ok(Value::Null)
            }
            (MarshalledArg::Ptr(_), MarshalType::Ptr) | (MarshalledArg::Ptr(_), MarshalType::Handle) => {
                let f: extern "C" fn(*mut c_void) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a_ptr) as i64))
            }
            (MarshalledArg::Ptr(_), MarshalType::CStr) => {
                let f: extern "C" fn(*mut c_void) -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f(a_ptr);
                if ptr.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            // buf = 2 C args, handled in call_2_args
            (MarshalledArg::Buf(_), _) => {
                // This shouldn't happen for a 1-arg call (buf is 2 C args).
                // But if the binding has 1 handl arg that is buf, c_arg_count
                // would be 2 and we'd be in call_2_args. So this is unreachable
                // from the normal path. Handle it anyway for safety.
                call_2_args(raw_ptr, std::slice::from_ref(arg), return_type, symbol)
            }
            // Catch-all for remaining (arg, return_type) combinations that
            // produce Buf return (which is invalid).
            (_, MarshalType::Buf) => {
                Err(EvalError::Other("ffi: buf is not a valid return type".to_string()))
            }
        }
    }
}

/// Call a C function with 2 C arguments.
unsafe fn call_2_args(
    raw_ptr: *mut c_void,
    args: &[MarshalledArg],
    return_type: &MarshalType,
    symbol: &str,
) -> Result<Value, EvalError> {
    // Special case: a single Buf handl arg expands to (ptr, len) = 2 C args.
    if args.len() == 1
        && let MarshalledArg::Buf(ref b) = args[0]
    {
        let ptr = b.as_ptr() as *mut c_void;
        let len = b.len();
        return unsafe { call_buf_2(raw_ptr, ptr, len, return_type, symbol) };
    }

    let (a_i32, a_i64, a_f64, a_ptr) = extract_c_value(&args[0]);
    let (b_i32, b_i64, b_f64, b_ptr) = extract_c_value(&args[1]);

    // SAFETY: raw_ptr is a valid C function pointer matching the arg types.
    unsafe {
        // i32, i32
        if matches!(args[0], MarshalledArg::I32(_)) && matches!(args[1], MarshalledArg::I32(_)) {
            return call_typed_2::<i32, i32>(raw_ptr, a_i32, b_i32, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I32(_)) && matches!(args[1], MarshalledArg::I64(_)) {
            return call_typed_2::<i32, i64>(raw_ptr, a_i32, b_i64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I32(_)) && matches!(args[1], MarshalledArg::F64(_)) {
            return call_typed_2::<i32, f64>(raw_ptr, a_i32, b_f64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I32(_)) && matches!(args[1], MarshalledArg::CStr(_)) {
            return call_typed_2::<i32, *const c_char>(raw_ptr, a_i32, b_ptr as *const c_char, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I32(_)) && matches!(args[1], MarshalledArg::Ptr(_)) {
            return call_typed_2::<i32, *mut c_void>(raw_ptr, a_i32, b_ptr, return_type, symbol);
        }
        // i64, *
        if matches!(args[0], MarshalledArg::I64(_)) && matches!(args[1], MarshalledArg::I32(_)) {
            return call_typed_2::<i64, i32>(raw_ptr, a_i64, b_i32, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I64(_)) && matches!(args[1], MarshalledArg::I64(_)) {
            return call_typed_2::<i64, i64>(raw_ptr, a_i64, b_i64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I64(_)) && matches!(args[1], MarshalledArg::F64(_)) {
            return call_typed_2::<i64, f64>(raw_ptr, a_i64, b_f64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I64(_)) && matches!(args[1], MarshalledArg::CStr(_)) {
            return call_typed_2::<i64, *const c_char>(raw_ptr, a_i64, b_ptr as *const c_char, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::I64(_)) && matches!(args[1], MarshalledArg::Ptr(_)) {
            return call_typed_2::<i64, *mut c_void>(raw_ptr, a_i64, b_ptr, return_type, symbol);
        }
        // f64, *
        if matches!(args[0], MarshalledArg::F64(_)) && matches!(args[1], MarshalledArg::I32(_)) {
            return call_typed_2::<f64, i32>(raw_ptr, a_f64, b_i32, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::F64(_)) && matches!(args[1], MarshalledArg::I64(_)) {
            return call_typed_2::<f64, i64>(raw_ptr, a_f64, b_i64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::F64(_)) && matches!(args[1], MarshalledArg::F64(_)) {
            return call_typed_2::<f64, f64>(raw_ptr, a_f64, b_f64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::F64(_)) && matches!(args[1], MarshalledArg::CStr(_)) {
            return call_typed_2::<f64, *const c_char>(raw_ptr, a_f64, b_ptr as *const c_char, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::F64(_)) && matches!(args[1], MarshalledArg::Ptr(_)) {
            return call_typed_2::<f64, *mut c_void>(raw_ptr, a_f64, b_ptr, return_type, symbol);
        }
        // cstr, *
        if matches!(args[0], MarshalledArg::CStr(_)) && matches!(args[1], MarshalledArg::I32(_)) {
            return call_typed_2::<*const c_char, i32>(raw_ptr, a_ptr as *const c_char, b_i32, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::CStr(_)) && matches!(args[1], MarshalledArg::I64(_)) {
            return call_typed_2::<*const c_char, i64>(raw_ptr, a_ptr as *const c_char, b_i64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::CStr(_)) && matches!(args[1], MarshalledArg::F64(_)) {
            return call_typed_2::<*const c_char, f64>(raw_ptr, a_ptr as *const c_char, b_f64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::CStr(_)) && matches!(args[1], MarshalledArg::CStr(_)) {
            return call_typed_2::<*const c_char, *const c_char>(raw_ptr, a_ptr as *const c_char, b_ptr as *const c_char, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::CStr(_)) && matches!(args[1], MarshalledArg::Ptr(_)) {
            return call_typed_2::<*const c_char, *mut c_void>(raw_ptr, a_ptr as *const c_char, b_ptr, return_type, symbol);
        }
        // ptr, *
        if matches!(args[0], MarshalledArg::Ptr(_)) && matches!(args[1], MarshalledArg::I32(_)) {
            return call_typed_2::<*mut c_void, i32>(raw_ptr, a_ptr, b_i32, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::Ptr(_)) && matches!(args[1], MarshalledArg::I64(_)) {
            return call_typed_2::<*mut c_void, i64>(raw_ptr, a_ptr, b_i64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::Ptr(_)) && matches!(args[1], MarshalledArg::F64(_)) {
            return call_typed_2::<*mut c_void, f64>(raw_ptr, a_ptr, b_f64, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::Ptr(_)) && matches!(args[1], MarshalledArg::CStr(_)) {
            return call_typed_2::<*mut c_void, *const c_char>(raw_ptr, a_ptr, b_ptr as *const c_char, return_type, symbol);
        }
        if matches!(args[0], MarshalledArg::Ptr(_)) && matches!(args[1], MarshalledArg::Ptr(_)) {
            return call_typed_2::<*mut c_void, *mut c_void>(raw_ptr, a_ptr, b_ptr, return_type, symbol);
        }

        Err(EvalError::Other(format!(
            "ffi '{}': unsupported 2-arg type combination",
            symbol
        )))
    }
}

/// Call a C function with a (ptr, len) buf pair.
unsafe fn call_buf_2(
    raw_ptr: *mut c_void,
    ptr: *mut c_void,
    len: usize,
    return_type: &MarshalType,
    symbol: &str,
) -> Result<Value, EvalError> {
    // SAFETY: raw_ptr is a valid C function pointer taking (void*, size_t).
    unsafe {
        match return_type {
            MarshalType::I32 => {
                let f: extern "C" fn(*mut c_void, usize) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(ptr, len) as i64))
            }
            MarshalType::I64 => {
                let f: extern "C" fn(*mut c_void, usize) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(ptr, len)))
            }
            MarshalType::F64 => {
                let f: extern "C" fn(*mut c_void, usize) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(ptr, len)))
            }
            MarshalType::Void => {
                let f: extern "C" fn(*mut c_void, usize) = std::mem::transmute(raw_ptr);
                f(ptr, len);
                Ok(Value::Null)
            }
            MarshalType::Ptr | MarshalType::Handle => {
                let f: extern "C" fn(*mut c_void, usize) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(ptr, len) as i64))
            }
            MarshalType::CStr => {
                let f: extern "C" fn(*mut c_void, usize) -> *const c_char = std::mem::transmute(raw_ptr);
                let ret = f(ptr, len);
                if ret.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ret);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            MarshalType::Buf => {
                Err(EvalError::Other("ffi: buf is not a valid return type".to_string()))
            }
        }
    }
}

/// Generic 2-arg typed call helper.
///
/// Calls a C function with two typed arguments and unmarshals the return.
///
/// # Safety
///
/// `raw_ptr` must be a valid C function pointer matching `(A, B) -> R`.
unsafe fn call_typed_2<A, B>(
    raw_ptr: *mut c_void,
    a: A,
    b: B,
    return_type: &MarshalType,
    symbol: &str,
) -> Result<Value, EvalError>
where
    A: Copy,
    B: Copy,
{
    // SAFETY: raw_ptr is a valid C function pointer. We transmute to the
    // correct return type and call it with the typed arguments.
    unsafe {
        match return_type {
            MarshalType::I32 => {
                let f: extern "C" fn(A, B) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b) as i64))
            }
            MarshalType::I64 => {
                let f: extern "C" fn(A, B) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b)))
            }
            MarshalType::F64 => {
                let f: extern "C" fn(A, B) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b)))
            }
            MarshalType::Void => {
                let f: extern "C" fn(A, B) = std::mem::transmute(raw_ptr);
                f(a, b);
                Ok(Value::Null)
            }
            MarshalType::Ptr | MarshalType::Handle => {
                let f: extern "C" fn(A, B) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b) as i64))
            }
            MarshalType::CStr => {
                let f: extern "C" fn(A, B) -> *const c_char = std::mem::transmute(raw_ptr);
                let ptr = f(a, b);
                if ptr.is_null() { return Ok(Value::Null); }
                let cstr = CStr::from_ptr(ptr);
                let s = cstr.to_str().map_err(|e| EvalError::Other(format!("ffi '{}': non-utf8 return: {}", symbol, e)))?;
                Ok(Value::String(s.to_string()))
            }
            MarshalType::Buf => {
                Err(EvalError::Other("ffi: buf is not a valid return type".to_string()))
            }
        }
    }
}

/// Call a C function with 3 C arguments.
unsafe fn call_3_args(
    raw_ptr: *mut c_void,
    args: &[MarshalledArg],
    return_type: &MarshalType,
    _symbol: &str,
) -> Result<Value, EvalError> {
    // For 3+ args, we support the common combinations of i32/i64/cstr/ptr.
    // This covers cases like open(path, flags, mode) = (cstr, i32, i32).
    let (a_i32, a_i64, _a_f64, a_ptr) = extract_c_value(&args[0]);
    let (b_i32, _b_i64, _b_f64, b_ptr) = extract_c_value(&args[1]);
    let (c_i32, c_i64, _c_f64, _c_ptr) = extract_c_value(&args[2]);

    // SAFETY: raw_ptr is a valid C function pointer matching the arg types.
    unsafe {
        // (cstr, i32, i32) — e.g. open(path, flags, mode)
        if matches!(args[0], MarshalledArg::CStr(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
        {
            return call_typed_3::<*const c_char, i32, i32>(
                raw_ptr,
                a_ptr as *const c_char,
                b_i32,
                c_i32,
                return_type,
            );
        }
        // (i32, i32, i32)
        if matches!(args[0], MarshalledArg::I32(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
        {
            return call_typed_3::<i32, i32, i32>(raw_ptr, a_i32, b_i32, c_i32, return_type);
        }
        // (ptr, i32, i32) — e.g. read(fd, buf, count) with fd as ptr
        if matches!(args[0], MarshalledArg::Ptr(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
        {
            return call_typed_3::<*mut c_void, i32, i32>(raw_ptr, a_ptr, b_i32, c_i32, return_type);
        }
        // (i64, i32, i32)
        if matches!(args[0], MarshalledArg::I64(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
        {
            return call_typed_3::<i64, i32, i32>(raw_ptr, a_i64, b_i32, c_i32, return_type);
        }
        // (cstr, i32, i64)
        if matches!(args[0], MarshalledArg::CStr(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I64(_))
        {
            return call_typed_3::<*const c_char, i32, i64>(
                raw_ptr,
                a_ptr as *const c_char,
                b_i32,
                c_i64,
                return_type,
            );
        }
        // (ptr, ptr, i32)
        if matches!(args[0], MarshalledArg::Ptr(_))
            && matches!(args[1], MarshalledArg::Ptr(_))
            && matches!(args[2], MarshalledArg::I32(_))
        {
            return call_typed_3::<*mut c_void, *mut c_void, i32>(raw_ptr, a_ptr, b_ptr, c_i32, return_type);
        }

        Err(EvalError::Other(
            "ffi: unsupported 3-arg type combination".to_string(),
        ))
    }
}

/// Generic 3-arg typed call helper.
unsafe fn call_typed_3<A, B, C>(
    raw_ptr: *mut c_void,
    a: A,
    b: B,
    c: C,
    return_type: &MarshalType,
) -> Result<Value, EvalError>
where
    A: Copy,
    B: Copy,
    C: Copy,
{
    // SAFETY: raw_ptr is a valid C function pointer.
    unsafe {
        match return_type {
            MarshalType::I32 => {
                let f: extern "C" fn(A, B, C) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c) as i64))
            }
            MarshalType::I64 => {
                let f: extern "C" fn(A, B, C) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c)))
            }
            MarshalType::F64 => {
                let f: extern "C" fn(A, B, C) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c)))
            }
            MarshalType::Void => {
                let f: extern "C" fn(A, B, C) = std::mem::transmute(raw_ptr);
                f(a, b, c);
                Ok(Value::Null)
            }
            MarshalType::Ptr | MarshalType::Handle => {
                let f: extern "C" fn(A, B, C) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c) as i64))
            }
            _ => Err(EvalError::Other(
                "ffi: unsupported return type for 3-arg call".to_string(),
            )),
        }
    }
}

/// Call a C function with 4 C arguments.
unsafe fn call_4_args(
    raw_ptr: *mut c_void,
    args: &[MarshalledArg],
    return_type: &MarshalType,
    _symbol: &str,
) -> Result<Value, EvalError> {
    let (a_i32, _a_i64, _a_f64, a_ptr) = extract_c_value(&args[0]);
    let (b_i32, _b_i64, _b_f64, b_ptr) = extract_c_value(&args[1]);
    let (c_i32, _c_i64, _c_f64, c_ptr) = extract_c_value(&args[2]);
    let (d_i32, _d_i64, _d_f64, _d_ptr) = extract_c_value(&args[3]);

    // SAFETY: raw_ptr is a valid C function pointer matching the arg types.
    unsafe {
        // (i32, i32, i32, i32)
        if matches!(args[0], MarshalledArg::I32(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
            && matches!(args[3], MarshalledArg::I32(_))
        {
            return call_typed_4::<i32, i32, i32, i32>(raw_ptr, a_i32, b_i32, c_i32, d_i32, return_type);
        }
        // (cstr, i32, i32, i32)
        if matches!(args[0], MarshalledArg::CStr(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
            && matches!(args[3], MarshalledArg::I32(_))
        {
            return call_typed_4::<*const c_char, i32, i32, i32>(
                raw_ptr,
                a_ptr as *const c_char,
                b_i32,
                c_i32,
                d_i32,
                return_type,
            );
        }
        // (ptr, i32, i32, i32)
        if matches!(args[0], MarshalledArg::Ptr(_))
            && matches!(args[1], MarshalledArg::I32(_))
            && matches!(args[2], MarshalledArg::I32(_))
            && matches!(args[3], MarshalledArg::I32(_))
        {
            return call_typed_4::<*mut c_void, i32, i32, i32>(raw_ptr, a_ptr, b_i32, c_i32, d_i32, return_type);
        }
        // (ptr, ptr, i32, i32)
        if matches!(args[0], MarshalledArg::Ptr(_))
            && matches!(args[1], MarshalledArg::Ptr(_))
            && matches!(args[2], MarshalledArg::I32(_))
            && matches!(args[3], MarshalledArg::I32(_))
        {
            return call_typed_4::<*mut c_void, *mut c_void, i32, i32>(raw_ptr, a_ptr, b_ptr, c_i32, d_i32, return_type);
        }
        // (ptr, ptr, ptr, i32)
        if matches!(args[0], MarshalledArg::Ptr(_))
            && matches!(args[1], MarshalledArg::Ptr(_))
            && matches!(args[2], MarshalledArg::Ptr(_))
            && matches!(args[3], MarshalledArg::I32(_))
        {
            return call_typed_4::<*mut c_void, *mut c_void, *mut c_void, i32>(
                raw_ptr, a_ptr, b_ptr, c_ptr, d_i32, return_type,
            );
        }

        Err(EvalError::Other(
            "ffi: unsupported 4-arg type combination".to_string(),
        ))
    }
}

/// Generic 4-arg typed call helper.
unsafe fn call_typed_4<A, B, C, D>(
    raw_ptr: *mut c_void,
    a: A,
    b: B,
    c: C,
    d: D,
    return_type: &MarshalType,
) -> Result<Value, EvalError>
where
    A: Copy,
    B: Copy,
    C: Copy,
    D: Copy,
{
    // SAFETY: raw_ptr is a valid C function pointer.
    unsafe {
        match return_type {
            MarshalType::I32 => {
                let f: extern "C" fn(A, B, C, D) -> i32 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c, d) as i64))
            }
            MarshalType::I64 => {
                let f: extern "C" fn(A, B, C, D) -> i64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c, d)))
            }
            MarshalType::F64 => {
                let f: extern "C" fn(A, B, C, D) -> f64 = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c, d)))
            }
            MarshalType::Void => {
                let f: extern "C" fn(A, B, C, D) = std::mem::transmute(raw_ptr);
                f(a, b, c, d);
                Ok(Value::Null)
            }
            MarshalType::Ptr | MarshalType::Handle => {
                let f: extern "C" fn(A, B, C, D) -> *mut c_void = std::mem::transmute(raw_ptr);
                Ok(Value::from(f(a, b, c, d) as i64))
            }
            _ => Err(EvalError::Other(
                "ffi: unsupported return type for 4-arg call".to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// FfiLibraryCache + register_ffi_op
// ---------------------------------------------------------------------------

/// A shared library cache for FFI dispatchers (ADR-0083 §4: "dlopen once,
/// reuse").
///
/// Create one of these at engine/host startup and use
/// [`FfiLibraryCache::dispatcher`] to create [`FfiDispatcher`] instances that
/// share the same cached `dlopen` handles.
#[derive(Default)]
pub struct FfiLibraryCache {
    inner: Arc<Mutex<LibraryCache>>,
}

impl FfiLibraryCache {
    /// Create a new, empty `FfiLibraryCache`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an [`FfiDispatcher`] for the given binding, sharing this cache.
    pub fn dispatcher(&self, binding: FfiBinding) -> FfiDispatcher {
        FfiDispatcher::with_cache(binding, Arc::clone(&self.inner))
    }
}

/// Create an [`FfiDispatcher`] and register it in an [`OpRegistry`] under an
/// op name.
///
/// This is the convenience function for wiring an FFI binding into the
/// existing [`OpRegistry`] dispatch (ADR-0083 §4: "An FfiOpFunction is just
/// a `Function` whose `call` does the marshal/call/unmarshal dance").
pub fn register_ffi_op(registry: &mut OpRegistry, op_name: &str, binding: FfiBinding) {
    registry.register(op_name, Arc::new(FfiDispatcher::new(binding)));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Function};
    use serde_json::json;

    // --- Test C functions (loaded via Library::this()) ---

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_double_i32(x: i32) -> i32 {
        x * 2
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_add_i32(a: i32, b: i32) -> i32 {
        a + b
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_identity_cstr(s: *const c_char) -> *const c_char {
        if s.is_null() {
            return std::ptr::null();
        }
        s
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_void_i32(_x: i32) {
        // no-op
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_triple_i64(x: i64) -> i64 {
        x * 3
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_double_f64(x: f64) -> f64 {
        x * 2.0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_no_args_i32() -> i32 {
        42
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_test_void_no_args() {
        // no-op
    }

    // --- MarshalType tests ---

    #[test]
    fn marshal_type_parse_all_descriptors() {
        assert_eq!(MarshalType::parse("i32"), Some(MarshalType::I32));
        assert_eq!(MarshalType::parse("i64"), Some(MarshalType::I64));
        assert_eq!(MarshalType::parse("f64"), Some(MarshalType::F64));
        assert_eq!(MarshalType::parse("cstr"), Some(MarshalType::CStr));
        assert_eq!(MarshalType::parse("buf"), Some(MarshalType::Buf));
        assert_eq!(MarshalType::parse("ptr"), Some(MarshalType::Ptr));
        assert_eq!(MarshalType::parse("void"), Some(MarshalType::Void));
        assert_eq!(MarshalType::parse("handle"), Some(MarshalType::Handle));
    }

    #[test]
    fn marshal_type_parse_unknown_returns_none() {
        assert_eq!(MarshalType::parse("xyz"), None);
        assert_eq!(MarshalType::parse(""), None);
        assert_eq!(MarshalType::parse("i128"), None);
    }

    #[test]
    fn marshal_type_c_arg_count() {
        assert_eq!(MarshalType::I32.c_arg_count(), 1);
        assert_eq!(MarshalType::I64.c_arg_count(), 1);
        assert_eq!(MarshalType::F64.c_arg_count(), 1);
        assert_eq!(MarshalType::CStr.c_arg_count(), 1);
        assert_eq!(MarshalType::Buf.c_arg_count(), 2);
        assert_eq!(MarshalType::Ptr.c_arg_count(), 1);
        assert_eq!(MarshalType::Void.c_arg_count(), 1);
        assert_eq!(MarshalType::Handle.c_arg_count(), 1);
    }

    // --- FfiBinding construction ---

    #[test]
    fn ffi_binding_construction() {
        let binding = FfiBinding {
            library: "libc.so.6".to_string(),
            symbol: "open".to_string(),
            arg_types: vec![MarshalType::CStr, MarshalType::I32],
            return_type: MarshalType::I32,
        };
        assert_eq!(binding.library, "libc.so.6");
        assert_eq!(binding.symbol, "open");
        assert_eq!(binding.arg_types.len(), 2);
        assert_eq!(binding.return_type, MarshalType::I32);
    }

    // --- FfiDispatcher raw C call tests ---

    fn make_dispatcher(symbol: &str, arg_types: Vec<MarshalType>, return_type: MarshalType) -> FfiDispatcher {
        FfiDispatcher::new(FfiBinding {
            library: "self".to_string(),
            symbol: symbol.to_string(),
            arg_types,
            return_type,
        })
    }

    #[test]
    fn ffi_dispatcher_calls_raw_c_symbol_i32() {
        let dispatcher = make_dispatcher("ffi_test_double_i32", vec![MarshalType::I32], MarshalType::I32);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn ffi_dispatcher_calls_raw_c_symbol_two_args() {
        let dispatcher = make_dispatcher(
            "ffi_test_add_i32",
            vec![MarshalType::I32, MarshalType::I32],
            MarshalType::I32,
        );
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!(17), json!(25)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn ffi_dispatcher_calls_raw_c_symbol_i64() {
        let dispatcher = make_dispatcher("ffi_test_triple_i64", vec![MarshalType::I64], MarshalType::I64);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!(14)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn ffi_dispatcher_calls_raw_c_symbol_f64() {
        let dispatcher = make_dispatcher("ffi_test_double_f64", vec![MarshalType::F64], MarshalType::F64);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!(21.0)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 42.0);
    }

    #[test]
    fn ffi_dispatcher_cstr_marshal_unmarshal() {
        let dispatcher = make_dispatcher(
            "ffi_test_identity_cstr",
            vec![MarshalType::CStr],
            MarshalType::CStr,
        );
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!("hello ffi")], &mut ctx).unwrap();
        assert_eq!(result, json!("hello ffi"));
    }

    #[test]
    fn ffi_dispatcher_void_return() {
        let dispatcher = make_dispatcher("ffi_test_void_i32", vec![MarshalType::I32], MarshalType::Void);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!(99)], &mut ctx).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn ffi_dispatcher_no_args_i32() {
        let dispatcher = make_dispatcher("ffi_test_no_args_i32", vec![], MarshalType::I32);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn ffi_dispatcher_no_args_void() {
        let dispatcher = make_dispatcher("ffi_test_void_no_args", vec![], MarshalType::Void);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[], &mut ctx).unwrap();
        assert_eq!(result, Value::Null);
    }

    // --- Error handling ---

    #[test]
    fn ffi_dispatcher_wrong_arg_count() {
        let dispatcher = make_dispatcher("ffi_test_double_i32", vec![MarshalType::I32], MarshalType::I32);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[json!(1), json!(2)], &mut ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expected 1 args, got 2"));
    }

    #[test]
    fn ffi_dispatcher_wrong_arg_type() {
        let dispatcher = make_dispatcher("ffi_test_double_i32", vec![MarshalType::I32], MarshalType::I32);
        let mut ctx = EvalContext::new();
        // Pass a string where i32 is expected.
        let result = dispatcher.call(&[json!("not a number")], &mut ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expected number"));
    }

    #[test]
    fn ffi_dispatcher_symbol_not_found() {
        let dispatcher = make_dispatcher("nonexistent_symbol_xyz", vec![], MarshalType::I32);
        let mut ctx = EvalContext::new();
        let result = dispatcher.call(&[], &mut ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    // --- register_ffi_op ---

    #[test]
    fn register_ffi_op_dispatches_via_op_function() {
        use handl_plugin_lib_handl::{OpFunction, OpRegistry};
        use std::sync::Arc;

        let mut registry = OpRegistry::new();
        register_ffi_op(
            &mut registry,
            "test.ffi_double",
            FfiBinding {
                library: "self".to_string(),
                symbol: "ffi_test_double_i32".to_string(),
                arg_types: vec![MarshalType::I32],
                return_type: MarshalType::I32,
            },
        );

        let shared = Arc::new(registry);
        let op_func = OpFunction::new("test.ffi_double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    // --- FfiLibraryCache ---

    #[test]
    fn ffi_library_cache_shares_libraries() {
        let cache = FfiLibraryCache::new();
        let d1 = cache.dispatcher(FfiBinding {
            library: "self".to_string(),
            symbol: "ffi_test_double_i32".to_string(),
            arg_types: vec![MarshalType::I32],
            return_type: MarshalType::I32,
        });
        let d2 = cache.dispatcher(FfiBinding {
            library: "self".to_string(),
            symbol: "ffi_test_triple_i64".to_string(),
            arg_types: vec![MarshalType::I64],
            return_type: MarshalType::I64,
        });

        let mut ctx = EvalContext::new();
        let r1 = d1.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(r1, json!(42));

        let r2 = d2.call(&[json!(14)], &mut ctx).unwrap();
        assert_eq!(r2, json!(42));
    }
}
