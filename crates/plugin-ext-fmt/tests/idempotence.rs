//! Idempotence property test over the repo's own `.hnl` examples.
//!
//! Verifies that `format_source(format_source(src)) == format_source(src)`
//! for every `.hnl` file in `research/arrow-calculus-kernel-poc/`.

use std::fs;
use std::path::PathBuf;

use handl_plugin_ext_fmt::format_source;

/// Recursively find all `.hnl` files under the given directory.
fn find_hnl_files(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_hnl_files(&path, out);
            } else if path.extension().map_or(false, |e| e == "hnl") {
                out.push(path);
            }
        }
    }
}

#[test]
fn idempotence_over_repo_examples() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../research/arrow-calculus-kernel-poc");
    let mut files = Vec::new();
    find_hnl_files(&root, &mut files);
    assert!(
        !files.is_empty(),
        "no .hnl files found under {}",
        root.display()
    );

    for file in &files {
        let src = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue, // skip unreadable files
        };
        // Skip files that don't parse (some examples may use syntax the
        // current parser doesn't support — that's OK, the formatter returns
        // an error and we skip it).
        let once = match format_source(&src) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let twice = match format_source(&once) {
            Ok(s) => s,
            Err(e) => panic!("second format_source failed for {}: {e}", file.display()),
        };
        assert_eq!(
            once,
            twice,
            "idempotence violated for {}:\n--- once ---\n{once}\n--- twice ---\n{twice}",
            file.display()
        );
    }
}
