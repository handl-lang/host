//! Build script for plugin-engine-eval-cranelift.
//!
//! On Linux, export dynamic symbols so that FFI tests can find `#[no_mangle]
//! extern "C"` test functions via `Library::this()`.

fn main() {
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic");
    }
}
