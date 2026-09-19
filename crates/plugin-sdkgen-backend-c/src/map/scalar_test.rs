use crate::map::scalar::{map_arg_type, map_field_type, map_return_type};
use crate::testutil::parse_snippet;

const PRELUDE: &str = "typedef unsigned long size_t;\n";

fn arg(decl: &str) -> Result<(String, String), String> {
    let sn = parse_snippet(&format!("{PRELUDE}{decl}\n"));
    map_arg_type(sn.fn_arg_type("f", 0), sn.pointer_width())
}

fn ret(decl: &str) -> Result<(String, String), String> {
    let sn = parse_snippet(&format!("{PRELUDE}{decl}\n"));
    map_return_type(sn.fn_ret_type("f"), sn.pointer_width())
}

#[test]
fn arg_type_table() {
    let cases = [
        ("int f(int x);", "Int", "i32"),
        ("int f(unsigned int x);", "Nat", "i32"),
        ("int f(long x);", "Int", "i64"),
        ("int f(long long x);", "Int", "i64"),
        ("int f(unsigned long x);", "Nat", "i64"),
        ("int f(unsigned long long x);", "Nat", "i64"),
        ("int f(double x);", "Float", "f64"),
        // sub-32-bit ints widen to i32 (full-width arg slots)
        ("int f(short x);", "Int", "i32"),
        ("int f(unsigned short x);", "Nat", "i32"),
        ("int f(signed char x);", "Int", "i32"),
        ("int f(unsigned char x);", "Nat", "i32"),
        ("int f(_Bool x);", "Int", "i32"),
        ("int f(size_t x);", "Nat", "i64"),
        // typedefs resolve through their canonical type
        ("int f(size_t_ x);", "Nat", "i64"),
    ];
    // size_t_ needs its own typedef to exercise canonical resolution
    for (decl, port, desc) in cases {
        let src = if decl.contains("size_t_") {
            "typedef unsigned long size_t_;\n".to_string() + decl
        } else {
            PRELUDE.to_string() + decl
        };
        let sn = parse_snippet(&src);
        let got = map_arg_type(sn.fn_arg_type("f", 0), sn.pointer_width())
            .unwrap_or_else(|e| panic!("{decl}: {e}"));
        assert_eq!(got, (port.to_string(), desc.to_string()), "{decl}");
    }
}

#[test]
fn arg_type_rejects_float_and_void() {
    assert!(arg("int f(float x);").unwrap_err().contains("unsupported"));
    assert!(arg("int f(void *p);").unwrap_err().contains("unsupported")); // pointer, not scalar
}

#[test]
fn return_type_table() {
    assert_eq!(ret("void f(void);").unwrap(), ("()".into(), "void".into()));
    assert_eq!(ret("int f(void);").unwrap(), ("Int".into(), "i32".into()));
    assert_eq!(
        ret("double f(void);").unwrap(),
        ("Float".into(), "f64".into())
    );
    assert_eq!(
        ret("void *f(void);").unwrap(),
        ("Handle".into(), "handle".into())
    );
    assert_eq!(
        ret("const char *f(void);").unwrap(),
        ("String".into(), "cstr".into())
    );
    assert_eq!(
        ret("char *f(void);").unwrap(),
        ("String".into(), "cstr".into())
    );
    // int* / struct* returns stay unsupported
    assert!(ret("int *f(void);").is_err());
    assert!(ret("struct s *f(void);").is_err());
}

#[test]
fn field_type_table() {
    let sn = parse_snippet(
        "typedef unsigned long size_t;\n\
         struct s { char a; unsigned char b; short c; unsigned short d; \
                    int e; unsigned int f; long g; unsigned long h; \
                    long long i; unsigned long long j; double k; size_t l; };\n",
    );
    let rec = sn.record_cursor("s");
    let expected = [
        ("a", "Int", "i8"),
        ("b", "Nat", "u8"),
        ("c", "Int", "i16"),
        ("d", "Nat", "u16"),
        ("e", "Int", "i32"),
        ("f", "Nat", "u32"),
        ("g", "Int", "i64"),
        ("h", "Nat", "u64"),
        ("i", "Int", "i64"),
        ("j", "Nat", "u64"),
        ("k", "Float", "f64"),
        ("l", "Nat", "usize"),
    ];
    let fields = crate::cx::children_of_kind(rec, clang_sys::CXCursor_FieldDecl);
    for (field, (name, port, desc)) in fields.iter().zip(expected.iter()) {
        let ty = unsafe { clang_sys::clang_getCursorType(*field) };
        let size = unsafe { clang_sys::clang_Type_getSizeOf(ty) };
        let got = map_field_type(ty, size as usize).unwrap();
        assert_eq!(got, (port.to_string(), desc.to_string()), "field {name}");
    }
}
