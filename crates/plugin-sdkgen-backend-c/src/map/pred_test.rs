use crate::map::pred::{is_char, is_integer_kind, is_size_t, is_void};
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
fn integer_kind_pred() {
    let sn = parse_snippet(SRC);
    assert!(is_integer_kind(sn.fn_arg_type("f", 0))); // int
    assert!(is_integer_kind(sn.fn_arg_type("f", 1))); // char
    assert!(is_integer_kind(sn.fn_arg_type("f", 2))); // size_t typedef
    assert!(is_integer_kind(sn.fn_arg_type("f", 3))); // socklen_t typedef
    assert!(is_integer_kind(sn.fn_arg_type("f", 6))); // enum
    assert!(!is_integer_kind(sn.fn_arg_type("f", 4))); // float
    assert!(!is_integer_kind(sn.fn_arg_type("f", 5))); // void*
}
