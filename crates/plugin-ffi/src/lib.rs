//! # handl-plugin-ffi
//!
//! Generic FFI dispatcher for handl (ADR-0083 §4).
//!
//! This crate provides [`FfiOpFunction`], a [`handl_platform::Function`]
//! implementation that dispatches handl calls to arbitrary C symbols loaded
//! via `dlopen` / `dlsym`. It plugs into the existing [`OpRegistry`] with no
//! engine changes.
//!
//! ## Pipeline
//!
//! 1. **Load** the C library via `dlopen` (cached by name — opened once,
//!    reused for the process lifetime).
//! 2. **Look up** the symbol via `dlsym` (cached by library + symbol).
//! 3. **Marshal** each `serde_json::Value` argument to a C type per its
//!    descriptor string.
//! 4. **Call** the C symbol (`unsafe extern "C"`).
//! 5. **Unmarshal** the return (C type → `serde_json::Value`).
//! 6. **Error clause** — when `error_on_lt_zero` is set and the integer return
//!    is negative, extract `errno` (via `__errno_location()` on Linux) and
//!    return `Err(EvalError::Other(format!("errno: {errno}")))`.
//!
//! ## Marshalling descriptors
//!
//! | Descriptor | C type            | JSON source         |
//! |------------|-------------------|---------------------|
//! | `"i32"`    | `int32_t` (value) | number (range-checked) |
//! | `"i64"`    | `int64_t` (value) | number              |
//! | `"f64"`    | `double` (value)  | number              |
//! | `"cstr"`   | `const char *`    | string (NUL-checked)|
//! | `"ptr"`    | `void *`          | number (address)    |
//! | `"void"`   | `void` (return)   | — (returns null)    |
//!
//! Buffer and struct descriptors are deferred until a `Bytes` type exists.
//!
//! [`OpRegistry`]: handl_plugin_lib_handl::OpRegistry

use std::collections::HashMap;
use std::ffi::{CString, c_void};
use std::sync::{Mutex, OnceLock};

use handl_platform::{EvalContext, EvalError, Function};
use libloading::Library;
use serde_json::Value as JsonValue;

use crate::error::last_errno;
use crate::marshal::{CArg, CRet, CRetKind, marshal_arg, return_kind, unmarshal_return};

mod error;
mod marshal;

#[cfg(test)]
mod tests;

/// Maximum number of arguments supported by the dispatcher.
///
/// The call-dispatch macro generates a typed function-pointer transmute for
/// every arity from 0 to this limit. Most POSIX syscalls take six or fewer
/// arguments.
const MAX_ARGS: usize = 6;

// ---------------------------------------------------------------------------
// Global caches (process-lifetime)
// ---------------------------------------------------------------------------

/// Cache of opened libraries: library name → `libloading::Library`.
///
/// Libraries are opened once via `dlopen` and kept resident for the lifetime
/// of the process so that cached symbol pointers remain valid.
static LIBRARY_CACHE: OnceLock<Mutex<HashMap<String, Library>>> = OnceLock::new();

/// Cache of resolved symbols: (library, symbol) → raw symbol pointer.
///
/// The raw pointer is valid as long as the owning [`Library`] stays in
/// [`LIBRARY_CACHE`], which is never evicted.
static SYMBOL_CACHE: OnceLock<Mutex<HashMap<(String, String), SymbolPtr>>> = OnceLock::new();

/// A raw symbol pointer wrapper that is `Send` + `Sync`.
///
/// `*mut c_void` is not `Send` by default (raw pointers are never
/// auto-`Send`/`Sync`). The pointer is only dereferenced inside `unsafe`
/// call sites that hold no lock, so sharing it across threads is sound.
#[derive(Clone, Copy)]
struct SymbolPtr(*mut c_void);
unsafe impl Send for SymbolPtr {}
unsafe impl Sync for SymbolPtr {}

fn library_cache() -> &'static Mutex<HashMap<String, Library>> {
    LIBRARY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn symbol_cache() -> &'static Mutex<HashMap<(String, String), SymbolPtr>> {
    SYMBOL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve a library by name, opening it once and caching the handle.
fn resolve_library(name: &str) -> Result<(), EvalError> {
    let mut cache = library_cache().lock().unwrap();
    if cache.contains_key(name) {
        return Ok(());
    }
    let lib = unsafe { Library::new(name) }
        .map_err(|e| EvalError::Other(format!("dlopen failed for {name}: {e}")))?;
    cache.insert(name.to_string(), lib);
    Ok(())
}

/// Resolve a symbol within a library, caching the raw pointer.
///
/// The library is opened (and cached) on first access; the symbol is looked
/// up via `dlsym` and its raw address cached under `(library, symbol)`.
fn resolve_symbol(library: &str, symbol: &str) -> Result<*mut c_void, EvalError> {
    let key = (library.to_string(), symbol.to_string());

    // Fast path: symbol already cached.
    {
        let cache = symbol_cache().lock().unwrap();
        if let Some(&SymbolPtr(ptr)) = cache.get(&key) {
            return Ok(ptr);
        }
    }

    // Ensure the library is loaded.
    resolve_library(library)?;

    // Look up the symbol and extract the raw pointer.
    let ptr = {
        let cache = library_cache().lock().unwrap();
        let lib = cache
            .get(library)
            .ok_or_else(|| EvalError::Other(format!("library not loaded: {library}")))?;
        let sym: libloading::Symbol<unsafe extern "C" fn()> =
            unsafe { lib.get(symbol.as_bytes()) }
                .map_err(|e| EvalError::Other(format!("dlsym failed for {symbol}: {e}")))?;
        unsafe { sym.try_as_raw_ptr() }
            .ok_or_else(|| EvalError::Other(format!("null symbol: {symbol}")))?
    };

    // Cache and return.
    let mut cache = symbol_cache().lock().unwrap();
    cache.insert(key, SymbolPtr(ptr));
    Ok(ptr)
}

// ---------------------------------------------------------------------------
// Call dispatch (macro-generated typed transmutes)
// ---------------------------------------------------------------------------

/// Invoke a C function pointer with typed arguments, dispatching on the
/// return kind.
///
/// All integer/pointer arguments are passed as `i64` (sound on x86-64 SysV
/// and AArch64 where 32-bit ints are sign-extended in-register); float
/// arguments are passed as `f64` so they land in XMM registers.
macro_rules! invoke {
    ($sym:expr, $ret:expr, [$($t:ty: $v:expr),* $(,)?]) => {
        match $ret {
            CRetKind::Int32 => {
                let f: unsafe extern "C" fn($($t),*) -> i32 = unsafe {
                    std::mem::transmute::<
                        *mut c_void,
                        unsafe extern "C" fn($($t),*) -> i32,
                    >($sym)
                };
                CRet::Int(unsafe { f($($v),*) } as i64)
            }
            CRetKind::Int64 => {
                let f: unsafe extern "C" fn($($t),*) -> i64 = unsafe {
                    std::mem::transmute::<
                        *mut c_void,
                        unsafe extern "C" fn($($t),*) -> i64,
                    >($sym)
                };
                CRet::Int(unsafe { f($($v),*) })
            }
            CRetKind::Float => {
                let f: unsafe extern "C" fn($($t),*) -> f64 = unsafe {
                    std::mem::transmute::<
                        *mut c_void,
                        unsafe extern "C" fn($($t),*) -> f64,
                    >($sym)
                };
                CRet::Float(unsafe { f($($v),*) })
            }
            CRetKind::Void => {
                let f: unsafe extern "C" fn($($t),*) = unsafe {
                    std::mem::transmute::<
                        *mut c_void,
                        unsafe extern "C" fn($($t),*),
                    >($sym)
                };
                unsafe { f($($v),*) };
                CRet::Void
            }
        }
    };
}

/// Recursively dispatch over argument kinds (`Int` / `Float`), building up
/// the typed function-pointer signature, then emit the call via [`invoke!`].
///
/// Each argument position branches on whether it is an integer/pointer
/// (`i64` slot) or a float (`f64` slot), producing 2^N typed call sites for
/// N arguments.
macro_rules! dispatch {
    // Base case: no more indices — emit the call with accumulated types/vals.
    (sym=$sym:expr, ret=$ret:expr, args=$args:expr,
     types=[$($t:ty),* $(,)?], vals=[$($v:expr),* $(,)?], rest=[]) => {
        invoke!($sym, $ret, [$($t: $v),*])
    };
    // Recursive step: process the next index, branching on Int/Float.
    (sym=$sym:expr, ret=$ret:expr, args=$args:expr,
     types=[$($t:ty),* $(,)?], vals=[$($v:expr),* $(,)?],
     rest=[$idx:tt $($rest:tt)*]) => {
        match $args[$idx] {
            CArg::Int(val) => dispatch!(
                sym=$sym, ret=$ret, args=$args,
                types=[$($t,)* i64], vals=[$($v,)* val],
                rest=[$($rest)*]
            ),
            CArg::Float(val) => dispatch!(
                sym=$sym, ret=$ret, args=$args,
                types=[$($t,)* f64], vals=[$($v,)* val],
                rest=[$($rest)*]
            ),
        }
    };
}

/// Generate a typed call function for a fixed arity.
macro_rules! gen_call_fn {
    ($n:ident, [$($idx:tt)*]) => {
        fn $n(sym: *mut c_void, ret: CRetKind, args: &[CArg]) -> CRet {
            let _ = &args;
            dispatch!(
                sym=sym, ret=ret, args=args,
                types=[], vals=[], rest=[$($idx)*]
            )
        }
    };
}

gen_call_fn!(call0, []);
gen_call_fn!(call1, [0]);
gen_call_fn!(call2, [0 1]);
gen_call_fn!(call3, [0 1 2]);
gen_call_fn!(call4, [0 1 2 3]);
gen_call_fn!(call5, [0 1 2 3 4]);
gen_call_fn!(call6, [0 1 2 3 4 5]);

/// Call a C function pointer with the given marshalled arguments.
///
/// Dispatches on arity (0..=[`MAX_ARGS`]), transmuting the raw symbol to the
/// appropriate typed function pointer and invoking it. The return is read
/// according to `ret_kind`.
fn call_c(sym: *mut c_void, ret: CRetKind, args: &[CArg]) -> Result<CRet, EvalError> {
    Ok(match args.len() {
        0 => call0(sym, ret, args),
        1 => call1(sym, ret, args),
        2 => call2(sym, ret, args),
        3 => call3(sym, ret, args),
        4 => call4(sym, ret, args),
        5 => call5(sym, ret, args),
        6 => call6(sym, ret, args),
        n => {
            return Err(EvalError::Other(format!(
                "too many FFI args: {n} (max {MAX_ARGS})"
            )))
        }
    })
}

// ---------------------------------------------------------------------------
// FfiOpFunction
// ---------------------------------------------------------------------------

/// A generic FFI dispatcher function implementing [`Function`].
///
/// Each call resolves the library (cached `dlopen`), resolves the symbol
/// (cached `dlsym`), marshals the JSON arguments to C types per
/// `arg_descriptors`, calls the C symbol, unmarshals the return per
/// `return_descriptor`, and optionally checks the error clause.
///
/// When `error_on_lt_zero` is `true` and the integer return value is
/// negative, the dispatcher reads `errno` synchronously and returns
/// `Err(EvalError::Other(format!("errno: {errno}")))` instead of the raw
/// return value.
#[derive(Clone)]
pub struct FfiOpFunction {
    /// Library name passed to `dlopen` (e.g. `"libc.so.6"`).
    library: String,
    /// Symbol name passed to `dlsym` (e.g. `"getpid"`).
    symbol: String,
    /// Per-argument marshalling descriptors (e.g. `["i32", "cstr"]`).
    arg_descriptors: Vec<String>,
    /// Return-value descriptor (e.g. `"i32"`, `"void"`).
    return_descriptor: String,
    /// When `true`, a negative integer return triggers an errno extraction.
    error_on_lt_zero: bool,
}

impl FfiOpFunction {
    /// Create a new FFI op function.
    ///
    /// - `library` — library name for `dlopen` (e.g. `"libc.so.6"`).
    /// - `symbol` — symbol name for `dlsym` (e.g. `"getpid"`).
    /// - `arg_descriptors` — one descriptor string per argument.
    /// - `return_descriptor` — the return type descriptor.
    /// - `error_on_lt_zero` — enable the negative-return error clause.
    pub fn new(
        library: impl Into<String>,
        symbol: impl Into<String>,
        arg_descriptors: Vec<String>,
        return_descriptor: impl Into<String>,
        error_on_lt_zero: bool,
    ) -> Self {
        Self {
            library: library.into(),
            symbol: symbol.into(),
            arg_descriptors,
            return_descriptor: return_descriptor.into(),
            error_on_lt_zero,
        }
    }
}

impl Function for FfiOpFunction {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        // Arity check.
        if args.len() != self.arg_descriptors.len() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                self.arg_descriptors.len(),
                args.len()
            )));
        }

        // Resolve symbol (cached dlopen + dlsym).
        let sym = resolve_symbol(&self.library, &self.symbol)?;

        // Marshal args. CStrings for `cstr` args are kept alive here.
        let mut keep_alive: Vec<CString> = Vec::with_capacity(args.len());
        let mut c_args: Vec<CArg> = Vec::with_capacity(args.len());
        for (desc, value) in self.arg_descriptors.iter().zip(args.iter()) {
            c_args.push(marshal_arg(desc, value, &mut keep_alive)?);
        }

        // Determine return kind and call.
        let ret_kind = return_kind(&self.return_descriptor)?;
        let ret = call_c(sym, ret_kind, &c_args)?;

        // Error clause: negative integer return → extract errno.
        if self.error_on_lt_zero {
            if let CRet::Int(v) = &ret {
                if *v < 0 {
                    let errno = last_errno();
                    return Err(EvalError::Other(format!("errno: {errno}")));
                }
            }
        }

        // Unmarshal return.
        unmarshal_return(&self.return_descriptor, &ret)
    }
}
