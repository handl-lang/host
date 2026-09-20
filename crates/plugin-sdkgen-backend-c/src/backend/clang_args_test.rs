use super::clang_args::clang_args;
use std::path::Path;

fn arg_strings(header: &Path, target: &str) -> Vec<String> {
    let (raw, ptrs) = clang_args(header, target, &[]);
    // ptrs must alias raw CStrings — same count, same contents.
    assert_eq!(raw.len(), ptrs.len());
    raw.iter()
        .map(|c| c.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn builds_language_include_and_target_args() {
    let args = arg_strings(Path::new("/usr/include/sys/socket.h"), "x86_64-linux-gnu");
    assert_eq!(&args[0..2], &["-x", "c"]);
    // The header's directory goes on the `-iquote` path (quoted includes
    // only), not `-I` — a system header dir would shadow `<...>` headers
    // sharing a basename (ICU's uchar.h vs the C11 one).
    let q = args.iter().position(|a| a == "-iquote").unwrap();
    assert_eq!(args[q + 1], "/usr/include/sys");
    assert!(args.contains(&"-I/usr/include".to_string()));
    let t = args.iter().position(|a| a == "-target").unwrap();
    assert_eq!(args[t + 1], "x86_64-linux-gnu");
}

#[test]
fn relative_header_gets_dot_include() {
    let args = arg_strings(Path::new("simple.h"), "aarch64-linux-gnu");
    let q = args.iter().position(|a| a == "-iquote").unwrap();
    assert_eq!(args[q + 1], ".");
}

#[test]
fn defines_become_dash_d_args() {
    let (raw, _) = clang_args(
        Path::new("x.h"),
        "x86_64-linux-gnu",
        &["_GNU_SOURCE".into(), "FOO=1".into()],
    );
    let args: Vec<String> = raw
        .iter()
        .map(|c| c.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"-D_GNU_SOURCE".to_string()));
    assert!(args.contains(&"-DFOO=1".to_string()));
}
