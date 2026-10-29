//! Build-script helper: vendor a `lib-*` package into `OUT_DIR` (ADR-0160 P3).
//!
//! A consumer that must stay **hermetic** (every `kit-*`, and
//! `plugin-lib-stdlib`) cannot load its `.hnl` at runtime. Instead its `build.rs`
//! calls [`vendor`] once, which copies the package (`pkg.hnl` + `src/**/*.hnl`)
//! into `OUT_DIR/<package>` and prints `cargo:rerun-if-changed`. The crate then
//! embeds the vendored file with
//! `include_str!(concat!(env!("OUT_DIR"), "/lib-x/src/index.hnl"))`.
//!
//! The *lib* repo ships no `Cargo.toml` — that is the point of ADR-0160; the
//! build glue lives in the consumer.

use std::path::{Path, PathBuf};

/// Copy `pkg_dir`'s package files into `OUT_DIR/<package name>` and return the
/// destination directory.
///
/// # Errors
///
/// Returns an error when the directory is not a package (no `pkg.hnl`) or a file
/// cannot be read/written.
pub fn vendor(pkg_dir: impl AsRef<Path>) -> Result<PathBuf, String> {
    let out_root = std::env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "vendor: OUT_DIR is not set (call from build.rs)".to_string())?;
    vendor_into(pkg_dir.as_ref(), &out_root)
}

/// [`vendor`] against an explicit output root — the test seam.
pub fn vendor_into(pkg_dir: &Path, out_root: &Path) -> Result<PathBuf, String> {
    let manifest = pkg_dir.join("pkg.hnl");
    if !pkg_dir.is_dir() {
        return Err(format!("vendor: {} is not a directory", pkg_dir.display()));
    }
    let name = pkg_dir
        .file_name()
        .ok_or_else(|| format!("vendor: {} has no file name", pkg_dir.display()))?
        .to_string_lossy()
        .to_string();
    let dest = out_root.join(&name);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(&dest).map_err(|e| format!("vendor: {}: {e}", dest.display()))?;
    // The manifest is optional: 21 of the 113 repos predate it. Their `.hnl`
    // sources still vendor, which is all a consumer of a legacy row needs.
    if manifest.is_file() {
        std::fs::copy(&manifest, dest.join("pkg.hnl"))
            .map_err(|e| format!("vendor: {}: {e}", manifest.display()))?;
        println!("cargo:rerun-if-changed={}", manifest.display());
    }
    copy_hnl(&pkg_dir.join("src"), &dest.join("src"))?;
    normalize_single_module(&dest);
    Ok(dest)
}

/// Give a single-module package the canonical `src/index.hnl` entry.
///
/// Most of the 113 repos predate ADR-0160's `src/index.hnl` rule and name the
/// file after the module (`src/type-handl.hnl`). Vendoring normalises that, so a
/// consumer always writes `include_str!(concat!(env!("OUT_DIR"),
/// "/lib-x/src/index.hnl"))` regardless of the repo's historical layout.
fn normalize_single_module(dest: &Path) {
    let Ok(manifest) = std::fs::read_to_string(dest.join("pkg.hnl")) else {
        return;
    };
    let modules = handl_platform::pkg_modules(&manifest);
    if modules.len() != 1 {
        return;
    }
    let index = dest.join("src/index.hnl");
    if index.is_file() {
        return;
    }
    let legacy = dest.join(format!("src/{}.hnl", modules[0]));
    if legacy.is_file() {
        let _ = std::fs::copy(&legacy, &index);
    }
}

/// Recursively copy every `.hnl` file from `src` into `dst`.
fn copy_hnl(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("vendor: {}: {e}", dst.display()))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("vendor: {}: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("vendor: {}: {e}", src.display()))?;
        let path = entry.path();
        if path.is_dir() {
            copy_hnl(&path, &dst.join(entry.file_name()))?;
        } else if path.extension().is_some_and(|e| e == "hnl") {
            let target = dst.join(entry.file_name());
            std::fs::copy(&path, &target).map_err(|e| format!("vendor: {}: {e}", path.display()))?;
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

/// The file a module resolves to inside a vendored package
/// (`src/index.hnl` for one module, `src/<name>/index.hnl` for many).
///
/// # Errors
///
/// Returns an error when `pkg.hnl` cannot be read or does not declare `module`.
pub fn module_file(vendored: &Path, module: &str) -> Result<PathBuf, String> {
    let manifest = std::fs::read_to_string(vendored.join("pkg.hnl"))
        .map_err(|e| format!("vendor: {}: {e}", vendored.display()))?;
    let modules = handl_platform::pkg_modules(&manifest);
    let entry = handl_platform::pkg_entry(&manifest);
    for rel in handl_platform::pkg_module_candidates(&modules, entry.as_deref(), module) {
        let path = vendored.join(&rel);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(format!(
        "vendor: {} declares no module `{module}`",
        vendored.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pkg-vendor-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn package(root: &Path, name: &str, manifest: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("pkg.hnl"), manifest).unwrap();
        for (rel, src) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, src).unwrap();
        }
        dir
    }

    #[test]
    fn vendors_manifest_and_hnl_only() {
        let root = temp("one");
        let pkg = package(
            &root,
            "lib-x",
            "{ modules: [\"x\"] }\n",
            &[("src/index.hnl", "{ a: c ~> true }\n"), ("src/notes.txt", "skip")],
        );
        let out = root.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let dest = vendor_into(&pkg, &out).unwrap();
        assert!(dest.join("pkg.hnl").is_file());
        assert!(dest.join("src/index.hnl").is_file());
        assert!(!dest.join("src/notes.txt").exists(), "non-.hnl is not vendored");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn module_file_follows_the_manifest() {
        let root = temp("two");
        let pkg = package(
            &root,
            "lib-multi",
            "{ modules: [\"a\", \"b\"] }\n",
            &[("src/a/index.hnl", "{}\n"), ("src/b/index.hnl", "{}\n")],
        );
        let out = root.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let dest = vendor_into(&pkg, &out).unwrap();
        assert!(module_file(&dest, "b").unwrap().ends_with("src/b/index.hnl"));
        assert!(module_file(&dest, "zzz").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_directory_is_an_error_but_a_manifest_is_optional() {
        let root = temp("three");
        let out = root.join("out");
        std::fs::create_dir_all(&out).unwrap();
        assert!(vendor_into(&root.join("nope"), &out).is_err());
        // 21 of the 113 repos predate `pkg.hnl`; their sources still vendor.
        let legacy = root.join("lib-legacy");
        std::fs::create_dir_all(legacy.join("src")).unwrap();
        std::fs::write(legacy.join("src/index.hnl"), "{ a: c ~> true }
").unwrap();
        let dest = vendor_into(&legacy, &out).unwrap();
        assert!(dest.join("src/index.hnl").is_file());
        assert!(!dest.join("pkg.hnl").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
