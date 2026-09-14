//! Build script for the LLVM backend.
//!
//! Probes for `llvm-config` when the `inkwell` feature is enabled and emits
//! helpful warnings / graceful failures when LLVM is missing.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_FEATURE_INKWELL").is_err() {
        println!("cargo:warning=inkwell feature disabled; LLVM backend is a placeholder.");
        return;
    }

    match Command::new("llvm-config").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("cargo:warning=found llvm-config version {version}");

            if let Ok(libdir) = Command::new("llvm-config").arg("--libdir").output() {
                let dir = String::from_utf8_lossy(&libdir.stdout).trim().to_string();
                if !dir.is_empty() {
                    println!("cargo:rustc-link-search=native={dir}");
                }
            }

            if let Ok(libs) = Command::new("llvm-config").arg("--libnames").output() {
                let libs = String::from_utf8_lossy(&libs.stdout);
                for lib in libs.split_whitespace() {
                    let base = lib
                        .trim_start_matches("lib")
                        .trim_end_matches(".so")
                        .trim_end_matches(".a")
                        .trim_end_matches(".dylib")
                        .trim_end_matches(".dll");
                    if !base.is_empty() {
                        println!("cargo:rustc-link-lib=dylib={base}");
                    }
                }
            }
        }
        _ => {
            let msg = "LLVM/inkwell requested but llvm-config not found. Install LLVM or build with `cargo build -p handl-plugin-compile-llvm` (without the `inkwell` feature).";
            println!("cargo:warning={msg}");
            eprintln!("{msg}");
            std::process::exit(1);
        }
    }
}
