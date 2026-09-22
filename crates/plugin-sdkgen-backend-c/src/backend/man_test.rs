use crate::backend::man::{extract_groff_name, extract_html_name, page_file_doc_in};

const GROFF: &str = ".TH SOCKET 2 2024-05-02 \"Linux man-pages\"\n\
    .SH NAME\n\
    socket \\- create an endpoint for communication\n\
    .SH LIBRARY\n\
    Standard C library\n";

#[test]
fn groff_name_extracts_description() {
    assert_eq!(
        extract_groff_name(GROFF, "socket").as_deref(),
        Some("create an endpoint for communication")
    );
    // A symbol not named in the NAME line gets no doc — stops
    // subsection pages from emitting mismatched text.
    assert_eq!(extract_groff_name(GROFF, "bind"), None);
}

#[test]
fn groff_name_handles_multi_name_lines() {
    let text = ".SH NAME\nerrno, EAGAIN, EINTR \\- error number macros\n.SH X\n";
    assert_eq!(
        extract_groff_name(text, "EAGAIN").as_deref(),
        Some("error number macros")
    );
}

#[test]
fn html_name_extracts_man7_layout() {
    let html = "<body><h2><a id=\"NAME\" href=\"#NAME\"></a>NAME &nbsp;</h2><pre>\n\
        socket - create an endpoint for communication\n</pre>\n\
        <h2><a id=\"LIBRARY\"></a>LIBRARY</h2></body>";
    assert_eq!(
        extract_html_name(html, "socket").as_deref(),
        Some("create an endpoint for communication")
    );
    assert_eq!(extract_html_name(html, "bind"), None);
}

#[test]
fn page_file_doc_reads_gz_and_follows_so() {
    let dir = std::env::temp_dir().join(format!("hnl_man_test_{}", std::process::id()));
    let man3 = dir.join("man3");
    std::fs::create_dir_all(&man3).unwrap();
    let write_gz = |name: &str, text: &str| {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        use std::io::Write;
        e.write_all(text.as_bytes()).unwrap();
        std::fs::write(man3.join(name), e.finish().unwrap()).unwrap();
    };
    write_gz("alias_fn.3.gz", ".so man3/real_fn.3\n");
    write_gz(
        "real_fn.3.gz",
        ".SH NAME\nreal_fn, alias_fn \\- do the real thing\n",
    );
    let root = dir.to_str().unwrap().to_string();
    assert_eq!(
        page_file_doc_in(&[root.as_str()], "alias_fn", "3").as_deref(),
        Some("do the real thing")
    );
    std::fs::remove_dir_all(&dir).ok();
}
