use serde_json::{Value, json};

use handl_kit_canonical::{Kit, SpannedToken, is_comment_kind, is_identifier_kind};

use crate::diagnostics::offset_to_position;

/// Static description of a known arrow combinator's signature.
struct CombinatorInfo {
    /// The full signature label, e.g. `insert({ elem, acc })`.
    label: &'static str,
    /// Human-readable documentation for the combinator.
    documentation: &'static str,
    /// `(label, documentation)` for each parameter.
    parameters: &'static [(&'static str, &'static str)],
}

/// Look up the static signature information for a known arrow combinator.
fn known_combinator(name: &str) -> Option<CombinatorInfo> {
    match name {
        "arr" => Some(CombinatorInfo {
            label: "arr(\"op\", args…)",
            documentation:
                "Lift a named pure function into the arrow. The function runs on the arrow's \
                 input; `arr` wraps it as a pure step.",
            parameters: &[
                ("op", "the named operation to lift into the arrow"),
                ("args…", "additional arguments forwarded to the operation"),
            ],
        }),
        "comp" => Some(CombinatorInfo {
            label: "comp(a, b)",
            documentation:
                "Compose two arrows: the output of `a` feeds the input of `b`. Written as \
                 `a >>> b` in mathematical notation.",
            parameters: &[
                ("a (first arrow)", "the first arrow to run"),
                ("b (second arrow)", "the second arrow, fed by the output of the first"),
            ],
        }),
        "first" => Some(CombinatorInfo {
            label: "first(inner)",
            documentation:
                "Run `inner` on the first component of a pair, passing the second through \
                 unchanged. Fan-out / parallel wiring.",
            parameters: &[("inner", "the arrow to apply to the first component of the pair")],
        }),
        "choice" => Some(CombinatorInfo {
            label: "choice(l, r)",
            documentation:
                "Route on `Either`: `l` handles `Left`, `r` handles `Right`. ArrowChoice — \
                 conditional branching.",
            parameters: &[
                ("l (Left case)", "arrow handling the Left case"),
                ("r (Right case)", "arrow handling the Right case"),
            ],
        }),
        "app" => Some(CombinatorInfo {
            label: "app(\"name\", args…)",
            documentation:
                "Dynamic / higher-order arrow application. Given `f : A ~> B` and `x : A`, \
                 `app(f, x)` invokes `f` on `x`.",
            parameters: &[
                ("name", "the named arrow to apply"),
                ("args…", "arguments passed to the arrow"),
            ],
        }),
        "insert" => Some(CombinatorInfo {
            label: "insert({ elem, acc })",
            documentation:
                "Insert an element into a sorted list, maintaining sorted order. Used as a \
                 building block for `sort`.",
            parameters: &[(
                "{ elem, acc }",
                "record with `elem` to insert and `acc` accumulator list",
            )],
        }),
        _ => None,
    }
}

/// `textDocument/signatureHelp`: find the enclosing call at `offset` and
/// return LSP `SignatureHelp` JSON, or `None` if the cursor is not inside a
/// call.
pub fn signature_help(source: &str, offset: usize) -> Option<Value> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;

    // Locate the unmatched `(` that encloses the cursor.
    let lparen_idx = find_enclosing_lparen(&tokens, offset)?;

    // The identifier immediately before the `(` is the callee name.
    let name_idx = lparen_idx.checked_sub(1)?;
    let name_token = tokens.get(name_idx)?;
    if !is_identifier_kind(&name_token.kind) {
        return None;
    }
    let name = name_token.text.as_str();

    // Determine which parameter the cursor is currently on.
    let active_param = count_active_parameter(&tokens, lparen_idx, offset);

    let (label, documentation, parameters): (String, String, Vec<(String, String)>) =
        match known_combinator(name) {
            Some(info) => (
                info.label.to_string(),
                info.documentation.to_string(),
                info.parameters
                    .iter()
                    .map(|(l, d)| (l.to_string(), d.to_string()))
                    .collect(),
            ),
            None => {
                // Unknown function: synthesize a signature from the arguments
                // actually present in the source.
                let n = count_args(&tokens, lparen_idx);
                let labels: Vec<String> = (1..=n).map(|i| format!("arg{}", i)).collect();
                let label = if labels.is_empty() {
                    format!("{}()", name)
                } else {
                    format!("{}({})", name, labels.join(", "))
                };
                let parameters = labels
                    .iter()
                    .map(|l| (l.clone(), format!("parameter `{}` of `{}`", l, name)))
                    .collect();
                (label, format!("Call to `{}`", name), parameters)
            }
        };

    let params_json: Vec<Value> = parameters
        .iter()
        .map(|(l, d)| {
            json!({
                "label": l,
                "documentation": d,
            })
        })
        .collect();

    // `offset_to_position` is the crate's canonical byte-offset → LSP position
    // converter; we keep it in scope so this module stays consistent with the
    // rest of the LSP host (and ready if signature labels ever carry ranges).
    let _ = offset_to_position(source, offset);

    Some(json!({
        "signatures": [{
            "label": label,
            "documentation": documentation,
            "parameters": params_json,
        }],
        "activeSignature": 0,
        "activeParameter": active_param,
    }))
}

/// Scan backwards from the cursor to find the nearest unmatched `token.lparen`
/// that encloses `offset`. Returns the index of that `(`, or `None` if the
/// cursor is not inside any parenthesized group.
fn find_enclosing_lparen(tokens: &[SpannedToken], offset: usize) -> Option<usize> {
    // Prefer the token that contains the cursor; if the cursor sits between
    // tokens (e.g. right after `(`), fall back to the last token ending at or
    // before the cursor.
    let start_idx = tokens
        .iter()
        .position(|t| t.span.start <= offset && offset < t.span.end)
        .or_else(|| tokens.iter().rposition(|t| t.span.end <= offset))?;

    let mut depth: i32 = 0;
    let mut i = start_idx;
    loop {
        let k = tokens[i].kind.as_str();
        if k == "token.rparen" {
            depth += 1;
        } else if k == "token.lparen" {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}

/// Count how many top-level (depth-0) commas appear between the `(` at
/// `lparen_idx` and the cursor `offset`. The result is the zero-based index of
/// the parameter the cursor is on.
fn count_active_parameter(tokens: &[SpannedToken], lparen_idx: usize, offset: usize) -> usize {
    let mut depth: i32 = 0;
    let mut commas: usize = 0;
    let mut j = lparen_idx + 1;
    while j < tokens.len() {
        let t = &tokens[j];
        // Stop once we reach the cursor.
        if t.span.start >= offset {
            break;
        }
        let k = t.kind.as_str();
        if k == "token.lparen" || k == "token.lbrace" || k == "token.lbracket" {
            depth += 1;
        } else if k == "token.rparen" || k == "token.rbrace" || k == "token.rbracket" {
            if k == "token.rparen" && depth == 0 {
                break;
            }
            depth -= 1;
        } else if depth == 0 && k == "token.comma" {
            commas += 1;
        }
        j += 1;
    }
    commas
}

/// Count the number of top-level arguments inside the call whose `(` is at
/// `lparen_idx`. Used to synthesize signatures for unknown functions.
fn count_args(tokens: &[SpannedToken], lparen_idx: usize) -> usize {
    let mut depth: i32 = 0;
    let mut commas: usize = 0;
    let mut has_content = false;
    let mut j = lparen_idx + 1;
    while j < tokens.len() {
        let k = tokens[j].kind.as_str();
        if k == "token.lparen" || k == "token.lbrace" || k == "token.lbracket" {
            depth += 1;
        } else if k == "token.rparen" || k == "token.rbrace" || k == "token.rbracket" {
            if k == "token.rparen" && depth == 0 {
                break;
            }
            depth -= 1;
        } else if depth == 0 && k == "token.comma" {
            commas += 1;
        } else if depth == 0 && !is_comment_kind(k) {
            has_content = true;
        }
        j += 1;
    }
    if has_content {
        commas + 1
    } else {
        0
    }
}
