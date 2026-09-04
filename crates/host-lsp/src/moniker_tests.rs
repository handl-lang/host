use crate::moniker::moniker;

#[test]
fn moniker_for_arrow_file_uses_relative_path() {
    // An arrow file under /repo/handl-packages/collection/sort.hnl yields the
    // moniker `collection/sort` (path relative to the kit root,
    // extension stripped).
    let source = "sort = 1";
    let offset = source.find("sort").unwrap();
    let path = "/repo/handl-packages/collection/sort.hnl";
    let result = moniker(source, offset, Some(path));
    assert_eq!(result.len(), 1, "expected one moniker: {result:?}");
    let m = &result[0];
    assert_eq!(m["scheme"].as_str(), Some("handl"));
    assert_eq!(
        m["identifier"].as_str(),
        Some("collection/sort"),
        "identifier should be relative to kit root: {m:?}"
    );
    assert_eq!(m["unique"].as_str(), Some("identifier"));
}

#[test]
fn moniker_for_contract_file_uses_contracts_prefix() {
    // A symbol inside a contracts/ file uses contracts/<stem>/<name>.
    let source = "nonneg = 1";
    let offset = source.find("nonneg").unwrap();
    let path = "/repo/contracts/numbers.hnl";
    let result = moniker(source, offset, Some(path));
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0]["identifier"].as_str(),
        Some("contracts/numbers/nonneg"),
        "contract moniker should be contracts/<file>/<name>: {result:?}"
    );
}

#[test]
fn moniker_for_local_symbol_uses_filename_stem() {
    // A plain file with no arrows/ or contracts/ segment falls back to
    // <stem>/<name>.
    let source = "myvar = 1";
    let offset = source.find("myvar").unwrap();
    let path = "/repo/src/utils.hnl";
    let result = moniker(source, offset, Some(path));
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0]["identifier"].as_str(),
        Some("utils/myvar"),
        "fallback moniker should be <stem>/<name>: {result:?}"
    );
}

#[test]
fn moniker_on_non_identifier_returns_empty() {
    let source = "x = 42";
    let offset = source.find('4').unwrap();
    let result = moniker(source, offset, Some("/repo/handl-packages/x.hnl"));
    assert!(result.is_empty(), "cursor on a number yields no moniker: {result:?}");
}

#[test]
fn moniker_without_file_path_falls_back_to_name() {
    let source = "sort = 1";
    let offset = source.find("sort").unwrap();
    let result = moniker(source, offset, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["identifier"].as_str(), Some("sort"));
}
