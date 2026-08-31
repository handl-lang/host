//! # Standalone bootstrap entry points (ADR-0044 Phase 8)
//!
//! These `#[unsafe(no_mangle)] pub extern "C"` functions provide the C ABI
//! bridge between a generated C `main` driver and the Rust evaluation
//! context. The standalone binary is produced by linking three pieces:
//!
//! 1. **The cranelift-emitted `.o` file** — contains the `node_N` native
//!    functions (compiled from the AST via [`compile_all_nodes_object`]).
//!    These are declared `Linkage::Export` so their symbols are visible to
//!    the linker.
//! 2. **This crate as a static library** (`crate-type = ["staticlib"]`) —
//!    exports the `helper_*` symbols (the runtime stub) and the
//!    `handl_standalone_*` entry points defined here.
//! 3. **A generated C `main` driver** — declares `extern` prototypes for
//!    the `node_N` functions and the `handl_standalone_*` entry points,
//!    builds the function-pointer table, calls
//!    [`handl_standalone_init`] → [`handl_standalone_run`] → prints the
//!    result → [`handl_standalone_destroy`].
//!
//! The driver embeds the serialized `Ast`, literal pool, and name pool as
//! JSON string constants (produced by [`crate::CraneliftEngine::compile_to_executable`]).
//!
//! [`compile_all_nodes_object`]: crate::compile_all_nodes_object

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

use handl_language::{Ast, LangError, Value};

use crate::compile::CompiledFn;
use crate::{EvalCtx, take_value};

/// Opaque standalone context.
///
/// Owns the deserialized `Ast` (boxed so its heap address is stable) and the
/// [`EvalCtx`] (which holds a raw pointer into the boxed `Ast`). The
/// `Box<Ast>` ensures the `Ast`'s address does not change when the
/// `StandaloneCtx` itself is moved or boxed, keeping the raw pointer in
/// [`EvalCtx`] valid.
pub struct StandaloneCtx {
    /// The deserialized AST (boxed for address stability). Kept alive so the
    /// raw pointer in `ctx` remains valid; never directly read.
    #[allow(dead_code)]
    ast: Box<Ast>,
    /// The evaluation context (holds a raw pointer to `ast`).
    ctx: EvalCtx,
}

/// Initialize a standalone evaluation context.
///
/// Deserializes the `Ast`, literal pool, and name pool from their JSON
/// strings, builds the per-node compiled function-pointer map from the
/// `fn_ptrs` / `fn_indices` arrays, and constructs an [`EvalCtx`] via
/// [`EvalCtx::for_standalone`].
///
/// # Arguments
///
/// - `ast_json`: serialized [`Ast`] (JSON, null-terminated UTF-8).
/// - `literals_json`: serialized `Vec<Value>` constant pool.
/// - `names_json`: serialized `Vec<String>` constant pool.
/// - `fn_ptrs`: array of `n_fns` native function pointers (`node_N`).
/// - `fn_indices`: array of `n_fns` arena indices (matching the pointers).
/// - `n_fns`: number of entries in `fn_ptrs` / `fn_indices`.
///
/// # Returns
///
/// An opaque context pointer on success, or null on failure (the error is
/// printed to stderr). The pointer must be freed with
/// [`handl_standalone_destroy`].
///
/// # Safety
///
/// The JSON strings must be valid null-terminated UTF-8. The `fn_ptrs` and
/// `fn_indices` arrays must contain `n_fns` valid entries, and each pointer
/// in `fn_ptrs` must be the address of a `node_N` function with the
/// signature `fn(*mut u8) -> *mut u8` (as emitted by cranelift).
#[unsafe(no_mangle)]
pub extern "C" fn handl_standalone_init(
    ast_json: *const c_char,
    literals_json: *const c_char,
    names_json: *const c_char,
    fn_ptrs: *const *const c_void,
    fn_indices: *const u64,
    n_fns: usize,
) -> *mut c_void {
    match standalone_init_inner(
        ast_json,
        literals_json,
        names_json,
        fn_ptrs,
        fn_indices,
        n_fns,
    ) {
        Ok(ctx) => Box::into_raw(Box::new(ctx)) as *mut c_void,
        Err(e) => {
            eprintln!("handl_standalone_init error: {}", e);
            std::ptr::null_mut()
        }
    }
}

/// Inner logic for [`handl_standalone_init`], separated for error handling.
fn standalone_init_inner(
    ast_json: *const c_char,
    literals_json: *const c_char,
    names_json: *const c_char,
    fn_ptrs: *const *const c_void,
    fn_indices: *const u64,
    n_fns: usize,
) -> Result<StandaloneCtx, LangError> {
    // Parse the JSON strings.
    let ast_str = unsafe { CStr::from_ptr(ast_json) }
        .to_str()
        .map_err(|e| LangError::Engine(format!("ast_json not valid UTF-8: {}", e)))?;
    let lit_str = unsafe { CStr::from_ptr(literals_json) }
        .to_str()
        .map_err(|e| LangError::Engine(format!("literals_json not valid UTF-8: {}", e)))?;
    let names_str = unsafe { CStr::from_ptr(names_json) }
        .to_str()
        .map_err(|e| LangError::Engine(format!("names_json not valid UTF-8: {}", e)))?;

    let ast: Ast = serde_json::from_str(ast_str)
        .map_err(|e| LangError::Engine(format!("deserialize Ast: {}", e)))?;
    let literals: Vec<Value> = serde_json::from_str(lit_str)
        .map_err(|e| LangError::Engine(format!("deserialize literals: {}", e)))?;
    let names: Vec<String> = serde_json::from_str(names_str)
        .map_err(|e| LangError::Engine(format!("deserialize names: {}", e)))?;

    // Box the Ast so its address is stable.
    let ast = Box::new(ast);

    // Build the compiled function-pointer map from the C arrays.
    let mut compiled = HashMap::with_capacity(n_fns);
    for i in 0..n_fns {
        let ptr = unsafe { *fn_ptrs.add(i) };
        let idx = unsafe { *fn_indices.add(i) } as usize;
        // SAFETY: the caller guarantees ptr is the address of a node_N
        // function with signature fn(*mut u8) -> *mut u8.
        let fn_ptr = unsafe { std::mem::transmute::<*const c_void, CompiledFn>(ptr) };
        compiled.insert(idx, fn_ptr);
    }

    // Construct the EvalCtx. The raw pointer to `ast` is valid because the
    // Box keeps the Ast at a stable heap address.
    let ctx = EvalCtx::for_standalone(&ast, compiled, literals, names);

    Ok(StandaloneCtx { ast, ctx })
}

/// Run the root node and return the result as a JSON string.
///
/// Calls the compiled native function for `root_idx`, checks for deferred
/// errors, and returns the result serialized as JSON. The returned string
/// is owned by the caller and must be freed with
/// [`handl_standalone_free_string`].
///
/// # Returns
///
/// A null-terminated JSON string on success, or null on failure (the error
/// is printed to stderr).
///
/// # Safety
///
/// `ctx` must be a valid pointer returned by [`handl_standalone_init`] and
/// not yet destroyed. `root_idx` must be a valid arena index with a
/// corresponding entry in the compiled function-pointer map.
#[unsafe(no_mangle)]
pub extern "C" fn handl_standalone_run(ctx: *mut c_void, root_idx: u64) -> *mut c_char {
    if ctx.is_null() {
        eprintln!("handl_standalone_run: null context");
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees ctx is a valid StandaloneCtx pointer.
    let standalone = unsafe { &mut *(ctx as *mut StandaloneCtx) };
    match standalone_run_inner(standalone, root_idx as usize) {
        Ok(json) => CString::into_raw(CString::new(json).unwrap_or_else(|_| {
            CString::new("handl_standalone_run: result contains NUL byte").unwrap()
        })),
        Err(e) => {
            eprintln!("handl_standalone_run error: {}", e);
            std::ptr::null_mut()
        }
    }
}

/// Inner logic for [`handl_standalone_run`].
fn standalone_run_inner(
    standalone: &mut StandaloneCtx,
    root_idx: usize,
) -> Result<String, LangError> {
    let ctx = &mut standalone.ctx;

    // Look up the root function.
    let root_fn =
        ctx.compiled.get(&root_idx).copied().ok_or_else(|| {
            LangError::Engine(format!("no compiled function for root {}", root_idx))
        })?;

    // Set current_ref to the root (matches run_native).
    ctx.current_ref = root_idx;

    // Call the root native function.
    let result_ptr: *mut Value = unsafe { root_fn(ctx as *mut EvalCtx as *mut u8) as *mut Value };

    // Check for deferred error.
    if let Some(err) = ctx.error.take() {
        // Reclaim the result pointer if non-null (avoid leak).
        if !result_ptr.is_null() {
            unsafe {
                let _ = take_value(result_ptr);
            }
        }
        return Err(err);
    }

    if result_ptr.is_null() {
        return Err(LangError::Engine(
            "native evaluation returned null without error".to_string(),
        ));
    }

    // Reclaim the result value and serialize to JSON.
    // SAFETY: the helper boxed this Value with Box::into_raw.
    let value = unsafe { take_value(result_ptr) };
    Ok(value.to_string())
}

/// Free a string returned by [`handl_standalone_run`].
///
/// # Safety
///
/// `ptr` must be a valid pointer returned by [`handl_standalone_run`] (or
/// null — calling with null is a no-op).
#[unsafe(no_mangle)]
pub extern "C" fn handl_standalone_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees ptr was returned by CString::into_raw.
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Destroy a standalone context, freeing all memory.
///
/// # Safety
///
/// `ctx` must be a valid pointer returned by [`handl_standalone_init`] (or
/// null — calling with null is a no-op). The pointer must not be used after
/// this call.
#[unsafe(no_mangle)]
pub extern "C" fn handl_standalone_destroy(ctx: *mut c_void) {
    if !ctx.is_null() {
        // SAFETY: caller guarantees ctx is a valid StandaloneCtx pointer.
        unsafe {
            let _ = Box::from_raw(ctx as *mut StandaloneCtx);
        }
    }
}

// ---------------------------------------------------------------------------
// C driver generation + linking (used by CraneliftEngine::compile_to_executable)
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};
use std::process::Command;

/// Escape a string for embedding in a C string literal.
///
/// Escapes backslash (`\` → `\\`) and double-quote (`"` → `\"`). All other
/// characters (including JSON's `\n` escape sequences, which are the literal
/// two characters backslash + n) pass through correctly after the
/// backslash-doubling.
fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

/// Generate the C `main` driver source that links the `.o` file with the
/// runtime stub and drives evaluation via the `handl_standalone_*` entry
/// points.
///
/// The driver:
/// 1. Declares `extern` prototypes for each `node_N` function (from
///    `function_names`).
/// 2. Embeds the serialized `Ast`, literal pool, and name pool as C string
///    constants.
/// 3. Builds the function-pointer / arena-index arrays.
/// 4. Calls `handl_standalone_init` → `handl_standalone_run` → prints the
///    result → `handl_standalone_free_string` → `handl_standalone_destroy`.
pub(crate) fn generate_c_driver(
    ast_json: &str,
    literals_json: &str,
    names_json: &str,
    function_names: &std::collections::HashMap<usize, String>,
    root_idx: usize,
) -> String {
    let mut c = String::with_capacity(4096);

    c.push_str("/* Auto-generated by handl-plugin-engine-eval-cranelift */\n");
    c.push_str("#include <stdio.h>\n");
    c.push_str("#include <stdint.h>\n");
    c.push_str("#include <stddef.h>\n\n");

    // Standalone entry points (from the Rust staticlib).
    c.push_str("/* Standalone entry points (Rust staticlib) */\n");
    c.push_str("extern void* handl_standalone_init(\n");
    c.push_str("    const char* ast_json,\n");
    c.push_str("    const char* literals_json,\n");
    c.push_str("    const char* names_json,\n");
    c.push_str("    const void** fn_ptrs,\n");
    c.push_str("    const uint64_t* fn_indices,\n");
    c.push_str("    size_t n_fns);\n");
    c.push_str("extern const char* handl_standalone_run(void* ctx, uint64_t root_idx);\n");
    c.push_str("extern void handl_standalone_free_string(const char* ptr);\n");
    c.push_str("extern void handl_standalone_destroy(void* ctx);\n\n");

    // Node function externs (from the cranelift-emitted .o).
    c.push_str("/* Node functions (cranelift-emitted .o) */\n");
    let mut indices: Vec<usize> = function_names.keys().copied().collect();
    indices.sort();
    for &idx in &indices {
        let name = &function_names[&idx];
        c.push_str(&format!("extern void* {}(void*);\n", name));
    }
    c.push('\n');

    // Embedded JSON constants.
    c.push_str("/* Embedded constant pools (JSON) */\n");
    c.push_str(&format!(
        "static const char AST_JSON[] = \"{}\";\n",
        escape_c_string(ast_json)
    ));
    c.push_str(&format!(
        "static const char LITERALS_JSON[] = \"{}\";\n",
        escape_c_string(literals_json)
    ));
    c.push_str(&format!(
        "static const char NAMES_JSON[] = \"{}\";\n\n",
        escape_c_string(names_json)
    ));

    // main()
    c.push_str("int main(void) {\n");
    if indices.is_empty() {
        // Empty AST → print null.
        c.push_str("    printf(\"null\\n\");\n");
        c.push_str("    return 0;\n");
        c.push_str("}\n");
        return c;
    }

    // Build the function-pointer and index arrays.
    c.push_str("    const void* fn_ptrs[] = { ");
    for (i, &idx) in indices.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_str(&format!("(const void*){}", function_names[&idx]));
    }
    c.push_str(" };\n");

    c.push_str("    const uint64_t fn_indices[] = { ");
    for (i, &idx) in indices.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        c.push_str(&format!("{}ULL", idx));
    }
    c.push_str(" };\n");

    c.push_str(&format!("    size_t n_fns = {};\n", indices.len()));
    c.push_str(&format!("    uint64_t root_idx = {}ULL;\n\n", root_idx));

    c.push_str("    void* ctx = handl_standalone_init(\n");
    c.push_str("        AST_JSON, LITERALS_JSON, NAMES_JSON,\n");
    c.push_str("        fn_ptrs, fn_indices, n_fns);\n");
    c.push_str("    if (!ctx) { return 1; }\n\n");
    c.push_str("    const char* result = handl_standalone_run(ctx, root_idx);\n");
    c.push_str("    if (result) {\n");
    c.push_str("        printf(\"%s\\n\", result);\n");
    c.push_str("        handl_standalone_free_string(result);\n");
    c.push_str("    } else {\n");
    c.push_str("        fprintf(stderr, \"evaluation failed\\n\");\n");
    c.push_str("    }\n");
    c.push_str("    handl_standalone_destroy(ctx);\n");
    c.push_str("    return result ? 0 : 1;\n");
    c.push_str("}\n");

    c
}

/// Find the path to the compiled static library
/// (`libhandl_plugin_engine_eval_cranelift.a`).
///
/// Uses `cargo metadata` to locate the workspace target directory, then
/// checks the `debug` and `release` profiles. If the static library is not
/// found, runs `cargo build` to produce it.
pub(crate) fn find_staticlib() -> Result<PathBuf, LangError> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| LangError::Engine("CARGO_MANIFEST_DIR not set".to_string()))?;
    let manifest_path = PathBuf::from(&manifest_dir).join("Cargo.toml");

    // Get the target directory from cargo metadata.
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(&manifest_path)
        .output()
        .map_err(|e| LangError::Engine(format!("cargo metadata failed: {}", e)))?;
    if !output.status.success() {
        return Err(LangError::Engine(format!(
            "cargo metadata error: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| LangError::Engine(format!("parse cargo metadata: {}", e)))?;
    let target_dir = metadata
        .get("target_directory")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LangError::Engine("no target_directory in metadata".to_string()))?;

    let lib_name = "libhandl_plugin_engine_eval_cranelift.a";

    // Check debug first (tests run in debug), then release.
    for profile in &["debug", "release"] {
        let path = PathBuf::from(target_dir).join(profile).join(lib_name);
        if path.exists() {
            return Ok(path);
        }
    }

    // Not found — build it.
    let build_status = Command::new("cargo")
        .args(["build", "-p", "handl-plugin-engine-eval-cranelift"])
        .current_dir(&manifest_dir)
        .status()
        .map_err(|e| LangError::Engine(format!("cargo build failed: {}", e)))?;
    if !build_status.success() {
        return Err(LangError::Engine(
            "cargo build for staticlib failed".to_string(),
        ));
    }

    // Try again.
    for profile in &["debug", "release"] {
        let path = PathBuf::from(target_dir).join(profile).join(lib_name);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(LangError::Engine(format!(
        "static library {} not found in {}/debug or {}/release",
        lib_name, target_dir, target_dir
    )))
}

/// Link the C driver + cranelift-emitted object file + Rust staticlib into a
/// standalone executable using `cc`.
///
/// The command is:
/// ```text
/// cc <main.c> <obj.o> <staticlib.a> -o <output> -lpthread -ldl -lm
/// ```
///
/// The system libraries (`-lpthread -ldl -lm`) are needed by the Rust
/// standard library embedded in the staticlib.
pub(crate) fn link_executable(
    main_c: &Path,
    obj: &Path,
    staticlib: &Path,
    output: &Path,
) -> Result<(), LangError> {
    let result = Command::new("cc")
        .arg(main_c)
        .arg(obj)
        .arg(staticlib)
        .arg("-o")
        .arg(output)
        .arg("-lpthread")
        .arg("-ldl")
        .arg("-lm")
        .output()
        .map_err(|e| LangError::Engine(format!("cc invocation failed: {}", e)))?;

    if !result.status.success() {
        return Err(LangError::Engine(format!(
            "cc linking failed:\n{}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }

    Ok(())
}

/// Compile a `Language` + `Ast` to a standalone executable.
///
/// This is the full ADR-0044 Phase 8 `compile(handlLanguage, native)` step,
/// used by [`crate::CraneliftEngine::compile_to_executable`]. It:
///
/// 1. Compiles the AST to a relocatable `.o` file (via
///    [`compile_all_nodes_object`]).
/// 2. Serializes the `Ast`, literal pool, and name pool to JSON.
/// 3. Generates a C `main` driver that embeds the JSON and declares `extern`
///    prototypes for each `node_N` function.
/// 4. Links the `.o` + the C driver + this crate's static library (which
///    exports the `helper_*` and `handl_standalone_*` symbols) with `cc`.
///
/// The resulting executable, when run, prints the evaluation result (as
/// JSON) to stdout — matching the host-built handl.
///
/// # Errors
///
/// Returns [`LangError`] on compilation, serialization, driver generation,
/// staticlib discovery, or linking failure.
///
/// [`compile_all_nodes_object`]: crate::compile_all_nodes_object
pub(crate) fn compile_to_executable(
    lang: &handl_language::Language,
    ast: &handl_language::Ast,
    output_path: &Path,
) -> Result<(), LangError> {
    use crate::compile_all_nodes_object;

    // Handle the empty-AST case: produce a trivial executable that prints
    // "null".
    if ast.is_empty() {
        let main_c_path = output_path.with_extension("c");
        let driver =
            "/* empty AST */\n#include <stdio.h>\nint main(void){printf(\"null\\n\");return 0;}\n";
        std::fs::write(&main_c_path, driver)
            .map_err(|e| LangError::Engine(format!("write main.c: {}", e)))?;
        let result = Command::new("cc")
            .arg(&main_c_path)
            .arg("-o")
            .arg(output_path)
            .output()
            .map_err(|e| LangError::Engine(format!("cc invocation failed: {}", e)))?;
        let _ = std::fs::remove_file(&main_c_path);
        if !result.status.success() {
            return Err(LangError::Engine(format!(
                "cc linking (empty AST) failed:\n{}",
                String::from_utf8_lossy(&result.stderr)
            )));
        }
        return Ok(());
    }

    // 1. Compile to object file.
    let obj_path = output_path.with_extension("o");
    let obj_result = compile_all_nodes_object(lang, ast, &obj_path)?;

    // 2. Serialize constant pools + AST to JSON.
    let ast_json = serde_json::to_string(ast)
        .map_err(|e| LangError::Engine(format!("serialize Ast: {}", e)))?;
    let literals_json = serde_json::to_string(&obj_result.literals)
        .map_err(|e| LangError::Engine(format!("serialize literals: {}", e)))?;
    let names_json = serde_json::to_string(&obj_result.names)
        .map_err(|e| LangError::Engine(format!("serialize names: {}", e)))?;

    // 3. Generate the C driver.
    let root_idx = ast.len() - 1;
    let main_c_path = output_path.with_extension("c");
    let driver = generate_c_driver(
        &ast_json,
        &literals_json,
        &names_json,
        &obj_result.function_names,
        root_idx,
    );
    std::fs::write(&main_c_path, &driver)
        .map_err(|e| LangError::Engine(format!("write main.c: {}", e)))?;

    // 4. Find the staticlib.
    let staticlib = find_staticlib()?;

    // 5. Link.
    link_executable(&main_c_path, &obj_path, &staticlib, output_path)?;

    // 6. Clean up temp files (best-effort).
    let _ = std::fs::remove_file(&obj_path);
    let _ = std::fs::remove_file(&main_c_path);

    Ok(())
}
