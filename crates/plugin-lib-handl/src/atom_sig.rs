//! # Atom signatures — `op "name" : Sig` declarations → checker type values
//!
//! P-013 T-148 ("atoms as arrows"): every host atom declared in a `.hnl`
//! package source carries a handl signature — `op "math.abs" :
//! Float ~> Float`. Until now the signature text was stripped before
//! eval and read by nothing. This module is the *reader*:
//!
//! - [`parse_op_declarations`] extracts `(qualified_name, sig_text)`
//!   pairs from a `.hnl` source (the same line-scan discipline as
//!   `parse_cap_declarations` — resilient, comment-aware, no parser
//!   dependency).
//! - [`sig_to_type_value`] interprets the signature *text* as a checker
//!   type value — the tagged JSON shape
//!   `{"__tag__": "type.*", "__registry__": "handl.canonical", …}` that
//!   [`TypeSignature::type_signature`](handl_platform::TypeSignature)
//!   implementations and `ModuleLookup::lookup_type_signature` already
//!   exchange (ADR-0083 / ADR-0151).
//!
//! The sig grammar is the ADR-0077 surface form, deliberately smaller
//! than the dependent type language:
//!
//! ```text
//! Sig       := Dom (~>|-> Cod)?            -- right-associative
//! Dom/Cod   := '(' Items ')' | App | Atom ['...']
//! Items     := Item (('*'|',') Item)*      -- `*` and `,` both product
//! Item      := Type | Type '...'           -- trailing `...` = rest
//! Type      := Name Type                   -- application (`List Int`)
//!            | '(' Items ')'               -- parens group/product
//!            | Name | '_'
//! ```
//!
//! Name mapping onto checker sorts follows the dependent checker's
//! builtin table (`dep_builtin_or_tag`): `Int`/`Nat`/`Number`/`Float` →
//! `type.number`, `Bool` → `type.bool`, `String`/`Str` → `type.string`,
//! `Unit` → `type.unit`, `Object` → `type.obj`, `Array`/bare `List` →
//! `type.array{element: type.unknown}`, `Fn`/`Function` → the dynamic
//! `type.function` top, `List T` → `type.array{element: T}`. Names with
//! no checker sort (`Bytes`, `Value`, type variables like `a`, `_`,
//! unknown identifiers) degrade to `type.unknown` — the dynamic top —
//! so an unrecognized sig spelling can never make typecheck *stricter*
//! than the pre-T-148 behavior. A sig that fails to parse at all yields
//! `None` and simply attaches nothing.
//!
//! [`TypeSignature::type_signature`]: https://docs.rs/handl-platform

use serde_json::{Map, Value};

/// The `__registry__` id stamped into minted signature type values.
///
/// Must match [`handl_plugin_lib_tag::CANONICAL_REGISTRY_ID`] and the
/// [`NbeTypecheckEngine`](https://docs.rs/hnl-tool-typecheck-nbe) default
/// registry — the checker only treats `type.*` values as sorts when the
/// registry id agrees. (`plugin-lib-handl` cannot depend on
/// `plugin-lib-tag` — the tag crate already depends on this one — so the
/// constant is duplicated here deliberately.)
pub const TYPE_SIG_REGISTRY_ID: &str = "handl.canonical";

// ---------------------------------------------------------------------------
// `op "name" : Sig` extraction
// ---------------------------------------------------------------------------

/// Parse `op` declarations from a `.hnl` source string, returning
/// `(qualified_name, signature_text)` pairs in source order.
///
/// Mirrors [`parse_cap_declarations`](crate::parse_cap_declarations): a
/// lightweight line scan that skips `--` comment lines and silently
/// ignores non-matching lines. Only the single-line form
/// `op "name" : Sig` is recognized — indented `guarantee:`/`requires:`/
/// `termination:` continuation lines carry no signature and are skipped
/// naturally. Both `~>` and the legacy `->` arrow spellings are accepted
/// (the split happens in [`sig_to_type_value`], which handles either).
pub fn parse_op_declarations(src: &str) -> Vec<(String, String)> {
    let mut ops = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        // Whole-word `op` prefix (same discipline as the `cap` scan).
        let rest = match trimmed.strip_prefix("op") {
            Some(rest) if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') => {
                rest.trim_start()
            }
            _ => continue,
        };
        // Quoted op name — `"..."` or `'...'`.
        let Some((name, after_name)) = extract_quoted(rest) else {
            continue;
        };
        // Expect `:` then the signature text.
        let Some(sig) = after_name.trim().strip_prefix(':') else {
            continue;
        };
        let sig = sig.trim();
        if !sig.is_empty() {
            ops.push((name, sig.to_string()));
        }
    }
    ops
}

/// Extract a quoted string from the start of `s`, returning the content
/// and the remainder. Supports `"..."` and `'...'`.
fn extract_quoted(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some((rest[..end].to_string(), &rest[end + quote.len_utf8()..]))
}

// ---------------------------------------------------------------------------
// Signature text → checker type value
// ---------------------------------------------------------------------------

/// Interpret an `op` signature text (`"Float ~> Float"`,
/// `"(Float * Float) ~> Float"`, `"Unit ~> Float"`, …) as a checker type
/// value — `{"__tag__": "type.function", "__registry__": …, "param": …,
/// "body": …}` for arrows, or the leaf sort for a bare type.
///
/// Returns `None` when the text does not parse as a signature — callers
/// treat that as "no declared signature" (the pre-T-148 dynamic-top
/// behavior), never as an error.
pub fn sig_to_type_value(sig: &str) -> Option<Value> {
    let mut p = SigParser::new(sig);
    let ty = p.parse_arrow()?;
    p.skip_ws();
    if !p.eof() {
        return None;
    }
    Some(ty)
}

/// Mint a tagged type value `{"__tag__": tag, "__registry__": id, …}`.
fn ty(tag: &str, fields: Map<String, Value>) -> Value {
    let mut m = fields;
    m.insert(
        handl_language::TAG_VALUE_KEY.to_string(),
        Value::String(tag.to_string()),
    );
    m.insert(
        handl_language::REGISTRY_VALUE_KEY.to_string(),
        Value::String(TYPE_SIG_REGISTRY_ID.to_string()),
    );
    Value::Object(m)
}

/// `{"__tag__": "type.unknown"}` — the dynamic top.
fn type_unknown() -> Value {
    ty("type.unknown", Map::new())
}

/// `{"__tag__": "type.array", "element": t}` — the uniform-array encoding.
fn array_of(element: Value) -> Value {
    let mut m = Map::new();
    m.insert("element".to_string(), element);
    ty("type.array", m)
}

/// `{"__tag__": "type.array", "elements": […]}` — the tuple/product
/// encoding the checker uses for `(x, y) ~>` domains.
fn tuple_of(elements: Vec<Value>) -> Value {
    let mut m = Map::new();
    m.insert("elements".to_string(), Value::Array(elements));
    ty("type.array", m)
}

/// `{"__tag__": "type.function", "param": p, "body": b}`.
fn function_of(param: Value, body: Value) -> Value {
    let mut m = Map::new();
    m.insert("param".to_string(), param);
    m.insert("body".to_string(), body);
    ty("type.function", m)
}

/// The dynamic function top — `Fn`/`Function` in sig position.
fn function_top() -> Value {
    function_of(type_unknown(), type_unknown())
}

/// Leaf-type mapping for a bare identifier, following the dependent
/// checker's builtin table. Names with no checker sort degrade to
/// `type.unknown` (see module docs).
fn leaf_type(name: &str) -> Value {
    match name {
        "Int" | "Nat" | "Number" | "Float" => ty("type.number", Map::new()),
        "Bool" => ty("type.bool", Map::new()),
        "String" | "Str" => ty("type.string", Map::new()),
        "Unit" => ty("type.unit", Map::new()),
        "Object" => ty("type.obj", Map::new()),
        "Array" | "List" => array_of(type_unknown()),
        "Fn" | "Function" => function_top(),
        // `Value`, `Bytes`, type variables (`a`, `_`, …) and any unknown
        // identifier: the dynamic top — never stricter than before.
        _ => type_unknown(),
    }
}

// ---------------------------------------------------------------------------
// Recursive-descent parser
// ---------------------------------------------------------------------------

/// A byte-level cursor over the signature text.
struct SigParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> SigParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            s: text.as_bytes(),
            i: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn eof(&self) -> bool {
        self.i >= self.s.len()
    }

    /// Eat the literal `lit` at the cursor (after whitespace).
    fn eat(&mut self, lit: &str) -> bool {
        self.skip_ws();
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }

    /// Peek the next non-whitespace byte.
    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.s.get(self.i).copied()
    }

    /// Parse an identifier `[A-Za-z_][A-Za-z0-9_]*` (ASCII — sig names are
    /// ASCII in practice; a non-ASCII byte simply fails the parse).
    fn ident(&mut self) -> Option<&'a str> {
        self.skip_ws();
        let start = self.i;
        if self.i >= self.s.len() {
            return None;
        }
        let b = self.s[self.i];
        if !(b.is_ascii_alphabetic() || b == b'_') {
            return None;
        }
        self.i += 1;
        while self.i < self.s.len() {
            let b = self.s[self.i];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.i += 1;
            } else {
                break;
            }
        }
        // SAFETY: the consumed span is ASCII by construction.
        std::str::from_utf8(&self.s[start..self.i]).ok()
    }

    /// `Sig := Dom (~>|-> Cod)?` — right-associative, so `A ~> B ~> C`
    /// reads `A ~> (B ~> C)`.
    fn parse_arrow(&mut self) -> Option<Value> {
        let dom = self.parse_dom()?;
        if self.eat("~>") || self.eat("->") {
            let cod = self.parse_arrow()?;
            return Some(function_of(dom, cod));
        }
        Some(dom)
    }

    /// A domain/codomain position: parenthesized items, an application,
    /// an atom, or `()`.
    fn parse_dom(&mut self) -> Option<Value> {
        let mut ty = if self.peek() == Some(b'(') {
            self.parse_paren()?
        } else {
            self.parse_app_or_atom()?
        };
        // Trailing `...` — a rest parameter: `Value...` /
        // `(A * B...)`. Encoded as the uniform array of the item type.
        if self.eat("...") {
            ty = array_of(ty);
        }
        Some(ty)
    }

    /// `'(' Items ')'` — `()` is `type.unit`; a single non-product item
    /// unwraps (parens group, and `(List Int)` is the application, not a
    /// 1-tuple); two or more items — or any explicit `*`/`,` separator —
    /// form the `type.array{elements}` product.
    fn parse_paren(&mut self) -> Option<Value> {
        if !self.eat("(") {
            return None;
        }
        if self.eat(")") {
            return Some(ty("type.unit", Map::new()));
        }
        let mut items: Vec<Value> = Vec::new();
        let mut had_sep = false;
        loop {
            items.push(self.parse_dom()?);
            if self.eat("*") || self.eat(",") {
                had_sep = true;
                continue;
            }
            break;
        }
        if !self.eat(")") {
            return None;
        }
        if items.len() == 1 && !had_sep {
            // `(T)` groups; `(List Int)` is the application itself.
            Some(items.pop().unwrap())
        } else {
            Some(tuple_of(items))
        }
    }

    /// An application `Name Type` or a bare atom. Juxtaposition is
    /// application (`List Int`); only `List` has a checker encoding —
    /// every other `Name Type` degrades to `type.unknown` (the arg is
    /// still parsed so malformed tails fail the whole sig).
    fn parse_app_or_atom(&mut self) -> Option<Value> {
        let name = self.ident()?;
        // Application: the name is followed by another type. `List T` is
        // the only application form the sig grammar uses; anything else
        // (`F x`, …) parses but maps to the dynamic top.
        let is_app = matches!(self.peek(), Some(b'(') | Some(b'_'))
            || self.peek().is_some_and(|b| b.is_ascii_alphabetic());
        if is_app {
            let arg = self.parse_dom()?;
            return Some(if name == "List" {
                array_of(arg)
            } else {
                type_unknown()
            });
        }
        Some(leaf_type(name))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(v: &Value) -> &str {
        v.get("__tag__").and_then(Value::as_str).unwrap()
    }

    #[test]
    fn parses_op_declarations() {
        let src = "-- comment\ncap \"math.abs\"\nop \"math.abs\" : Float ~> Float\n  guarantee: True\nop 'math.e' : Unit ~> Float\nnot an op line\n";
        let ops = parse_op_declarations(src);
        assert_eq!(
            ops,
            vec![
                ("math.abs".to_string(), "Float ~> Float".to_string()),
                ("math.e".to_string(), "Unit ~> Float".to_string()),
            ]
        );
    }

    #[test]
    fn simple_arrow() {
        let v = sig_to_type_value("Float ~> Float").unwrap();
        assert_eq!(tag(&v), "type.function");
        assert_eq!(tag(&v["param"]), "type.number");
        assert_eq!(tag(&v["body"]), "type.number");
        assert_eq!(v["__registry__"].as_str().unwrap(), TYPE_SIG_REGISTRY_ID);
    }

    #[test]
    fn product_domain() {
        let v = sig_to_type_value("(Float * Float) ~> Float").unwrap();
        assert_eq!(tag(&v["param"]), "type.array");
        let els = v["param"]["elements"].as_array().unwrap();
        assert_eq!(els.len(), 2);
        assert_eq!(tag(&els[0]), "type.number");
        // Comma-separated tuples are the same product encoding.
        let v2 = sig_to_type_value("(String, Value, Value) ~> Value").unwrap();
        assert_eq!(tag(&v2["param"]), "type.array");
        assert_eq!(v2["param"]["elements"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn unit_and_nullary() {
        let v = sig_to_type_value("Unit ~> Float").unwrap();
        assert_eq!(tag(&v["param"]), "type.unit");
        assert_eq!(tag(&v["body"]), "type.number");
        // `() ~> Value` — the empty domain is also unit.
        let v = sig_to_type_value("() ~> Value").unwrap();
        assert_eq!(tag(&v["param"]), "type.unit");
        assert_eq!(tag(&v["body"]), "type.unknown");
    }

    #[test]
    fn rest_and_application() {
        let v = sig_to_type_value("Value... ~> Value").unwrap();
        assert_eq!(tag(&v["param"]), "type.array");
        assert_eq!(tag(&v["param"]["element"]), "type.unknown");

        let v = sig_to_type_value("Bytes ~> (List Int)").unwrap();
        assert_eq!(tag(&v["param"]), "type.unknown");
        assert_eq!(tag(&v["body"]), "type.array");
        assert_eq!(tag(&v["body"]["element"]), "type.number");

        let v = sig_to_type_value("(Array * Value...) ~> Array").unwrap();
        let els = v["param"]["elements"].as_array().unwrap();
        assert_eq!(els.len(), 2);
        assert_eq!(tag(&els[1]), "type.array");
    }

    #[test]
    fn unknown_names_degrade_to_top() {
        for name in ["Bytes", "Value", "a", "_", "Weird"] {
            let v = sig_to_type_value(&format!("{name} ~> {name}")).unwrap();
            assert_eq!(tag(&v["param"]), "type.unknown", "{name}");
            assert_eq!(tag(&v["body"]), "type.unknown", "{name}");
        }
        // `Fn`/`Function` get the dynamic function top.
        let v = sig_to_type_value("Fn ~> Value").unwrap();
        assert_eq!(tag(&v["param"]), "type.function");
        assert_eq!(tag(&v["param"]["param"]), "type.unknown");
    }

    #[test]
    fn legacy_arrow_and_right_assoc() {
        let v = sig_to_type_value("String -> String").unwrap();
        assert_eq!(tag(&v), "type.function");
        // `A ~> B ~> C` reads `A ~> (B ~> C)`.
        let v = sig_to_type_value("Unit ~> Float ~> Bool").unwrap();
        assert_eq!(tag(&v["body"]), "type.function");
        assert_eq!(tag(&v["body"]["body"]), "type.bool");
    }

    #[test]
    fn malformed_sigs_return_none() {
        for bad in [
            "",
            "Float",
            "~>",
            "Float ~>",
            "(Float *",
            "Float * * Float",
            "Float ~> ~>",
        ] {
            // `Float` alone is actually a valid bare-type sig — keep it.
            if bad == "Float" {
                assert!(sig_to_type_value(bad).is_some());
            } else {
                assert!(sig_to_type_value(bad).is_none(), "{bad}");
            }
        }
    }
}
