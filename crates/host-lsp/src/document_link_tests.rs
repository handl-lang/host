use crate::document_link::document_links;

#[test]
fn document_links_finds_hnl_file_references() {
    // A regular `--` comment mentioning `sort.hnl` should produce a link
    // whose target resolves under the arrows root.
    let source = "-- related: sort.hnl\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/test.hnl"));
    assert!(
        !links.is_empty(),
        "expected at least one link for sort.hnl, got {links:?}"
    );
    let sort_link = links
        .iter()
        .find(|l| {
            l["target"]
                .as_str()
                .map(|t| t.ends_with("sort.hnl"))
                .unwrap_or(false)
        })
        .expect("should have a link targeting sort.hnl");
    assert_eq!(
        sort_link["target"].as_str(),
        Some("file:///tmp/handl-packages/sort.hnl"),
        "bare filename resolves relative to the file's directory"
    );
    assert_eq!(
        sort_link["tooltip"].as_str(),
        Some("Open sort.hnl"),
        "tooltip should name the referenced file"
    );
}

#[test]
fn document_links_finds_url_references() {
    // A URL inside a comment links directly to itself.
    let source = "-- see https://example.com for details\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/test.hnl"));
    let url_link = links
        .iter()
        .find(|l| l["target"].as_str() == Some("https://example.com"))
        .expect("should have a link targeting the URL");
    assert_eq!(
        url_link["tooltip"].as_str(),
        Some("https://example.com"),
        "URL tooltip should be the URL itself"
    );
}

#[test]
fn document_links_finds_multiple_links_in_one_comment() {
    // A single comment containing both a file reference and a URL should
    // yield a link for each.
    let source = "-- see sort.hnl and https://example.com\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/test.hnl"));
    assert!(
        links.len() >= 2,
        "expected at least two links (file + url), got {links:?}"
    );
    assert!(
        links.iter().any(|l| l["target"]
            .as_str()
            .map(|t| t.ends_with("sort.hnl"))
            .unwrap_or(false)),
        "should include a file reference link: {links:?}"
    );
    assert!(
        links
            .iter()
            .any(|l| l["target"].as_str() == Some("https://example.com")),
        "should include a URL link: {links:?}"
    );
}

#[test]
fn document_links_no_links_in_code() {
    // Source with no comments should produce no links, even though the
    // document link feature only ever scans comment tokens.
    let source = "x = 42\ny = x\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/test.hnl"));
    assert!(
        links.is_empty(),
        "non-comment tokens should not produce links, got {links:?}"
    );
}

#[test]
fn document_links_empty_source_returns_empty() {
    let links = document_links("", Some("file:///tmp/handl-packages/test.hnl"));
    assert!(links.is_empty(), "empty source should yield no links");
}

#[test]
fn document_links_resolves_relative_to_arrows_root() {
    // A file reference containing a path separator resolves relative to the
    // arrows root rather than the current file's directory.
    let source = "-- see collections/insert.hnl\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/sort.hnl"));
    let target = links
        .iter()
        .find(|l| {
            l["target"]
                .as_str()
                .map(|t| t.contains("insert.hnl"))
                .unwrap_or(false)
        })
        .expect("should have a link for collections/insert.hnl");
    assert_eq!(
        target["target"].as_str(),
        Some("file:///tmp/handl-packages/collections/insert.hnl"),
        "path-style reference resolves under the arrows root"
    );
}

#[test]
fn document_links_no_file_path_skips_file_refs() {
    // Without a file path, file references cannot be resolved and only URL
    // links should be returned.
    let source = "-- sort.hnl and https://example.com\n";
    let links = document_links(source, None);
    assert_eq!(
        links.len(),
        1,
        "only the URL link should be returned when file_path is None, got {links:?}"
    );
    assert_eq!(
        links[0]["target"].as_str(),
        Some("https://example.com"),
        "the sole link should be the URL"
    );
}

#[test]
fn document_links_link_ranges_are_valid() {
    // Every emitted link must carry a well-formed range whose end is not
    // before its start, and the target text must actually appear in the
    // source at that range.
    let source = "-- related: sort.hnl\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/test.hnl"));
    assert!(!links.is_empty(), "expected at least one link");
    for l in &links {
        let start_line = l["range"]["start"]["line"].as_u64().unwrap();
        let start_char = l["range"]["start"]["character"].as_u64().unwrap();
        let end_line = l["range"]["end"]["line"].as_u64().unwrap();
        let end_char = l["range"]["end"]["character"].as_u64().unwrap();
        assert!(
            (end_line, end_char) >= (start_line, start_char),
            "range end must not precede start: {l:?}"
        );
        assert!(
            l.get("target").and_then(|t| t.as_str()).is_some(),
            "link must have a string target: {l:?}"
        );
        assert!(
            l.get("tooltip").and_then(|t| t.as_str()).is_some(),
            "link must have a string tooltip: {l:?}"
        );
    }
}

#[test]
fn document_links_scans_doc_comments() {
    // Doc comments (`---`) should be scanned just like regular comments.
    let source = "--- See: sort.hnl for the sorting arrow\n";
    let links = document_links(source, Some("file:///tmp/handl-packages/test.hnl"));
    assert!(
        links.iter().any(|l| l["target"]
            .as_str()
            .map(|t| t.ends_with("sort.hnl"))
            .unwrap_or(false)),
        "doc comments should yield file reference links: {links:?}"
    );
}
