use std::collections::BTreeSet;

use serde_json::{Value, json};

use handl_kit_canonical::{LexFactory, LexKind, Kit};

pub fn completions(source: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();

    for (text, kind) in LexFactory::all_keywords() {
        let (item_kind, detail) = match kind {
            LexKind::Bool(_) => (21, "boolean"),
            LexKind::Null => (21, "null"),
            _ => (14, "keyword"),
        };
        items.push(json!({ "label": text, "kind": item_kind, "detail": detail }));
    }

    if let Ok(tokens) = Kit::new().lex(source) {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for token in tokens {
            if let LexKind::Identifier(name) = &token.kind {
                names.insert(name.clone());
            }
        }
        for name in names {
            items.push(json!({ "label": name, "kind": 6, "detail": "identifier" }));
        }
    }

    items
}
