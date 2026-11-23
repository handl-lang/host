//! Contextual-keyword retokenization (ADR-0158 `handle … acc` clause).
//!
//! Some grammar separators name a *contextual* keyword — a token kind in
//! the language's `keyword` category that deliberately has **no lexer
//! rule**, because reserving the word would break sources that use it as
//! an ordinary identifier. The canonical example is `acc`: it is the most
//! common accumulator name in handl sources, so `data/lexer_rules.yaml`
//! leaves it unreserved and `token.keyword.acc` exists only as a token
//! *kind* + grammar separator (`handle <e> with <h> acc <s0>`).
//!
//! The Pratt parser matches `child_separators` by token *kind*, so an
//! identifier `acc` never satisfies the `token.keyword.acc` separator and
//! the optional `acc` child is silently dropped — the clause and its seed
//! expression parse as stray following statements instead.
//!
//! [`parse_with_contextual_keywords`] closes the gap host-side, fully
//! data-driven (ADR-0044 — no `kind == "handle"` checks):
//!
//! 1. Parse the token stream normally.
//! 2. Scan the AST for nodes whose grammar shape declares an optional
//!    trailing child gated on a *contextual* separator — a keyword-category
//!    token kind with a surface `label` that no lexer rule produces.
//! 3. When the first non-ignorable token after such a node's span is an
//!    identifier carrying that label, retokenize that single token to the
//!    keyword kind and re-parse.
//! 4. Accept the re-parse only if it succeeds; on failure the token is
//!    permanently left as an identifier (the keyword reading was wrong at
//!    that position) and the scan continues with the next candidate.
//!
//! Anchoring on the parsed node's span is what keeps `acc` an ordinary
//! identifier everywhere else: `acc := 0`, `acc + 1`, `{acc: 1}`, and
//! `acc` in call/field position are never adjacent to a node missing its
//! optional child, so they are never retokenized. The one residual
//! ambiguity is inherent to contextual keywords — `handle f with h`
//! followed (across any layout) by `acc <expr>` reads as the clause,
//! exactly like `else` after `if`; a statement that must start with the
//! identifier `acc` cannot directly follow a `handle` form.
//!
//! Each loop iteration either permanently retokenizes one identifier
//! (identifier → keyword is monotonic; keyword tokens are never
//! candidates) or permanently rejects one token position, so the loop is
//! bounded by the number of identifier tokens carrying a contextual
//! label.

use std::collections::HashSet;

use handl_language::{Ast, AstNode, ChildSeparator, LangError, Language, Tokens};
use hnl_tool_parse::ParserRequest;
use rustc_hash::FxHashMap;

use crate::ParseEngineDyn;

/// A token position whose identifier text matches a contextual keyword
/// label in a separator-gated optional-child position.
struct Candidate {
    /// Index into `Tokens.0` of the identifier token to retokenize.
    token_index: usize,
    /// The keyword token kind to assign to the token.
    kind: String,
}

/// Parse `toks`, retokenizing contextual-keyword identifiers where the
/// grammar's optional-child separators call for them (see module docs).
///
/// Falls back to a plain [`ParseEngineDyn::parse_dyn`] whenever the
/// language declares no contextual separators or no candidate positions —
/// the result is then identical to the unmodified parse.
///
/// # Errors
///
/// Propagates the parse engine's error from the initial parse. Re-parse
/// failures after a tentative retokenization are *not* propagated — they
/// reject that candidate position and the scan continues.
pub(crate) fn parse_with_contextual_keywords(
    lang: &Language,
    engine: &dyn ParseEngineDyn,
    toks: &Tokens,
) -> Result<Ast, LangError> {
    let parse = |t: &Tokens| engine.parse_dyn(lang, &ParserRequest::new(t.clone()));

    // Contextual keyword kinds: `kind → surface label`, collected from every
    // optional-child separator in the grammar. Empty ⇒ nothing to do.
    let contextual = contextual_keywords(lang);
    if contextual.is_empty() {
        return parse(toks);
    }
    // Grammar shapes that declare optional children, keyed by produced node
    // kind — the only shapes that can gate on a contextual separator.
    let shapes = optional_child_shapes(lang);

    let mut toks = toks.clone();
    let mut rejected: HashSet<usize> = HashSet::new();
    let mut ast = parse(&toks)?;

    while let Some(cand) = find_candidate(lang, &ast, &toks, &shapes, &contextual, &rejected) {
        let old_kind = toks.0[cand.token_index].kind.clone();
        toks.0[cand.token_index].kind = cand.kind.clone();
        match parse(&toks) {
            Ok(new_ast) => ast = new_ast,
            Err(_) => {
                // The keyword reading does not parse here — this `acc` is
                // an ordinary identifier. Revert and never retry it.
                toks.0[cand.token_index].kind = old_kind;
                rejected.insert(cand.token_index);
            }
        }
    }
    Ok(ast)
}

/// Collect `kind → label` for every separator token kind that is a
/// *contextual* keyword: declared in the `keyword` category with a surface
/// `label`, but produced by **no** lexer rule (a kind with a lexer rule is
/// an ordinary reserved keyword and already reaches the parser).
fn contextual_keywords(lang: &Language) -> FxHashMap<String, String> {
    let mut map = FxHashMap::default();
    for row in lang.grammar.iter() {
        let shape = &row.child_shape;
        for &j in &shape.optional_children {
            // Optional child `j` is gated on `child_separators[j - 1]`
            // (the parser peeks the separator before parsing child `j`).
            let Some(sep) = j.checked_sub(1).and_then(|i| shape.child_separators.get(i)) else {
                continue;
            };
            for kind in separator_kinds(sep) {
                if let Some(label) = contextual_label(lang, kind) {
                    map.insert(kind.to_string(), label.to_string());
                }
            }
        }
    }
    map
}

/// Grammar rows' child shapes that declare optional children, keyed by the
/// node kind they produce.
fn optional_child_shapes<'l>(
    lang: &'l Language,
) -> FxHashMap<&'l str, Vec<&'l handl_language::AstShapeRef>> {
    let mut map: FxHashMap<&'l str, Vec<&'l handl_language::AstShapeRef>> = FxHashMap::default();
    for row in lang.grammar.iter() {
        let shape = &row.child_shape;
        if !shape.optional_children.is_empty() {
            map.entry(shape.node_kind.as_str()).or_default().push(shape);
        }
    }
    map
}

/// The surface label of `kind` when it is a contextual keyword — a
/// keyword-category token kind with a `label` that no lexer rule emits.
fn contextual_label<'l>(lang: &'l Language, kind: &'l str) -> Option<&'l str> {
    if !lang.token_in_category(kind, "keyword") {
        return None;
    }
    let def = lang.token_kind(kind)?;
    let label = def.label.as_deref()?;
    if lang.lexer_rules.iter().any(|r| r.token_kind == kind) {
        return None;
    }
    Some(label)
}

/// The token kinds a [`ChildSeparator`] can match.
fn separator_kinds(sep: &ChildSeparator) -> Vec<&str> {
    match sep {
        ChildSeparator::Required(k) | ChildSeparator::Optional(k) => vec![k.as_str()],
        ChildSeparator::OneOf(ks) => ks.iter().map(String::as_str).collect(),
    }
}

/// Whether `kind` is ignorable in the token stream — trivia or layout —
/// mirroring the Pratt cursor's data-driven skip (`lang.tokens` flags).
fn is_ignorable(lang: &Language, kind: &str) -> bool {
    lang.tokens
        .get_by_identity(kind)
        .map(|d| d.trivia || d.layout)
        .unwrap_or(false)
}

/// The number of children recorded in a node's `children` payload.
fn children_len(node: &AstNode) -> usize {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .map_or(0, |a| a.len())
}

/// Index of the first non-ignorable token starting at or after `offset`,
/// or `None` when the stream ends first. Mirrors the cursor's view: trivia
/// and layout tokens are transparent.
fn next_meaningful_token(lang: &Language, toks: &Tokens, offset: usize) -> Option<usize> {
    // Tokens are emitted in source order, so `start` is sorted — binary
    // search for the first token at or after `offset`.
    let start = toks
        .0
        .partition_point(|t| t.start < offset)
        .min(toks.0.len());
    if start >= toks.0.len() {
        return None;
    }
    toks.0[start..]
        .iter()
        .position(|t| !is_ignorable(lang, t.kind.as_str()))
        .map(|i| start + i)
}

/// Find the earliest (in source order) retokenization candidate: a node
/// missing an optional child whose separator is a contextual keyword,
/// immediately followed by an identifier carrying that keyword's label.
fn find_candidate(
    lang: &Language,
    ast: &Ast,
    toks: &Tokens,
    shapes: &FxHashMap<&str, Vec<&handl_language::AstShapeRef>>,
    contextual: &FxHashMap<String, String>,
    rejected: &HashSet<usize>,
) -> Option<Candidate> {
    let ident_kind = lang
        .token_kind_for_label("identifier")
        .unwrap_or("token.identifier");
    // Best candidate so far, ordered by (node span start, token index) —
    // source order, deterministic across runs.
    let mut best: Option<(usize, usize, String)> = None;

    for node in ast.iter() {
        let Some(node_shapes) = shapes.get(node.kind.as_str()) else {
            continue;
        };
        let Some(span) = node.span else {
            continue;
        };
        // The next child position the parser would attempt is the node's
        // current arity; it is a candidate only when that position is a
        // declared optional child (trailing-suffix optionals).
        let arity = children_len(node);
        let Some(tok_idx) = next_meaningful_token(lang, toks, span.end) else {
            continue;
        };
        let tok = &toks.0[tok_idx];
        if tok.kind != ident_kind || rejected.contains(&tok_idx) {
            continue;
        }
        for shape in node_shapes {
            if !shape.optional_children.contains(&arity) {
                continue;
            }
            let Some(sep) = arity
                .checked_sub(1)
                .and_then(|i| shape.child_separators.get(i))
            else {
                continue;
            };
            for kind in separator_kinds(sep) {
                let Some(label) = contextual.get(kind) else {
                    continue;
                };
                if tok.text == *label
                    && best
                        .as_ref()
                        .map_or(true, |(s, i, _)| (span.start, tok_idx) < (*s, *i))
                {
                    best = Some((span.start, tok_idx, kind.to_string()));
                }
            }
        }
    }

    best.map(|(_, token_index, kind)| Candidate { token_index, kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_kinds_flattens_all_variants() {
        assert_eq!(
            separator_kinds(&ChildSeparator::Required("a".into())),
            vec!["a"]
        );
        assert_eq!(
            separator_kinds(&ChildSeparator::Optional("b".into())),
            vec!["b"]
        );
        assert_eq!(
            separator_kinds(&ChildSeparator::OneOf(vec!["c".into(), "d".into()])),
            vec!["c", "d"]
        );
    }
}
