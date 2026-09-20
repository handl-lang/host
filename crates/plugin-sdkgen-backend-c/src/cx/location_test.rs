use crate::cx::{from_main_file, is_system_header};
use crate::testutil::parse_snippet;

#[test]
fn snippet_decls_are_main_file_not_system() {
    let sn = parse_snippet("int add(int a, int b);\n");
    let c = sn.fn_cursor("add");
    assert!(from_main_file(c));
    assert!(!is_system_header(c));
}

#[test]
fn macro_renamed_decl_resolves_expansion_to_main_file() {
    // ICU's urename.h pattern: the declaration's name is produced by a
    // renaming macro, so its spelling location is <scratch space>. The
    // expansion location must still classify as the main file.
    let sn = parse_snippet("#define greet greet_1\nint greet(int a);\n");
    let c = sn.fn_cursor("greet_1");
    assert!(from_main_file(c));
    assert!(!is_system_header(c));
}
