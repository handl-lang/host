use crate::map::arg::map_arg_with_peek;
use crate::testutil::parse_snippet;

const PRELUDE: &str =
    "typedef unsigned long size_t;\ntypedef unsigned int socklen_t;\nstruct foo { int x; };\n";

/// Map arg `idx` of `decl` (PRELUDE prepended), peeking at the next arg.
fn map(decl: &str, idx: u32) -> Result<(String, String, bool), String> {
    let sn = parse_snippet(&format!("{PRELUDE}{decl}\n"));
    let ty = sn.fn_arg_type("f", idx);
    let next = {
        let f = sn.fn_cursor("f");
        let n = unsafe { clang_sys::clang_Cursor_getNumArguments(f) };
        if (idx as i32) + 1 < n {
            Some(sn.fn_arg_cursor("f", idx + 1))
        } else {
            None
        }
    };
    let variadic = unsafe {
        clang_sys::clang_isFunctionTypeVariadic(clang_sys::clang_getCursorType(sn.fn_cursor("f")))
    } != 0;
    map_arg_with_peek(ty, next, sn.pointer_width(), variadic)
        .map(|i| (i.port, i.descriptor, i.is_size_pair))
}

#[test]
fn plain_scalars() {
    assert_eq!(
        map("int f(int x);", 0).unwrap(),
        ("Int".into(), "i32".into(), false)
    );
    assert_eq!(
        map("int f(size_t n);", 0).unwrap(),
        ("Size".into(), "i64".into(), false)
    );
    // float has no f32 descriptor — widening f64 bits would be wrong.
    assert!(
        map("int f(float x);", 0)
            .unwrap_err()
            .contains("unsupported argument type")
    );
}

#[test]
fn const_char_star_is_cstr() {
    assert_eq!(
        map("int f(const char *s);", 0).unwrap(),
        ("String".into(), "cstr".into(), false)
    );
    assert_eq!(
        map("int f(char *s);", 0).unwrap(),
        ("String".into(), "cstr".into(), false)
    );
}

#[test]
fn ptr_len_pairs_collapse() {
    // const void* + size_t -> in_buf
    let (port, desc, pair) = map("int f(const void *b, size_t n);", 0).unwrap();
    assert_eq!(
        (port.as_str(), desc.as_str(), pair),
        ("Bytes", "in_buf", true)
    );
    // const char* + int -> cbytes
    let (port, desc, pair) = map("int f(const char *b, int n);", 0).unwrap();
    assert_eq!(
        (port.as_str(), desc.as_str(), pair),
        ("Bytes", "cbytes", true)
    );
    // const struct* + socklen_t (integer typedef, not size_t) -> in_buf
    let (port, desc, pair) = map("int f(const struct foo *a, socklen_t n);", 0).unwrap();
    assert_eq!(
        (port.as_str(), desc.as_str(), pair),
        ("Bytes", "in_buf", true)
    );
    // A length-named int counts too — but a non-length int (`fd`,
    // `flags`) must NOT collapse: binding it as a buffer length would
    // silently misgenerate the call.
    let (_, desc, pair) = map("int f(const void *b, int len);", 0).unwrap();
    assert_eq!((desc.as_str(), pair), ("in_buf", true));
    assert!(map("int f(const void *b, int fd);", 0).is_err());
}

#[test]
fn const_void_star_without_len_is_unsupported() {
    assert!(
        map("int f(const void *b);", 0)
            .unwrap_err()
            .contains("unsupported pointer type")
    );
}

#[test]
fn mutable_char_star_with_len_is_rejected() {
    assert!(
        map("int f(char *b, int n);", 0)
            .unwrap_err()
            .contains("unsupported mutable buffer")
    );
}

#[test]
fn void_star_is_handle() {
    assert_eq!(
        map("int f(void *p);", 0).unwrap(),
        ("Handle".into(), "ptr".into(), false)
    );
}

#[test]
fn nonconst_struct_star_is_out_struct() {
    let sn = parse_snippet(&format!("{PRELUDE}int f(struct foo *p);\n"));
    let ty = sn.fn_arg_type("f", 0);
    let info = map_arg_with_peek(ty, None, sn.pointer_width(), false).unwrap();
    assert!(info.is_out_struct);
    assert_eq!(info.record_name.as_deref(), Some("foo"));
    assert!(!info.consumes);
}

#[test]
fn variadic_only_size_t_counts_as_len() {
    // non-variadic: const char* + int collapses
    let (_, d, p) = map("int f(const char *s, int n);", 0).unwrap();
    assert_eq!((d.as_str(), p), ("cbytes", true));
    // variadic: same shape stays cstr (open(path, flags, ...) guard)
    let (port, d, p) = map("int f(const char *s, int n, ...);", 0).unwrap();
    assert_eq!((port.as_str(), d.as_str(), p), ("String", "cstr", false));
    // but variadic with a real size_t still collapses
    let (_, d, p) = map("int f(const void *b, size_t n, ...);", 0).unwrap();
    assert_eq!((d.as_str(), p), ("in_buf", true));
}

#[test]
fn double_pointers_and_int_stars_unsupported() {
    assert!(
        map("int f(char **p);", 0)
            .unwrap_err()
            .contains("unsupported pointer type")
    );
    assert!(
        map("int f(int *p);", 0)
            .unwrap_err()
            .contains("unsupported pointer type")
    );
}
