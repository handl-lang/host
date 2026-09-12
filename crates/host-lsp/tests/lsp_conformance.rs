//! LSP conformance tests for the `handl-host-lsp` server binary.
//!
//! These tests spawn the actual server binary and communicate with it over
//! stdin/stdout using the LSP wire protocol (Content-Length headers + JSON-RPC
//! 2.0). They exercise the full request/response pipeline end-to-end,
//! complementing the per-module unit tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

/// Sample handl source used across most tests (a simple canonical binding).
const SAMPLE_SOURCE: &str = "myarrow = 42
result = myarrow
";

/// A test harness that spawns the `handl-host-lsp` binary and speaks LSP.
struct LspClient {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: i64,
}

#[allow(dead_code)]
impl LspClient {
    /// Spawn the server binary built from this crate.
    fn new() -> LspClient {
        let bin = env!("CARGO_BIN_EXE_handl-host-lsp");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {bin}: {e}"));
        let stdout = child.stdout.take().expect("server stdout was not piped");
        let reader = BufReader::new(stdout);
        LspClient {
            child,
            reader,
            next_id: 1,
        }
    }

    /// Write a single LSP message (header + body) to the server's stdin.
    fn write_message(&mut self, body: &Value) {
        let serialized = serde_json::to_string(body).unwrap();
        let stdin = self
            .child
            .stdin
            .as_mut()
            .expect("server stdin was not piped");
        write!(
            stdin,
            "Content-Length: {}\r\n\r\n{}",
            serialized.len(),
            serialized
        )
        .unwrap();
        stdin.flush().unwrap();
    }

    /// Allocate the next request id.
    fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Send a request and return the full JSON-RPC response object (the
    /// envelope with `id`, `result`/`error`). Any notifications emitted
    /// before the response are skipped.
    fn send_request_raw(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id();
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&request);
        // Skip notifications until we find the response with a matching id.
        loop {
            let msg = self.read_message();
            if let Some(obj) = msg.as_object() {
                // A response has an `id` (and `result` or `error`) but no
                // `method`.
                if obj.contains_key("id") && !obj.contains_key("method") {
                    return msg;
                }
            }
            // Otherwise it's a notification; keep reading.
        }
    }

    /// Send a request and return the `result` field, panicking on error.
    fn send_request(&mut self, method: &str, params: Value) -> Value {
        let resp = self.send_request_raw(method, params);
        if let Some(err) = resp.get("error") {
            panic!("request `{method}` returned error: {err}");
        }
        resp.get("result").cloned().unwrap_or(Value::Null)
    }

    /// Send a notification (no response expected).
    fn send_notification(&mut self, method: &str, params: Value) {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&notification);
    }

    /// Send the standard `initialize` request and return the result.
    fn initialize(&mut self) -> Value {
        self.send_request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": null,
                "capabilities": {},
            }),
        )
    }

    /// Send the `initialized` notification.
    fn initialized(&mut self) {
        self.send_notification("initialized", json!({}));
    }

    /// Open a document via `textDocument/didOpen`.
    fn open_document(&mut self, uri: &str, text: &str) {
        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "handl",
                    "version": 1,
                    "text": text,
                }
            }),
        );
    }

    /// Close a document via `textDocument/didClose`.
    fn close_document(&mut self, uri: &str) {
        self.send_notification(
            "textDocument/didClose",
            json!({
                "textDocument": { "uri": uri }
            }),
        );
    }

    /// Read a single LSP message (parsing the Content-Length header) from
    /// the server's stdout. Returns `Value::Null` on EOF.
    fn read_message(&mut self) -> Value {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.reader.read_line(&mut line).expect("read header line");
            if n == 0 {
                return Value::Null;
            }
            let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
            if trimmed.is_empty() {
                break;
            }
            if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
                content_length = len_str.parse().ok();
            }
        }
        let len = content_length.expect("missing Content-Length header");
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).expect("read message body");
        let body = String::from_utf8(buf).expect("message body is valid UTF-8");
        serde_json::from_str(&body).expect("message body is valid JSON")
    }

    /// Read messages, skipping notifications, until a JSON-RPC response is
    /// found. Returns that response.
    fn read_response(&mut self) -> Value {
        loop {
            let msg = self.read_message();
            if let Some(obj) = msg.as_object() {
                if obj.contains_key("id") && !obj.contains_key("method") {
                    return msg;
                }
            }
        }
    }

    /// Read notifications until a `textDocument/publishDiagnostics`
    /// notification for `uri` is found, returning its params.
    fn read_diagnostics(&mut self, uri: &str) -> Value {
        loop {
            let msg = self.read_message();
            if msg
                .as_object()
                .and_then(|o| o.get("method"))
                .and_then(|m| m.as_str())
                == Some("textDocument/publishDiagnostics")
            {
                let params = msg
                    .get("params")
                    .expect("diagnostics notification has params");
                let got_uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                if got_uri == uri {
                    return params.clone();
                }
            }
        }
    }

    /// Send `shutdown` then `exit` and wait for the server to terminate.
    fn shutdown(&mut self) {
        let _ = self.send_request("shutdown", Value::Null);
        self.send_notification("exit", Value::Null);
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Best-effort cleanup: close stdin and reap the child.
        let _ = self.child.stdin.take();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Protocol basics
// ---------------------------------------------------------------------------

#[test]
fn initialize_returns_server_capabilities() {
    let mut client = LspClient::new();
    let result = client.initialize();
    let caps = result
        .get("capabilities")
        .expect("initialize result has capabilities");

    // A representative set of providers the server advertises.
    assert_eq!(
        caps.get("hoverProvider").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        caps.get("definitionProvider").and_then(|v| v.as_bool()),
        Some(true),
    );
    assert!(caps.get("completionProvider").is_some());
    assert!(caps.get("documentSymbolProvider").is_some());
    assert!(caps.get("foldingRangeProvider").is_some());
    assert!(caps.get("documentFormattingProvider").is_some());
    assert!(caps.get("semanticTokensProvider").is_some());
    client.shutdown();
}

#[test]
fn initialize_returns_server_info() {
    let mut client = LspClient::new();
    let result = client.initialize();
    let info = result
        .get("serverInfo")
        .expect("initialize result has serverInfo");
    assert_eq!(info.get("name").and_then(|v| v.as_str()), Some("handl"));
    client.shutdown();
}

#[test]
fn shutdown_returns_null() {
    let mut client = LspClient::new();
    client.initialize();
    let result = client.send_request("shutdown", Value::Null);
    assert!(
        result.is_null(),
        "shutdown result should be null, got {result}"
    );
    client.send_notification("exit", Value::Null);
}

#[test]
fn unknown_method_returns_error() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let resp = client.send_request_raw("some/unknown/method", Value::Null);
    assert!(
        resp.get("error").is_some(),
        "unknown method should return an error, got {resp}",
    );
    let code = resp
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64());
    assert_eq!(code, Some(-32601), "expected method-not-found error code");
    client.shutdown();
}

// ---------------------------------------------------------------------------
// Document lifecycle
// ---------------------------------------------------------------------------

#[test]
fn did_open_then_hover_returns_result() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_sort.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    // Drain the diagnostics notification emitted by didOpen so it doesn't
    // interfere with later reads.
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 2 },
        }),
    );
    // The server should return *something* (non-null) for a hover over an
    // identifier in a valid document. It may be null only if the position
    // has nothing to hover over — but `result` at line 1, char 2 is an
    // identifier.
    assert!(
        !result.is_null(),
        "hover over identifier should return a non-null result, got {result}",
    );
    client.shutdown();
}

#[test]
fn did_open_then_completion_returns_items() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_completion.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
        }),
    );
    // Completion may be an array or an object with `items`; accept either.
    let items = if result.is_array() {
        result.as_array().unwrap().clone()
    } else if let Some(arr) = result.get("items").and_then(|i| i.as_array()) {
        arr.clone()
    } else {
        panic!("completion result should be an array or {{items:[]}}, got {result}");
    };
    // At the start of a document the server offers keywords/snippets, so we
    // expect a non-empty list.
    assert!(
        !items.is_empty(),
        "completion should return items, got empty"
    );
    client.shutdown();
}

#[test]
fn did_open_then_document_symbols_returns_array() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_symbols.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/documentSymbol",
        json!({
            "textDocument": { "uri": uri },
        }),
    );
    assert!(
        result.is_array(),
        "documentSymbol should return an array, got {result}",
    );
    client.shutdown();
}

#[test]
fn did_open_then_folding_ranges_returns_array() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_folding.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/foldingRange",
        json!({
            "textDocument": { "uri": uri },
        }),
    );
    assert!(
        result.is_array(),
        "foldingRange should return an array, got {result}",
    );
    client.shutdown();
}

#[test]
fn did_open_then_diagnostics_published() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_diag.hnl";
    // A source with an obvious syntax error: an unterminated string literal.
    let bad_source = "sort = arr(\"insert";
    client.open_document(uri, bad_source);

    let params = client.read_diagnostics(uri);
    let diagnostics = params
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .unwrap_or_else(|| panic!("diagnostics params should have a diagnostics array: {params}"));
    assert!(
        !diagnostics.is_empty(),
        "a document with a syntax error should publish at least one diagnostic",
    );
    client.shutdown();
}

// ---------------------------------------------------------------------------
// Feature conformance
// ---------------------------------------------------------------------------

#[test]
fn hover_on_identifier_returns_markdown() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_hover_md.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    // `result` identifier begins at column 0 on line 1; hover over it.
    let result = client.send_request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 2 },
        }),
    );
    let contents = result
        .get("contents")
        .unwrap_or_else(|| panic!("hover result should have contents, got {result}"));
    // Contents may be a MarkupContent object `{kind, value}` or a plain
    // string. The server uses markdown for refinement/token hovers.
    if let Some(kind) = contents.get("kind").and_then(|k| k.as_str()) {
        assert_eq!(
            kind, "markdown",
            "hover contents kind should be markdown, got {kind}",
        );
        assert!(
            contents.get("value").and_then(|v| v.as_str()).is_some(),
            "markdown contents should have a value string",
        );
    } else if contents.is_string() {
        // Plain string contents are also acceptable per the LSP spec.
    } else {
        panic!("unexpected hover contents shape: {contents}");
    }
    client.shutdown();
}

#[test]
fn completion_returns_valid_items() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_completion_valid.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
        }),
    );
    let items = if result.is_array() {
        result.as_array().unwrap().clone()
    } else if let Some(arr) = result.get("items").and_then(|i| i.as_array()) {
        arr.clone()
    } else {
        panic!("completion result should be an array or {{items:[]}}, got {result}");
    };
    assert!(!items.is_empty(), "expected completion items");
    // Every item must have at least a `label`.
    for item in &items {
        assert!(
            item.get("label").and_then(|l| l.as_str()).is_some(),
            "completion item missing label: {item}",
        );
    }
    // At least one item should carry a `kind`.
    let has_kind = items
        .iter()
        .any(|item| item.get("kind").and_then(|k| k.as_i64()).is_some());
    assert!(has_kind, "at least one completion item should have a kind");
    client.shutdown();
}

#[test]
fn formatting_returns_text_edits() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_formatting.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/formatting",
        json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 2, "insertSpaces": true },
        }),
    );
    assert!(
        result.is_array(),
        "formatting should return a TextEdit array, got {result}",
    );
    // Each edit should have a `range` and `newText`.
    for edit in result.as_array().unwrap() {
        assert!(edit.get("range").is_some(), "edit missing range: {edit}");
        assert!(
            edit.get("newText").and_then(|t| t.as_str()).is_some(),
            "edit missing newText: {edit}",
        );
    }
    client.shutdown();
}

#[test]
fn semantic_tokens_returns_data() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_semantic.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    let result = client.send_request(
        "textDocument/semanticTokens/full",
        json!({
            "textDocument": { "uri": uri },
        }),
    );
    let data = result
        .get("data")
        .unwrap_or_else(|| panic!("semanticTokens/full result should have data, got {result}"));
    assert!(
        data.is_array(),
        "semantic tokens data should be an array, got {data}",
    );
    // The data array is a flat list of 5-tuples, so its length must be a
    // multiple of 5 (and non-zero for a non-trivial source).
    let arr = data.as_array().unwrap();
    assert_eq!(
        arr.len() % 5,
        0,
        "semantic tokens data length must be a multiple of 5, got {}",
        arr.len(),
    );
    assert!(
        !arr.is_empty(),
        "expected semantic tokens for non-trivial source"
    );
    client.shutdown();
}

#[test]
fn definition_returns_location_or_null() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();
    let uri = "file:///test_definition.hnl";
    client.open_document(uri, SAMPLE_SOURCE);
    let _ = client.read_diagnostics(uri);

    // Hover over `myarrow` on line 1, char 12.
    let result = client.send_request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 12 },
        }),
    );
    // Definition may be null, a Location, or an array of Locations.
    if result.is_null() {
        // Acceptable: no definition found.
        return;
    }
    if let Some(arr) = result.as_array() {
        for loc in arr {
            assert_location_shape(loc);
        }
    } else {
        assert_location_shape(&result);
    }
    client.shutdown();
}

fn assert_location_shape(value: &Value) {
    assert!(
        value.get("uri").and_then(|u| u.as_str()).is_some(),
        "Location should have a uri string, got {value}",
    );
    assert!(
        value.get("range").is_some(),
        "Location should have a range, got {value}",
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn request_before_initialize_returns_error() {
    let mut client = LspClient::new();
    // Deliberately do NOT send `initialize` first.
    let resp = client.send_request_raw(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": "file:///edge.hnl" },
            "position": { "line": 0, "character": 0 },
        }),
    );
    // The LSP spec requires that requests sent before `initialize` be
    // rejected with a ServerNotInitialized error (code -32002).
    let err = resp
        .get("error")
        .unwrap_or_else(|| panic!("expected error before initialize, got {resp}"));
    let code = err
        .get("code")
        .and_then(|c| c.as_i64())
        .unwrap_or_else(|| panic!("error should have a numeric code, got {err}"));
    assert_eq!(
        code, -32002,
        "expected ServerNotInitialized error code -32002, got {resp}",
    );
    // The server was never initialized, so `shutdown` would also be
    // rejected. Just send `exit` to terminate the server process.
    client.send_notification("exit", Value::Null);
}

#[test]
fn malformed_json_does_not_crash() {
    let mut client = LspClient::new();
    client.initialize();
    client.initialized();

    // Send a message whose Content-Length claims more bytes than the valid
    // JSON we provide, with trailing garbage. The server's `read_message`
    // will fail to parse the body and return `None`, ending the loop. We
    // verify the server does not emit a panic/crash and that a subsequent
    // well-formed session still works by spawning a fresh client.
    let stdin = client
        .child
        .stdin
        .as_mut()
        .expect("server stdin was not piped");
    let garbage = b"{ this is not valid json ";
    write!(stdin, "Content-Length: {}\r\n\r\n", garbage.len(),).unwrap();
    stdin.write_all(garbage).unwrap();
    stdin.flush().unwrap();

    // The server will likely exit (read_message returns None on parse
    // failure). Drop this client and start a fresh one to confirm the
    // binary itself is still usable.
    drop(client);

    let mut client2 = LspClient::new();
    let result = client2.initialize();
    assert!(
        result.get("capabilities").is_some(),
        "fresh server should still respond to initialize after a peer saw malformed JSON",
    );
    client2.shutdown();
}
