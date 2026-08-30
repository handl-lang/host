use std::collections::BTreeSet;

use serde_json::{Value, json};

use handl_kit_canonical::{DynFactory, DynLexer, Kit};
use handl_plugin_token_bool::BoolKind;
use handl_plugin_token_identifier::IdentKind;
use handl_plugin_token_null::NullKind;

pub fn completions(source: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();

    for (text, kind) in DynLexer::all_keywords() {
        let (item_kind, detail) = if kind.downcast_ref::<BoolKind>().is_some() {
            (21, "boolean")
        } else if kind.downcast_ref::<NullKind>().is_some() {
            (21, "null")
        } else {
            (14, "keyword")
        };
        items.push(json!({ "label": text, "kind": item_kind, "detail": detail }));
    }

    if let Ok(tokens) = Kit::new().lex(source) {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for token in tokens {
            if let Some(IdentKind::Ident(name)) = token.kind.downcast_ref::<IdentKind>() {
                names.insert(name.clone());
            }
        }
        for name in names {
            items.push(json!({ "label": name, "kind": 6, "detail": "identifier" }));
        }
    }

    items
}
