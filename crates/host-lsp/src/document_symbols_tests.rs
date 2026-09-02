use crate::document_symbols::document_symbols;

/// Helper: does the symbol list contain an entry with the given name and kind?
fn has_symbol(symbols: &[serde_json::Value], name: &str, kind: u64) -> bool {
    symbols.iter().any(|s| {
        s["name"].as_str() == Some(name) && s["kind"].as_u64() == Some(kind)
    })
}

#[test]
fn document_symbols_for_arrow_file_has_loop_and_proc() {
    let src = "\
loop {
  acc: []
} (
  proc (state) -> match state.acc {
    []: { out: state.acc }
  }
)
";
    let symbols = document_symbols(src);
    assert!(
        has_symbol(&symbols, "loop", 23),
        "expected a `loop` function symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "proc", 23),
        "expected a `proc` function symbol, got: {symbols:?}"
    );
}

#[test]
fn document_symbols_for_arrow_file_has_match_arms() {
    let src = "\
loop {
  remaining: input,
  acc:       []
} (
  proc (state) -> match state.remaining {
    []: {
      out: state.acc
    },

    [h, ...t]: {
      state: { remaining: t, acc: state.acc }
    }
  }
)
";
    let symbols = document_symbols(src);
    assert!(
        has_symbol(&symbols, "[]", 13),
        "expected a `[]` match-arm variable symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "[h, ...t]", 13),
        "expected a `[h, ...t]` match-arm variable symbol, got: {symbols:?}"
    );
}

#[test]
fn document_symbols_for_arrow_file_has_object_keys_and_where() {
    let src = "\
loop {
  elem: input.0,
  acc:  input.1
} (
  proc (state) -> match state.acc {
    [h, ...t]:
      { state: { elem: state.elem, acc: t } }
      where rest_inserted = insert({ elem: state.elem, acc: t })
  }
)
";
    let symbols = document_symbols(src);
    // Object keys become Field (8) symbols.
    assert!(
        has_symbol(&symbols, "elem", 8),
        "expected an `elem` field symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "acc", 8),
        "expected an `acc` field symbol, got: {symbols:?}"
    );
    // The `where` binding becomes a Variable (13) symbol.
    assert!(
        has_symbol(&symbols, "rest_inserted", 13),
        "expected a `rest_inserted` where-binding symbol, got: {symbols:?}"
    );
}

#[test]
fn document_symbols_for_contract_file_has_axioms() {
    let src = "\
axiom list.head : List a -> a
axiom id : a -> a
axiom fn.apply : (a x a) -> a
";
    let symbols = document_symbols(src);
    assert!(
        has_symbol(&symbols, "list.head", 11),
        "expected a `list.head` interface symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "id", 11),
        "expected an `id` interface symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "fn.apply", 11),
        "expected an `fn.apply` interface symbol, got: {symbols:?}"
    );

    // The detail should carry the signature text.
    let head = symbols
        .iter()
        .find(|s| s["name"].as_str() == Some("list.head"))
        .expect("list.head symbol should exist");
    let detail = head["detail"].as_str().expect("detail should be a string");
    assert!(
        detail.contains("List a -> a"),
        "detail should contain the signature, got: {detail}"
    );
}

#[test]
fn document_symbols_for_binding_file_has_variables() {
    let src = "\
server: type.any = http_server.new()
route: type.null = http_server.route(server, \"GET\", \"/hello\", \"Hello\")
start: type.null = http_server.start(server, \"127.0.0.1:8081\")
";
    let symbols = document_symbols(src);
    assert!(
        has_symbol(&symbols, "server", 13),
        "expected a `server` variable symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "route", 13),
        "expected a `route` variable symbol, got: {symbols:?}"
    );
    assert!(
        has_symbol(&symbols, "start", 13),
        "expected a `start` variable symbol, got: {symbols:?}"
    );
}

#[test]
fn document_symbols_empty_source_returns_empty() {
    let symbols = document_symbols("");
    assert!(symbols.is_empty(), "expected no symbols for empty source");
}

#[test]
fn document_symbols_match_expression_has_symbol() {
    let src = "\
result = match input {
  []: 0,
  [h, ...t]: 1
}
";
    let symbols = document_symbols(src);
    assert!(
        has_symbol(&symbols, "match", 23),
        "expected a `match` function symbol, got: {symbols:?}"
    );
    // The top-level binding should also be reported as a variable.
    assert!(
        has_symbol(&symbols, "result", 13),
        "expected a `result` binding symbol, got: {symbols:?}"
    );
}

#[test]
fn document_symbols_if_expression_has_symbol() {
    let src = "value = if cond then 1 else 2";
    let symbols = document_symbols(src);
    assert!(
        has_symbol(&symbols, "if", 23),
        "expected an `if` function symbol, got: {symbols:?}"
    );
}
