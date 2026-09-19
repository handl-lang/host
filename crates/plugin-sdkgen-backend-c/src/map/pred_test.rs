use crate::map::pred::{is_char, is_len_like, is_size_t, is_void};
use crate::testutil::parse_snippet;

const SRC: &str = "\
typedef unsigned long size_t;
typedef unsigned int socklen_t;
enum color { RED };
int f(int i, char c, size_t n, socklen_t l, float x, void *p, enum color e);
void g(void);
";

#[test]
fn void_pred() {
    let sn = parse_snippet(SRC);
    assert!(is_void(sn.fn_ret_type("g")));
    assert!(!is_void(sn.fn_ret_type("f")));
}

#[test]
fn char_pred() {
    let sn = parse_snippet(SRC);
    assert!(is_char(sn.fn_arg_type("f", 1)));
    assert!(!is_char(sn.fn_arg_type("f", 0)));
}

#[test]
fn size_t_pred() {
    let sn = parse_snippet(SRC);
    assert!(is_size_t(sn.fn_arg_type("f", 2)));
    // An integer typedef that is not spelled `size_t` is not size_t.
    assert!(!is_size_t(sn.fn_arg_type("f", 3)));
}

#[test]
fn len_like_pred() {
    let sn = parse_snippet(SRC);
    // size_t / socklen_t spellings count by type.
    assert!(is_len_like(sn.fn_arg_type("f", 2), "anything"));
    assert!(is_len_like(sn.fn_arg_type("f", 3), "anything"));
    // Bare ints count only when the name reads as a length — `flags`
    // must not collapse a (ptr, flags) pair.
    assert!(is_len_like(sn.fn_arg_type("f", 0), "len"));
    assert!(is_len_like(sn.fn_arg_type("f", 0), "buflen"));
    assert!(is_len_like(sn.fn_arg_type("f", 0), "count"));
    assert!(is_len_like(sn.fn_arg_type("f", 0), "__n"));
    assert!(!is_len_like(sn.fn_arg_type("f", 0), "flags"));
    assert!(!is_len_like(sn.fn_arg_type("f", 0), "i"));
    // Non-integer types never count, whatever the name.
    assert!(!is_len_like(sn.fn_arg_type("f", 4), "len"));
}
