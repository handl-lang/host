use std::collections::BTreeSet;

use serde_json::{Value, json};

use handl_kit_canonical::{Kit, is_identifier_kind};

/// The canonical keyword set for the handl language.
///
/// Each entry is `(text, is_bool, is_null)`.
const KEYWORDS: &[(&str, bool, bool)] = &[
    ("true", true, false),
    ("false", true, false),
    ("null", false, true),
    ("match", false, false),
    ("handle", false, false),
    ("as", false, false),
    ("typeof", false, false),
    ("not", false, false),
    ("import", false, false),
    ("export", false, false),
    ("module", false, false),
];

pub fn completions(source: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();

    // Add keyword completions from the canonical keyword set.
    for &(text, is_bool, is_null) in KEYWORDS {
        let (item_kind, detail) = if is_bool {
            (21, "boolean")
        } else if is_null {
            (21, "null")
        } else {
            (14, "keyword")
        };
        items.push(json!({ "label": text, "kind": item_kind, "detail": detail }));
    }

    // Collect identifier names from the source.
    if let Ok(kit) = Kit::new() {
        if let Ok(tokens) = kit.lex(source) {
            let mut names: BTreeSet<String> = BTreeSet::new();
            for token in tokens.as_slice() {
                if is_identifier_kind(&token.kind) {
                    names.insert(token.text.clone());
                }
            }
            for name in names {
                items.push(json!({ "label": name, "kind": 6, "detail": "identifier" }));
            }
        }
    }

    items
}
