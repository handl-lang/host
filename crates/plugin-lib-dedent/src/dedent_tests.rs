use super::dedent::dedent;

#[test]
fn dedent_simple() {
    let input = "    Hello,\n    World!";
    assert_eq!(dedent(input), "Hello,\nWorld!");
}

#[test]
fn dedent_preserves_relative_indent() {
    let input = "    Hello,\n      World!";
    assert_eq!(dedent(input), "Hello,\n  World!");
}

#[test]
fn dedent_no_indent() {
    assert_eq!(dedent("Hello,\nWorld!"), "Hello,\nWorld!");
}

#[test]
fn dedent_empty() {
    assert_eq!(dedent(""), "");
}

#[test]
fn dedent_blank_lines_ignored() {
    let input = "    Hello,\n\n    World!";
    assert_eq!(dedent(input), "Hello,\n\nWorld!");
}

#[test]
fn dedent_trims_leading_trailing_newlines() {
    let input = "\n\n  Hello,\n  World!\n\n";
    assert_eq!(dedent(input), "Hello,\nWorld!");
}

#[test]
fn dedent_tabs() {
    let input = "\tHello,\n\tWorld!";
    assert_eq!(dedent(input), "Hello,\nWorld!");
}

#[test]
fn dedent_mixed_indent() {
    let input = "  \tHello,\n  \tWorld!";
    assert_eq!(dedent(input), "Hello,\nWorld!");
}

#[test]
fn dedent_only_whitespace_lines() {
    assert_eq!(dedent("   \n  \n   "), "   \n  \n   ");
}

#[test]
fn dedent_single_line() {
    assert_eq!(dedent("    Hello!"), "Hello!");
}

#[test]
fn dedent_trailing_whitespace_line() {
    let input = "  Hello,\n  World!\n  ";
    assert_eq!(dedent(input), "Hello,\nWorld!\n");
}

#[test]
fn dedent_crlf_input() {
    // .lines() splits on \r\n and .join("\n") normalizes to LF
    let input = "  Hello,\r\n  World!";
    assert_eq!(dedent(input), "Hello,\nWorld!");
}

#[test]
fn dedent_uneven_indent() {
    let input = "    Hello,\n  World!";
    assert_eq!(dedent(input), "  Hello,\nWorld!");
}

#[test]
fn dedent_single_trailing_newline() {
    let input = "  Hello,\n  World!\n";
    assert_eq!(dedent(input), "Hello,\nWorld!");
}
