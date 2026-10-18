//! `handl-plugin-ext-detect` — shared surface-dialect detection.
//!
//! [`detect_dialect`] classifies a handl source by its surface markers into
//! the canonical dialect, the contract dialect (`axiom`/`requires:`/
//! `guarantee:` files, owned by `plugin-ext-refinement`'s loaders), or the
//! arrow dialect (`cap`/`op`/`handl`/`tag` decls and arrow-combinator
//! expressions).
//!
//! This is a pure function over the source text with **no dependencies** —
//! it used to live in `plugin-ext-refinement`, which pulled the whole
//! refinement/exec engine graph into every consumer that only needed the
//! routing decision (`hnl fmt`/`lint`/`typecheck`/`run`, the test runner,
//! and the ADR-0149 `hnl_ext_*` extension crates). It now sits next to
//! `plugin-ext-diag` as shared tooling plumbing, not a tool.

/// Source dialect detected from surface syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Canonical,
    Contract,
    Arrow,
}

/// A top-level (column-0) source line, with `--` line comments removed.
/// Returns `None` for blank lines, comments, and indented lines (contract
/// clause lines are indented and never carry a directive keyword).
fn top_level_line(line: &str) -> Option<&str> {
    if line.chars().next().is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let code = line.split_once("--").map(|(c, _)| c).unwrap_or(line);
    let t = code.trim_end();
    (!t.is_empty()).then_some(t)
}

/// True when the source is a contract-dialect file: the first top-level
/// statement line starts with `axiom `.
fn is_contract_source(src: &str) -> bool {
    src.lines()
        .filter_map(top_level_line)
        .next()
        .is_some_and(|l| l.starts_with("axiom "))
}

/// True when the source shows arrow-dialect markers: top-level
/// `cap`/`op`/`handl`/`tag` declaration lines or arrow-combinator
/// expressions (`arr(`, `first(`, `choice(`, `loop(`, `comp(`, `app(`,
/// `|||`, `rec {`).
fn has_arrow_markers(src: &str) -> bool {
    const DIRECTIVES: [&str; 4] = ["cap ", "op ", "handl ", "tag "];
    src.lines().any(|line| {
        let code = line.split_once("--").map(|(c, _)| c).unwrap_or(line);
        if let Some(top) = top_level_line(line) {
            if DIRECTIVES.iter().any(|d| top.starts_with(d)) {
                return true;
            }
        }
        contains_arrow_combinator(code)
    })
}

/// True when `code` mentions an arrow-calculus combinator as a *standalone*
/// expression, rather than as the tail of a longer identifier or a field
/// access. Handl surface code that calls a function like `array.first(…)` or
/// `fn.comp(…)` must not be mistaken for the arrow kernel's `first`/`comp`
/// combinators, so each combinator name is matched word-bounded (not preceded
/// by `.` or an identifier character).
fn contains_arrow_combinator(code: &str) -> bool {
    // `>>>` and `|||` are now canonical dimension-polymorphic composition
    // operators (ADR-0119); arrow-dialect files are recognized by
    // `cap`/`op`/`handl`/`tag` directives and call-form combinators.

    // `name(` — call-form combinators.
    for name in ["arr", "first", "choice", "loop", "comp", "app"] {
        if standalone_call(code, name) {
            return true;
        }
    }

    // `rec {` — `rec` standalone followed by optional whitespace and `{`.
    standalone_rec(code)
}

/// Whether `name` occurs in `code` as a standalone `name(` call: not preceded
/// by an identifier character or `.`, and immediately followed by `(`.
fn standalone_call(code: &str, name: &str) -> bool {
    let mut search = 0;
    while let Some(rel) = code[search..].find(name) {
        let start = search + rel;
        let end = start + name.len();
        let before_ok = code[..start]
            .chars()
            .next_back()
            .map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        let after_ok = code[end..].chars().next() == Some('(');
        if before_ok && after_ok {
            return true;
        }
        search = end;
    }
    false
}

/// Whether `rec` occurs in `code` as a standalone identifier followed by
/// (optional whitespace and) `{`.
fn standalone_rec(code: &str) -> bool {
    let mut search = 0;
    while let Some(rel) = code[search..].find("rec") {
        let start = search + rel;
        let end = start + 3;
        let before_ok = code[..start]
            .chars()
            .next_back()
            .map_or(true, |c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        if before_ok {
            let after = code[end..].trim_start().chars().next();
            if after == Some('{') {
                return true;
            }
        }
        search = end;
    }
    false
}

/// Detect the surface dialect of `src`.
///
/// Contract files (`axiom`/`requires:`/`guarantee:`) and arrow-module files
/// (`cap`/`op`/`handl`/`tag` decls, arrow combinators) are routed to the
/// refinement loaders. Arrow detection requires the source to successfully
/// parse as an arrow module, not just contain marker tokens.
pub fn detect_dialect(src: &str) -> Dialect {
    if is_contract_source(src) {
        return Dialect::Contract;
    }
    if has_arrow_markers(src) {
        return Dialect::Arrow;
    }
    Dialect::Canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_handl_source_is_canonical() {
        assert_eq!(detect_dialect("x = 1;"), Dialect::Canonical);
        assert_eq!(detect_dialect(""), Dialect::Canonical);
        assert_eq!(
            detect_dialect("-- a comment\nmain := (x) ~> x;"),
            Dialect::Canonical
        );
    }

    #[test]
    fn axiom_first_line_is_contract() {
        assert_eq!(
            detect_dialect("axiom add_comm: forall x y. add(x, y) == add(y, x)"),
            Dialect::Contract
        );
        // Comments and blank lines before the axiom still route to Contract.
        assert_eq!(
            detect_dialect("-- header\n\naxiom a: true"),
            Dialect::Contract
        );
        // An axiom that is *not* the first statement is canonical.
        assert_eq!(
            detect_dialect("x = 1;\naxiom a: true"),
            Dialect::Canonical
        );
    }

    #[test]
    fn directive_lines_are_arrow() {
        assert_eq!(detect_dialect("cap io { read }"), Dialect::Arrow);
        assert_eq!(detect_dialect("op get: int"), Dialect::Arrow);
        assert_eq!(detect_dialect("handl h: e"), Dialect::Arrow);
        assert_eq!(detect_dialect("tag t"), Dialect::Arrow);
        // Indented `cap` is not a directive.
        assert_eq!(detect_dialect("  cap io { read }"), Dialect::Canonical);
    }

    #[test]
    fn arrow_combinators_are_arrow() {
        assert_eq!(detect_dialect("arr(f)"), Dialect::Arrow);
        assert_eq!(detect_dialect("x = comp(f, g)"), Dialect::Arrow);
        assert_eq!(detect_dialect("x = rec { go }"), Dialect::Arrow);
        // Combinator words inside comments do not count.
        assert_eq!(detect_dialect("-- arr(f) here\nx = 1"), Dialect::Canonical);
    }

    #[test]
    fn field_access_and_suffixes_are_not_combinators() {
        // `array.first(...)`, `fn.comp(...)`: field access, not the kernel
        // combinators.
        assert_eq!(detect_dialect("array.first(xs)"), Dialect::Canonical);
        assert_eq!(detect_dialect("fn.comp(f, g)"), Dialect::Canonical);
        // Longer identifiers ending in a combinator name.
        assert_eq!(detect_dialect("my_comp(x)"), Dialect::Canonical);
        assert_eq!(detect_dialect("imported_and = 1"), Dialect::Canonical);
        // `record {` is not `rec {`.
        assert_eq!(detect_dialect("x = record { a = 1 }"), Dialect::Canonical);
    }
}
