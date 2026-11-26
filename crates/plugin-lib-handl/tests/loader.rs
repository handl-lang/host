//! Parity test: the self-hosted handl loader (`handl-packages/loader/loader.hnl`)
//! must produce identical results to the Rust bootstrap transforms.
//!
//! The loader needs the `char`/`str` floors: `char` is native
//! (`plugin-lib-char-native`) and `str` is `str-handl`, itself prepared by
//! the Rust bootstrap path (the bootstrap paradox — ADR-0094).

use std::sync::Arc;

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_handl::CharModule;
use handl_plugin_lib_handl::{
    HandlLoader, HandlModule, ModuleResolver, build_str_handl_module, expand_tag_declarations,
    install_loader, loader, parse_cap_declarations, parse_tag_declarations, strip_comment_lines,
    strip_declaration_lines, transform_export_to_object,
};

/// Bootstrap implementations, re-exposed for parity checking.

/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

mod bootstrap {
    pub fn strip_comment_lines(src: &str) -> String {
        src.lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn strip_declaration_lines(src: &str) -> String {
        src.lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                if line.starts_with(' ') || line.starts_with('\t') {
                    if trimmed.starts_with("guarantee")
                        || trimmed.starts_with("requires")
                        || trimmed.starts_with("termination")
                    {
                        return false;
                    }
                }
                for kw in ["cap", "op", "handl"] {
                    if let Some(rest) = trimmed.strip_prefix(kw) {
                        if rest.is_empty()
                            || rest.starts_with(' ')
                            || rest.starts_with('\t')
                            || rest.starts_with('"')
                        {
                            return false;
                        }
                    }
                }
                // NOTE: `import`/`from` lines are kept (hoisted by
                // `transform_export_to_object`).
                true
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Split a module source into its top-level statements, returning each
    /// statement's verbatim text in source order. Mirrors
    /// `handl_runtime::module_prep::top_level_segments` (and the handl
    /// `top_level_segments` in `loader.hnl`): boundaries are `;` at bracket
    /// depth 0, a depth-0 newline not followed by an indented line, and a
    /// depth-0 `export`/`import`/`from` keyword mid-statement (with
    /// `from`/`import`-tail suppression and `.`-dotted names never
    /// splitting). Strings, `--` line comments, and `/-- … --/` block
    /// comments are tracked so keywords inside them are ignored.
    fn top_level_segments(src: &str) -> Vec<String> {
        #[derive(Clone, Copy, PartialEq)]
        enum Lead {
            Pending,
            From,
            Import,
            Export,
            Other,
        }

        let bytes = src.as_bytes();
        let mut segs: Vec<String> = Vec::new();
        let mut start = 0usize;
        let mut depth: i32 = 0;
        let mut in_str: Option<u8> = None;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut lead = Lead::Pending;
        let mut tail_seen = false;

        macro_rules! cut {
            ($end:expr, $next:expr, $lead:expr) => {{
                let text = &src[start..$end];
                if !text.trim().is_empty() {
                    segs.push(text.to_string());
                }
                start = $next;
                lead = $lead;
                tail_seen = false;
            }};
        }

        let mut i = 0usize;
        while i < bytes.len() {
            let c = bytes[i];
            if in_line_comment {
                if c == b'\n' {
                    in_line_comment = false;
                } else {
                    i += 1;
                    continue;
                }
            }
            if in_block_comment {
                if c == b'-' && bytes.get(i + 1) == Some(&b'-') && bytes.get(i + 2) == Some(&b'/') {
                    in_block_comment = false;
                    i += 3;
                } else {
                    i += 1;
                }
                continue;
            }
            if let Some(q) = in_str {
                if c == b'\\' {
                    i += 1;
                } else if c == q {
                    in_str = None;
                }
                if lead == Lead::Pending {
                    lead = Lead::Other;
                }
                i += 1;
                continue;
            }
            if c == b'-' && bytes.get(i + 1) == Some(&b'-') {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if c == b'/' && bytes.get(i + 1) == Some(&b'-') && bytes.get(i + 2) == Some(&b'-') {
                in_block_comment = true;
                if lead == Lead::Pending {
                    lead = Lead::Other;
                }
                i += 3;
                continue;
            }
            match c {
                b'"' | b'\'' => {
                    in_str = Some(c);
                    if lead == Lead::Pending {
                        lead = Lead::Other;
                    }
                    i += 1;
                }
                b'(' | b'[' | b'{' => {
                    depth += 1;
                    if lead == Lead::Pending {
                        lead = Lead::Other;
                    }
                    i += 1;
                }
                b')' | b']' | b'}' => {
                    depth -= 1;
                    if lead == Lead::Pending {
                        lead = Lead::Other;
                    }
                    i += 1;
                }
                b';' if depth == 0 => {
                    cut!(i, i + 1, Lead::Pending);
                    i += 1;
                }
                b'\n' if depth == 0 => {
                    let next = bytes.get(i + 1).copied();
                    if next != Some(b' ') && next != Some(b'\t') {
                        cut!(i, i + 1, Lead::Pending);
                    }
                    i += 1;
                }
                _ if depth == 0 && (c.is_ascii_alphabetic() || c == b'_') => {
                    let mut j = i + 1;
                    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
                    {
                        j += 1;
                    }
                    let word = &src[i..j];
                    let dotted = i > 0 && bytes[i - 1] == b'.';
                    let kw = if !dotted && (word == "export" || word == "import" || word == "from")
                    {
                        Some(word)
                    } else {
                        None
                    };
                    match (lead, kw) {
                        (Lead::Pending, Some("export")) => lead = Lead::Export,
                        (Lead::Pending, Some("from")) => lead = Lead::From,
                        (Lead::Pending, Some("import")) => lead = Lead::Import,
                        (Lead::Pending, _) => lead = Lead::Other,
                        (Lead::From, Some("import" | "export")) if !tail_seen => {
                            tail_seen = true;
                        }
                        (Lead::Import, Some("from")) if !tail_seen => {
                            tail_seen = true;
                        }
                        (_, Some(k)) => {
                            let next_lead = match k {
                                "export" => Lead::Export,
                                "from" => Lead::From,
                                _ => Lead::Import,
                            };
                            cut!(i, i, next_lead);
                        }
                        (_, None) => {}
                    }
                    i = j;
                }
                _ => {
                    if lead == Lead::Pending && !c.is_ascii_whitespace() {
                        lead = Lead::Other;
                    }
                    i += 1;
                }
            }
        }
        let text = &src[start..];
        if !text.trim().is_empty() {
            segs.push(text.to_string());
        }
        segs
    }

    /// The text after the `export` keyword when `trimmed` begins an export
    /// declaration — `None` when `export` does not sit at a word boundary
    /// at the start of the statement. Mirrors
    /// `handl_runtime::module_prep::export_decl_rest`.
    fn export_decl_rest(trimmed: &str) -> Option<&str> {
        let rest = trimmed.strip_prefix("export")?;
        if rest.is_empty() {
            return Some(rest);
        }
        let next = rest.as_bytes()[0];
        if !(next.is_ascii_alphanumeric() || next == b'_') {
            Some(rest.trim_start())
        } else {
            None
        }
    }

    fn collect_import_lines(src: &str) -> Vec<String> {
        top_level_segments(src)
            .iter()
            .map(|seg| seg.trim().to_string())
            .filter(|seg| is_import_line(seg))
            .map(|seg| {
                let line = from_export_to_import(&seg)
                    .map(|(import_line, _)| import_line)
                    .unwrap_or(seg);
                // Inside a `do {}` block, `import <names>` trailing-tokens
                // parsing does not stop at the newline — terminate each
                // import with `;` so the following statement can't merge in.
                format!("{};", line)
            })
            .collect()
    }

    /// If a `from` line uses the re-export form `from "..." export x, y`,
    /// rewrite it to a plain import `from "..." import x as __x, y as __y`
    /// and return the (name, local-alias) pairs that should appear in the
    /// module record. Mirrors `handl_runtime::module_prep`'s handling.
    fn from_export_to_import(line: &str) -> Option<(String, Vec<(String, String)>)> {
        let (path, rest) = extract_quoted_path(line)?;
        let rest = rest.trim_start();
        if let Some(tail) = rest.strip_prefix("export") {
            if !tail.starts_with(' ') && !tail.starts_with('\t') && !tail.is_empty() {
                return None;
            }
            let names: Vec<String> = tail
                .trim_start()
                .split(',')
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .collect();
            if names.is_empty() {
                return None;
            }
            let aliases: Vec<String> = names.iter().map(|n| format!("__{n}")).collect();
            let imports: Vec<String> = names
                .iter()
                .zip(&aliases)
                .map(|(n, a)| format!("{} as {}", n, a))
                .collect();
            let import_line = format!("from \"{}\" import {}", path, imports.join(", "));
            return Some((import_line, names.into_iter().zip(aliases).collect()));
        }
        None
    }

    /// Extract the first quoted path in a `from` line and the text that
    /// follows it.
    fn extract_quoted_path<'a>(line: &'a str) -> Option<(String, &'a str)> {
        let trimmed = line.trim_start();
        let after_from = trimmed.strip_prefix("from")?;
        let after_from = after_from.trim_start();
        let quote = after_from.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let after_quote = &after_from[quote.len_utf8()..];
        let end = after_quote.find(quote)?;
        let path = &after_quote[..end];
        let rest = &after_quote[end + quote.len_utf8()..];
        Some((path.to_string(), rest))
    }

    fn drop_import_lines(src: &str) -> String {
        top_level_segments(src)
            .iter()
            .filter(|seg| !is_import_line(seg.trim()))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Names bound by a `from "spec" import a, b as c` / `import name`
    /// statement — they join the no-exports record projection. Mirrors
    /// `loader.hnl`'s `import_bound_locals`.
    fn import_bound_locals(line: &str) -> Vec<String> {
        let t = line.trim_start();
        if t.starts_with("from") {
            let Some((_, rest)) = extract_quoted_path(t) else {
                return Vec::new();
            };
            let rest = rest.trim_start();
            let Some(tail) = rest.strip_prefix("import") else {
                return Vec::new();
            };
            if !tail.is_empty() && !tail.starts_with(' ') && !tail.starts_with('\t') {
                return Vec::new();
            }
            let tail = tail.trim_start();
            let inner = if tail.starts_with('{') && tail.ends_with('}') && tail.len() > 1 {
                &tail[1..tail.len() - 1]
            } else {
                tail
            };
            inner
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty() && *item != "*")
                .map(|item| {
                    item.find(" as ")
                        .map(|i| item[i + 4..].trim().to_string())
                        .unwrap_or_else(|| item.to_string())
                })
                .filter(|local| !local.is_empty())
                .collect()
        } else if let Some(rest) = t.strip_prefix("import") {
            let rest = rest.trim_start();
            if rest.starts_with('"') || rest.starts_with('\'') {
                return Vec::new();
            }
            let name = rest
                .find("from")
                .map(|i| rest[..i].trim().to_string())
                .unwrap_or_else(|| rest.trim().to_string());
            if is_valid_ident(&name) {
                vec![name]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    fn is_import_line(trimmed: &str) -> bool {
        if let Some(rest) = trimmed.strip_prefix("import") {
            if rest.is_empty()
                || rest.starts_with(' ')
                || rest.starts_with('\t')
                || rest.starts_with('"')
                || rest.starts_with('\'')
            {
                return true;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("from") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                return true;
            }
        }
        false
    }

    fn is_valid_ident(s: &str) -> bool {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) if c.is_alphabetic() || c == '_' => {
                chars.all(|c| c.is_alphanumeric() || c == '_')
            }
            _ => false,
        }
    }

    /// Whether `rest` (the text after `export`) opens a `data` family
    /// declaration. Mirrors `loader.hnl`'s `is_data_decl_rest` /
    /// `handl_runtime::module_prep::is_data_decl_rest`.
    fn is_data_decl_rest(rest: &str) -> bool {
        let rest = rest.trim_start();
        let Some(after) = rest.strip_prefix("data") else {
            return false;
        };
        match after.chars().next() {
            Some(c) if c.is_alphanumeric() || c == '_' => return false,
            _ => {}
        }
        after
            .trim_start()
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    }

    /// Skip a balanced `open…close` group at the start of `s`, returning
    /// the remainder. `None` when the group never closes. Mirrors
    /// `loader.hnl`'s `skip_balanced` /
    /// `handl_runtime::module_prep::skip_balanced`.
    fn skip_balanced<'a>(s: &'a str, open: char, close: char) -> Option<&'a str> {
        if !s.starts_with(open) {
            return Some(s);
        }
        let mut depth = 0i32;
        let mut in_str: Option<u8> = None;
        for (i, c) in s.char_indices() {
            if let Some(q) = in_str {
                if c as u32 == q as u32 {
                    in_str = None;
                }
                continue;
            }
            match c {
                '"' | '\'' => in_str = Some(c as u8),
                _ if c == open => depth += 1,
                _ if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&s[i + c.len_utf8()..]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The interior of the balanced `open…close` group at the start of
    /// `s`. `None` when the group never closes. Mirrors `loader.hnl`'s
    /// `balanced_inner` / `handl_runtime::module_prep::balanced_inner`.
    fn balanced_inner<'a>(s: &'a str, open: char, close: char) -> Option<&'a str> {
        if !s.starts_with(open) {
            return None;
        }
        let mut depth = 0i32;
        let mut in_str: Option<u8> = None;
        for (i, c) in s.char_indices() {
            if let Some(q) = in_str {
                if c as u32 == q as u32 {
                    in_str = None;
                }
                continue;
            }
            match c {
                '"' | '\'' => in_str = Some(c as u8),
                _ if c == open => depth += 1,
                _ if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&s[open.len_utf8()..i]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Split `s` at `sep` characters at bracket depth zero (strings
    /// respected) — the `|` constructor separator of a `data F :: …`
    /// arm list and the `,` entry separator of the record form. Mirrors
    /// `loader.hnl`'s `split_top_level` /
    /// `handl_runtime::module_prep::split_top_level`.
    fn split_top_level(s: &str, sep: char) -> Vec<&str> {
        let bytes = s.as_bytes();
        let sep = sep as u8;
        let mut items = Vec::new();
        let mut depth = 0i32;
        let mut in_str: Option<u8> = None;
        let mut start = 0usize;
        let mut i = 0usize;
        while i < bytes.len() {
            let c = bytes[i];
            if let Some(q) = in_str {
                if c == b'\\' {
                    i += 1;
                } else if c == q {
                    in_str = None;
                }
            } else {
                match c {
                    b'"' | b'\'' => in_str = Some(c),
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    _ if c == sep && depth == 0 => {
                        items.push(&s[start..i]);
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        items.push(&s[start..]);
        items
    }

    /// The leading identifier of a constructor arm (`C1(x : T)` → `C1`).
    /// Mirrors `loader.hnl`'s `ctor_arm_name`.
    fn ctor_arm_name(arm: &str) -> Option<String> {
        let arm = arm.trim_start();
        let end = arm
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(arm.len());
        let name = &arm[..end];
        is_valid_ident(name).then(|| name.to_string())
    }

    /// The field key of a `name : value` record entry — an identifier
    /// followed by a depth-zero `:` that is not `::`. Mirrors
    /// `loader.hnl`'s `record_field_key` /
    /// `handl_runtime::module_prep::record_field_key`.
    fn record_field_key(item: &str) -> Option<String> {
        let item = item.trim();
        let bytes = item.as_bytes();
        let mut depth = 0i32;
        let mut in_str: Option<u8> = None;
        let mut i = 0usize;
        while i < bytes.len() {
            let c = bytes[i];
            if let Some(q) = in_str {
                if c == b'\\' {
                    i += 1;
                } else if c == q {
                    in_str = None;
                }
            } else {
                match c {
                    b'"' | b'\'' => in_str = Some(c),
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    b':' if depth == 0 => {
                        if bytes
                            .get(i + 1)
                            .is_some_and(|&n| n == b':' || n == b'=')
                        {
                            return None;
                        }
                        let key = item[..i].trim();
                        return is_valid_ident(key).then(|| key.to_string());
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        None
    }

    /// The names a `data` declaration binds, for the module record: the
    /// family name plus each constructor name. `decl` is the full
    /// `data …` text (keyword included). `None` when no family name is
    /// recognizable — the caller reports rather than silently dropping
    /// the declaration. Mirrors `loader.hnl`'s `data_decl_bound_names` /
    /// `handl_runtime::module_prep::data_decl_bound_names`.
    fn data_decl_bound_names(decl: &str) -> Option<Vec<String>> {
        let mut s = decl.trim_start().strip_prefix("data")?.trim_start();
        let fam_end = s
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(s.len());
        let family = &s[..fam_end];
        if !is_valid_ident(family) {
            return None;
        }
        let mut names = vec![family.to_string()];
        s = s[fam_end..].trim_start();
        if s.starts_with('<') {
            s = skip_balanced(s, '<', '>')?.trim_start();
        } else if s.starts_with('(') {
            s = skip_balanced(s, '(', ')')?.trim_start();
        }
        if let Some(ctors) = s.strip_prefix(":=") {
            let rhs = ctors.trim_start();
            if rhs.starts_with('{') {
                // `data F := { C : R, … }` — the record form's keys are
                // the constructor names.
                if let Some(inner) = balanced_inner(rhs, '{', '}') {
                    for entry in split_top_level(inner, ',') {
                        if let Some(key) = record_field_key(entry) {
                            if is_valid_ident(&key) && !names.iter().any(|n| *n == key) {
                                names.push(key);
                            }
                        }
                    }
                }
            } else {
                // `data F := C0 | C1(x : T) | …` — constructor arms.
                for arm in split_top_level(rhs, '|') {
                    if let Some(name) = ctor_arm_name(arm) {
                        if !names.iter().any(|n| *n == name) {
                            names.push(name);
                        }
                    }
                }
            }
        }
        Some(names)
    }

    /// Whether `rhs` is a top-level arrow *signature* — contract data
    /// erased to `null`. Mirrors
    /// `handl_runtime::module_prep::is_signature_rhs`: a depth-0 `~[` is
    /// only a signature when `graded_arrow_is_signature` holds (an
    /// n-cell *value* export like `export alpha := f ~[2]~> g` is kept);
    /// an ungraded `~>` is a signature only when a params list carries a
    /// bare `[`/`{` element (a list/record *type* in params position).
    fn is_signature_rhs(rhs: &str) -> bool {
        let bytes = rhs.as_bytes();
        let mut depth: i32 = 0;
        let mut paren_depth: i32 = 0;
        let mut in_str: Option<u8> = None;
        let mut prev_sig = 0u8;
        let mut list_type_param = false;
        let mut i = 0usize;
        while i < bytes.len() {
            let c = bytes[i];
            if let Some(q) = in_str {
                if c == b'\\' {
                    i += 1;
                } else if c == q {
                    in_str = None;
                }
                i += 1;
                continue;
            }
            match c {
                b'"' | b'\'' => in_str = Some(c),
                b'(' => {
                    depth += 1;
                    paren_depth += 1;
                }
                b'[' | b'{' => {
                    depth += 1;
                    if paren_depth > 0 && (prev_sig == b'(' || prev_sig == b',') {
                        list_type_param = true;
                    }
                }
                b')' => {
                    depth -= 1;
                    paren_depth -= 1;
                }
                b']' | b'}' => depth -= 1,
                b'~' if depth == 0 => match bytes.get(i + 1) {
                    Some(&b'[') => return graded_arrow_is_signature(rhs, i),
                    Some(&b'>') => return list_type_param,
                    _ => {}
                },
                _ => {}
            }
            if !c.is_ascii_whitespace() {
                prev_sig = c;
            }
            i += 1;
        }
        false
    }

    /// Whether a depth-0 `~[` at byte `tilde` opens a graded-arrow
    /// *signature* `Type ~[1]~> Type` (erase) rather than a genuine
    /// n-cell *value* `f ~[N]~> g` (keep): all corpus signatures grade
    /// `~[1]~>`; any other grade is a cell term. Grade `1` erases only
    /// when the domain is type-shaped — `(...)`/`{...}` params, a bare
    /// `[T]` list type, or a `Capitalized` type head. Mirrors
    /// `handl_runtime::module_prep::graded_arrow_is_signature`.
    fn graded_arrow_is_signature(rhs: &str, tilde: usize) -> bool {
        let bytes = rhs.as_bytes();
        let mut gdepth = 1i32;
        let mut gend = bytes.len();
        let mut j = tilde + 2;
        while j < bytes.len() {
            match bytes[j] {
                b'[' => gdepth += 1,
                b']' => {
                    gdepth -= 1;
                    if gdepth == 0 {
                        gend = j;
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if rhs[tilde + 2..gend].trim() != "1" {
            return false;
        }
        let lhs = rhs[..tilde].trim();
        match lhs.as_bytes().last() {
            None => true,
            Some(b')') | Some(b'}') => true,
            Some(b']') => lhs.starts_with('['),
            Some(_) => lhs.as_bytes()[0].is_ascii_uppercase(),
        }
    }

    /// Whether a non-binding top-level `seg` continues the in-flight
    /// item instead of opening a new statement. Mirrors `loader.hnl`'s
    /// `segment_continues_item` — see
    /// `handl_runtime::module_prep::segment_continues_item` for the full
    /// rationale; `lines` is the item's accumulated expression text
    /// (oldest-first — the oracle appends).
    fn segment_continues_item(lines: &[String], seg: &str) -> bool {
        let seg = seg.trim_start();
        let Some(&b) = seg.as_bytes().first() else {
            return false;
        };
        if matches!(
            b,
            b'.' | b')'
                | b']'
                | b'}'
                | b','
                | b';'
                | b':'
                | b'?'
                | b'|'
                | b'&'
                | b'='
                | b'<'
                | b'>'
                | b'+'
                | b'*'
                | b'/'
                | b'%'
                | b'^'
                | b'~'
                | b'@'
                | b'#'
                | b'$'
                | b'`'
                | b'-'
                | b'!'
        ) {
            return true;
        }
        let first_word = seg
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()
            .unwrap_or("");
        if matches!(
            first_word,
            "else"
                | "catch"
                | "finally"
                | "then"
                | "elif"
                | "otherwise"
                | "and"
                | "or"
                | "xor"
                | "not"
                | "in"
                | "of"
                | "is"
                | "as"
                | "mod"
                | "div"
                | "rem"
                | "where"
                | "with"
                | "when"
                | "unless"
                | "until"
                | "by"
                | "via"
        ) {
            return true;
        }
        if b == b'{' && item_expects_block(lines) {
            return true;
        }
        item_expression_incomplete(lines)
    }

    /// Whether the accumulated expression text ends mid-expression.
    /// Mirrors `loader.hnl`'s `item_expression_incomplete`; `lines` is
    /// oldest-first here (the oracle appends), so the tail is `last()`.
    fn item_expression_incomplete(lines: &[String]) -> bool {
        let Some(tail) = lines.last().map(|l| l.trim_end()) else {
            return false;
        };
        let Some(&tb) = tail.as_bytes().last() else {
            return false;
        };
        if matches!(
            tb,
            b'+' | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'='
                | b'<'
                | b'>'
                | b'!'
                | b'&'
                | b'|'
                | b'^'
                | b'~'
                | b'?'
                | b':'
                | b','
                | b'.'
        ) {
            return true;
        }
        if tail.ends_with("->")
            || tail.ends_with("=>")
            || tail.ends_with("~>")
            || tail.ends_with("|>")
            || tail.ends_with("<|")
        {
            return true;
        }
        let word = tail
            .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
            .find(|w| !w.is_empty())
            .unwrap_or("");
        matches!(
            word,
            "if" | "else"
                | "match"
                | "while"
                | "for"
                | "handle"
                | "do"
                | "then"
                | "in"
                | "of"
                | "and"
                | "or"
                | "xor"
                | "not"
                | "is"
                | "as"
                | "mod"
                | "div"
                | "rem"
                | "return"
                | "yield"
                | "where"
                | "with"
        )
    }

    /// Whether the accumulated expression ends inside a construct still
    /// owed its `{ … }` block. Mirrors `loader.hnl`'s
    /// `item_expects_block`; `lines` is oldest-first (the oracle
    /// appends), joined directly.
    fn item_expects_block(lines: &[String]) -> bool {
        const KEYWORDS: &[&str] = &[
            "if", "else", "match", "while", "for", "handle", "do", "then", "in", "of",
        ];
        let text = lines.join("\n");
        let bytes = text.as_bytes();
        let mut last_end: Option<usize> = None;
        let mut i = 0usize;
        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_alphanumeric() || c == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if KEYWORDS.contains(&&text[start..i]) {
                    last_end = Some(i);
                }
            } else {
                i += 1;
            }
        }
        let Some(pos) = last_end else {
            return false;
        };
        let mut depth = 0i32;
        let mut seen_open = false;
        for &c in &bytes[pos..] {
            match c {
                b'{' => {
                    depth += 1;
                    seen_open = true;
                }
                b'}' => depth -= 1,
                _ => {}
            }
        }
        !seen_open || depth > 0
    }

    pub fn transform_export_to_object(src: &str) -> String {
        // Mirrors `loader.hnl`'s `transform_export_to_object`: the source is
        // segmented into top-level statements (`top_level_segments`), so an
        // `export` is recognized wherever a statement can begin — at column
        // 0, indented, after `;`, or mid-line — and its RHS is captured
        // whole. Non-private entries become object fields; private entries
        // become `do`-block assignments emitted before the object
        // (private-binding hoisting, mirroring
        // `handl_runtime::module_prep`). An `export` declaration that can't
        // be transformed panics — never a silent drop (the handl side
        // fails via `error.raise`; the fallback bootstrap panics).
        // In-flight item: a `name := expr` binding or a verbatim
        // `export data …` declaration — mirrors `loader.hnl`'s `cur`
        // record (`kind: "bind" | "decl"`).
        enum Cur {
            Bind {
                name: String,
                lines: Vec<String>,
            },
            Decl {
                lines: Vec<String>,
                exports: Vec<String>,
            },
        }
        impl Cur {
            fn lines_mut(&mut self) -> &mut Vec<String> {
                match self {
                    Cur::Bind { lines, .. } | Cur::Decl { lines, .. } => lines,
                }
            }

            /// Read-only view of the accumulated lines — used by the
            /// continuation check. Mirrors `loader.hnl`'s `c.lines`.
            fn lines(&self) -> &[String] {
                match self {
                    Cur::Bind { lines, .. } | Cur::Decl { lines, .. } => lines,
                }
            }
        }
        // A finalized private/do-block statement: a `name := expr`
        // assignment or a verbatim `export data …` declaration.
        enum Priv {
            Bind {
                name: String,
                expr_lines: Vec<String>,
            },
            Decl {
                lines: Vec<String>,
            },
        }

        let mut entries: Vec<(String, Vec<String>)> = Vec::new();
        let mut privates: Vec<Priv> = Vec::new();
        let mut current: Option<Cur> = None;
        let mut current_is_private: bool = false;
        // Whether the module's last top-level statement is a binding. When
        // it is not (a trailing `do { … }` or record literal) the body's
        // own value is the module record and no name projection is
        // appended — mirrors `loader.hnl`'s `tail` flag.
        let mut tail_is_binding = true;
        macro_rules! finalize {
            () => {
                if let Some(entry) = current.take() {
                    match entry {
                        Cur::Bind { name, lines } => {
                            if current_is_private {
                                privates.push(Priv::Bind {
                                    name,
                                    expr_lines: lines,
                                });
                            } else {
                                entries.push((name, lines));
                            }
                        }
                        // `export data …` — the verbatim declaration joins
                        // the do-block prelude; each bound name joins the
                        // record as a `name: name` projection (mirrors
                        // `loader.hnl`'s `flush` / `PrepItem::Decl`).
                        Cur::Decl { lines, exports } => {
                            privates.push(Priv::Decl { lines });
                            for n in exports {
                                entries.push((n.clone(), vec![n]));
                            }
                        }
                    }
                }
            };
        }
        for seg in top_level_segments(src) {
            let trimmed = seg.trim();
            if trimmed.is_empty() {
                continue;
            }
            if is_import_line(trimmed) {
                finalize!();
                current_is_private = false;
                if let Some((_, names)) = from_export_to_import(trimmed) {
                    for (name, alias) in names {
                        entries.push((name, vec![alias]));
                    }
                }
                continue;
            }
            if let Some(rest) = export_decl_rest(trimmed) {
                finalize!();
                current_is_private = false;
                // `export data F := C0 | …` / `export data F := { … }` — a
                // family declaration, not a `name := expr` binding: keep
                // the decl verbatim and export the names it binds
                // (mirrors `loader.hnl` / `module_prep`'s `PrepItem::Decl`).
                if is_data_decl_rest(rest) {
                    let Some(exports) = data_decl_bound_names(rest) else {
                        panic!(
                            "module prep: cannot read the family/constructor \
                             names of `export data` declaration: {trimmed}"
                        );
                    };
                    current = Some(Cur::Decl {
                        lines: vec![rest.trim().to_string()],
                        exports,
                    });
                    tail_is_binding = true;
                    continue;
                }
                let Some((sep_pos, sep_len)) = find_binding_sep(rest) else {
                    panic!(
                        "module prep: cannot transform `export` declaration \
                         (expected `export <name> <:=|: > …`): {trimmed}"
                    );
                };
                let name = strip_export_name(&rest[..sep_pos]);
                if name.is_empty() {
                    panic!("module prep: `export` declaration has no binding name: {trimmed}");
                }
                let rhs = rest[sep_pos + sep_len..].trim();
                let is_type_only = (sep_len == 1 && rest.as_bytes().get(sep_pos) == Some(&b':'))
                    || is_signature_rhs(rhs);
                let expr_start = if is_type_only { "null" } else { rhs };
                let mut expr_lines = Vec::new();
                if !expr_start.is_empty() {
                    expr_lines.push(expr_start.to_string());
                }
                current = Some(Cur::Bind {
                    name,
                    lines: expr_lines,
                });
                tail_is_binding = true;
                continue;
            }
            // Statement that is not an import or `export` declaration:
            // detect a *private* top-level binding so helpers survive into
            // the do-block instead of being dropped or swallowed into the
            // previous export's expression.
            if let Some((sep_pos, sep_len)) = find_binding_sep(trimmed) {
                let name = strip_export_name(&trimmed[..sep_pos]).trim().to_string();
                if is_valid_ident(&name) {
                    finalize!();
                    let is_type_only =
                        sep_len == 1 && trimmed.as_bytes().get(sep_pos) == Some(&b':');
                    let expr_start = if is_type_only {
                        "null"
                    } else {
                        trimmed[sep_pos + sep_len..].trim()
                    };
                    let mut expr_lines = Vec::new();
                    if !expr_start.is_empty() {
                        expr_lines.push(expr_start.to_string());
                    }
                    current = Some(Cur::Bind {
                        name,
                        lines: expr_lines,
                    });
                    current_is_private = true;
                    tail_is_binding = true;
                    continue;
                }
            }
            // A non-binding segment that cannot continue the in-flight
            // item is a new top-level statement. When it is the module's
            // last statement and is itself a bare expression (`do { … }`,
            // a record literal, call) its value is the module record —
            // no projection is appended. Segments that *do* continue join
            // the item's expression verbatim — mirrors `loader.hnl`'s
            // `segment_continues_item` gate.
            let continues = match &current {
                Some(item) => segment_continues_item(item.lines(), trimmed),
                None => false,
            };
            if continues {
                if let Some(item) = &mut current {
                    item.lines_mut().push(seg.clone());
                }
            } else {
                finalize!();
                current_is_private = false;
                tail_is_binding = false;
            }
        }
        finalize!();

        let imports = collect_import_lines(src);

        let mut prelude = String::new();
        for p in &privates {
            match p {
                Priv::Bind { name, expr_lines } => {
                    prelude.push_str(name);
                    prelude.push_str(" := ");
                    prelude.push_str(expr_lines.join("\n").trim());
                    prelude.push_str(";\n");
                }
                // `export data …` — the verbatim declaration statement;
                // the desugar pass expands it inside this `do` block.
                Priv::Decl { lines } => {
                    prelude.push_str(&lines.join("\n"));
                    prelude.push_str(";\n");
                }
            }
        }

        if entries.is_empty() {
            // No `export` declarations: the module record is *every*
            // top-level bound name — private bindings plus the locals
            // each hoisted import statement binds. The body is wrapped in
            // `do { … }` (imports first, verbatim statements next, the
            // record last) so the whole module evaluates as one
            // expression under the single-statement parser — mirrors
            // `loader.hnl` and `handl_runtime::module_prep`.
            if imports.is_empty() && privates.is_empty() {
                return src.to_string();
            }
            // The projection is appended only when the module's last
            // top-level statement is a binding; a trailing `do { … }`/
            // record literal is already the module's interface value.
            let mut fields: Vec<String> = Vec::new();
            if tail_is_binding {
                let push_field = |fields: &mut Vec<String>, name: &str| {
                    if is_valid_ident(name) && !fields.iter().any(|f| f == name) {
                        fields.push(name.to_string());
                    }
                };
                for p in &privates {
                    if let Priv::Bind { name, .. } = p {
                        push_field(&mut fields, name);
                    }
                }
                for seg in top_level_segments(src) {
                    let trimmed = seg.trim();
                    if is_import_line(trimmed) {
                        for local in import_bound_locals(trimmed) {
                            push_field(&mut fields, &local);
                        }
                    }
                }
            }
            let mut out = String::from("do {\n");
            for imp in &imports {
                out.push_str(imp);
                out.push('\n');
            }
            out.push_str(&drop_import_lines(src));
            if !fields.is_empty() {
                out.push('\n');
                out.push_str("{ ");
                out.push_str(
                    &fields
                        .iter()
                        .map(|f| format!("{f}: {f}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push_str(" }");
            }
            out.push('\n');
            out.push('}');
            return out;
        }

        let mut body = prelude;
        body.push_str("{ ");
        for (i, (name, expr_lines)) in entries.iter().enumerate() {
            if i > 0 {
                body.push_str(", ");
            }
            body.push_str(name);
            body.push_str(": ");
            body.push_str(expr_lines.join("\n").trim());
        }
        body.push_str(" }");

        if imports.is_empty() {
            if privates.is_empty() {
                body
            } else {
                format!("do {{\n{}\n}}", body)
            }
        } else {
            format!("do {{\n{}\n{}\n}}", imports.join("\n"), body)
        }
    }

    fn strip_export_name(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut depth: i32 = 0;
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b'<' | b':' if depth == 0 => {
                    return s[..i].trim().to_string();
                }
                _ => {}
            }
            i += 1;
        }
        s.trim().to_string()
    }

    fn find_binding_sep(s: &str) -> Option<(usize, usize)> {
        // `:=` is the ONLY binding separator (`=`/`==` are equality, `::`
        // is gone). A lone `:` is the type-only `export x : T` separator.
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut depth: i32 = 0;
        let mut first_colon: Option<usize> = None;
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b':' if depth == 0 => {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                        return Some((i, 2));
                    }
                    if first_colon.is_none() {
                        first_colon = Some(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        first_colon.map(|pos| (pos, 1))
    }

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

    pub fn parse_cap_declarations(src: &str) -> Vec<String> {
        let mut caps = Vec::new();
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            let rest = match trimmed.strip_prefix("cap") {
                Some(rest)
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') =>
                {
                    rest.trim_start()
                }
                _ => continue,
            };
            if let Some(name) = extract_quoted(rest) {
                caps.push(name.0);
            }
        }
        caps
    }

    pub fn parse_tag_declarations(src: &str) -> Vec<(String, String)> {
        let mut tags = Vec::new();
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            let rest = match trimmed.strip_prefix("tag") {
                Some(rest)
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') =>
                {
                    rest
                }
                _ => continue,
            };
            let sep = rest
                .find(":=")
                .map(|i| (i, 2))
                .or_else(|| rest.find('=').map(|i| (i, 1)));
            if let Some((eq_idx, sep_len)) = sep {
                let name = rest[..eq_idx].trim().to_string();
                let payload_type = rest[eq_idx + sep_len..].trim().to_string();
                if !name.is_empty()
                    && !payload_type.is_empty()
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    tags.push((name, payload_type));
                }
            }
        }
        tags
    }

    pub fn expand_tag_declarations(src: &str) -> String {
        let tags = parse_tag_declarations(src);
        if tags.is_empty() {
            return src.to_string();
        }
        let remaining: String = src
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("--") {
                    return true;
                }
                if let Some(rest) = trimmed.strip_prefix("tag") {
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                        let sep_idx = rest.find(":=").or_else(|| rest.find('='));
                        if let Some(eq_idx) = sep_idx {
                            let name = rest[..eq_idx].trim();
                            if !name.is_empty()
                                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                            {
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut out = String::from("do {\n");
        for (name, _payload_type) in &tags {
            out.push_str(&format!(
                "  {name} := _0 ~> hash_map.set({{value: _0}}, \"__tag__\", \"{name}\")\n"
            ));
        }
        for line in remaining.lines() {
            if line.trim().is_empty() {
                out.push('\n');
            } else {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push('}');
        out
    }
}

/// Resolver providing the `char` floor for `str-handl`.
struct CharResolver;

impl ModuleResolver for CharResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        match name {
            "char" => Some(Arc::new(CharModule)),
            _ => None,
        }
    }
}

/// Build and install the handl loader over the `char`/`str` floors.
///
/// `LOADER` is a `OnceLock` — first install wins — so once a loader is
/// present there is nothing to do. Rebuilding `str-handl` anyway would
/// run `analyze_source` per file *through the installed loader*
/// (interpreted `loader.analyze` over ~46 modules): an eval-cost
/// explosion under parallel test load that has flaked this binary
/// (resource kill, not a correctness bug — the eval path is
/// thread-local/`OnceLock` only).
fn install() {
    if loader().is_some() {
        return;
    }
    let str_module = build_str_handl_module(&CharResolver);
    let _ = install_loader(HandlLoader::new(vec![
        Arc::new(CharModule),
        Arc::new(str_module),
    ]));
}

/// Representative `.hnl` module sources covering every transform.
const CORPUS: &[&str] = &[
    // Bare record with header comments.
    "--- header\n--- more\n\n{ f: x ~> x }",
    // cap/op/handl declarations with continuation lines.
    "cap \"fs.read\"\ncap 'fs.write'\nop \"fs.read\" : Path -> Bytes\n  guarantee: True\n  requires: x\nhandl \"fs.impl\" := y ~> y\n  termination: total\n{ f: x ~> x }",
    // import forms.
    "from \"char\" import * as char\nfrom 'str' import * as 'str'\nimport \"math\" as m\nimport \"a/b/c\"\nimport bad\n{ f: x ~> x }",
    // named imports (bare name and `as` alias) plus star-without-as.
    "from \"len\" import len\nfrom \"reverse\" import reverse as rev\nexport a := x ~> len(x)",
    // export bindings (`:=`, multi-line bodies, trailing record
    // literal continues the last export's expression).
    "export a := x ~> x\nexport b := [1, 2]\nexport c := x ~> do {\n  y := x + 1\n  y\n}\n{ f: 1 }",
    // Segment-aware boundaries: an `export` is recognized wherever a
    // statement can begin — sharing a line with an import, after `;`,
    // indented, or as the whole module on one line.
    "from \"char\" import * as char export f: F := (x) ~> do { if x > 0 { 1 } else { 0 } }",
    "export inc := (x) ~> x + 1; export dec := (x) ~> x - 1",
    "  export indented := (x) ~> x * 2",
    "helper := (x) ~> x + 1 export apply := (x) ~> helper(x)",
    "export sign: Sign := (x) ~> do { if x > 0 { 1.0 } else { if x < 0 { -1.0 } else { 0.0 } } }",
    // `.`-dotted names (`m.from`, `char.from_chars`) and keywords inside
    // strings never split a statement.
    "export f := char.from_chars\nexport g := m.from\nexport h := \"import x\"",
    // tag declarations (`:=` is the separator; a stray `=` still splits
    // for tolerance).
    "tag ok := Value\ntag err := Error\n{ ok: ok, err: err }",
    // tag lines that are NOT declarations.
    "tag1 x := 1\ntags := t ~> t\n{ f: 1 }",
    // exports mixed with comments and blanks.
    "-- comment\n\nexport dedent := (text: String) ~> {\n  trim(text)\n}\n\n-- tail\nexport other := a >= b && a != b",
    // binding-separator edge cases (`:=` is the only binding operator).
    "export f := x ~> if a == b { x } else { x }\nexport g := a => b\nexport h := r ~> match r { []: 0, [h, ...t]: h }",
    // indented cap continuation variants and non-declaration keywords.
    "cap\nop\nhandl\nimport\nfrom\ncapx \"no\"\noperator := x ~> x\n{ f: 1 }",
    // `from ... export ...` re-export shorthand: single name, multiple
    // names, single-quoted spec, a `from` line whose tail is NOT `export`,
    // and a `from`-export line interleaved with a plain export.
    "from \"./upper.hnl\" export upper\nfrom \"./pair.hnl\" export a, b\nfrom './q.hnl' export q\nfrom \"char\" import * as char\nexport z := x ~> x\nfrom \"./w.hnl\" export w",
    // Private top-level bindings before, between, and after exports —
    // hoisted into the do-block so exported closures can see them.
    "helper := x ~> x + 1\nconst := 3\nexport a := x ~> helper(x)\nmiddle := y ~> y * const\nexport b := x ~> middle(x)",
    // Private bindings with no exports at all — wrapped in `do { … }` with
    // a trailing record projecting every bound name.
    "helper := x ~> x + 1\nother := helper(1)",
    // Private multi-line arrow between exports.
    "export a := x ~> helper(x)\nhelper := x ~> do {\n  y := x + 1\n  y\n}\nexport b := x ~> helper(x)",
    // Graded-arrow signature exports erase to `null`; n-cell *value*
    // exports (`f ~[2]~> g`, string-law domains) stay. A grade-1 arrow
    // over a capitalized type head is still a signature.
    "export Sig := (Int) ~[1]~> Result<Int, String>\nexport BareSig := Int ~[1]~> Int\nexport f := (x) ~> x\nexport alpha := f ~[2]~> f\nexport law := \"a >>> b\" ~[2]~> \"id\"",
    // `export data` family declarations — both the `:= C0 | …` constructor-arm
    // and `:= { … }` record forms: the decl stays verbatim for desugar
    // and the family/ctor names project into the export record.
    "export data Nat := Zero | Succ(n : Nat)\n",
    "export data Nat := { Zero : Nat, Succ : (pred : Nat) ~> Nat }\n",
    "export data Pair<A> := Fst(x : A) | Snd(y : A)\nexport data List := { Nil : List, Cons : (head : Int, tail : List) ~> List }\n",
    // `data`-prefixed identifiers that are NOT declarations.
    "export data_transfer := 5\nexport data := 6\n",
    // `export data` mixed with ordinary exports and a private helper.
    "helper := n ~> n + 1\nexport data Nat := Zero | Succ(n : Nat)\nexport bump := n ~> helper(n)\n",
    // NOTE: whole-package sources (e.g. str-handl.hnl) are excluded: the
    // interpreted transform over ~25k chars is far too slow for a test —
    // see AGENTS.md "loader self-hosting" for the performance note.
];

/// Malformed `export` declarations must fail loudly on *both* sides of
/// the seam: `loader.hnl` raises via `error.raise` (the loader call
/// errors), and the bootstrap fallback panics — which is also what the
/// public `transform_export_to_object` does once the loader call fails.
/// Never a silent drop into `{}`.
#[test]
fn separatorless_export_fails_loudly() {
    ensure_lang();
    install();

    // The handl loader itself errors (eval failure via `error.raise`).
    let err = loader()
        .expect("loader installed")
        .call("transform_export_to_object", "export just_a_name\n")
        .expect_err("loader must fail on a separator-less export");
    let msg = format!("{err}");
    assert!(
        !msg.is_empty(),
        "loader-side failure should carry a message, got: {msg}"
    );

    // The bootstrap mirror panics with the diagnostic.
    let caught =
        std::panic::catch_unwind(|| bootstrap::transform_export_to_object("export just_a_name\n"));
    let panic_msg = caught.expect_err("bootstrap must panic on separator-less export");
    let text = panic_msg
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic_msg.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        text.contains("export") && text.contains("cannot transform"),
        "panic should name the export declaration, got: {text}"
    );

    // And the public entry point — loader error falls back to the
    // bootstrap panic — is loud too.
    let caught = std::panic::catch_unwind(|| transform_export_to_object("export just_a_name\n"));
    assert!(caught.is_err(), "public transform must fail loudly");
}

/// An `export` declaration with a separator but no binding name is loud.
#[test]
fn nameless_export_fails_loudly() {
    ensure_lang();
    install();
    assert!(
        loader()
            .expect("loader installed")
            .call("transform_export_to_object", "export := 1\n")
            .is_err(),
        "loader must fail on a nameless export"
    );
    let caught =
        std::panic::catch_unwind(|| bootstrap::transform_export_to_object("export := 1\n"));
    let panic_msg = caught.expect_err("bootstrap must panic on a nameless export");
    let text = panic_msg
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic_msg.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        text.contains("no binding name"),
        "panic should name the missing binding name, got: {text}"
    );
}

/// `export data F := { C : R, … }` / `export data F :: C0 | C1(…)` —
/// the prep keeps the declaration verbatim and projects the family
/// plus each constructor name (mirrors
/// `handl_runtime::module_prep`). Exercised through the bootstrap path
/// (`with_loader_disabled`) and through the installed `.hnl` loader —
/// both must produce the decl plus the `Nat: Nat`/`Zero: Zero`/
/// `Succ: Succ` projections.
#[test]
fn export_data_decl_transforms() {
    ensure_lang();
    for src in [
        "export data Nat := { Zero : Nat, Succ : (pred : Nat) ~> Nat }\n",
        "export data Nat := Zero | Succ(n : Nat)\n",
    ] {
        let out = handl_plugin_lib_handl::with_loader_disabled(|| transform_export_to_object(src));
        assert!(
            out.contains("data Nat"),
            "decl must survive verbatim, got:\n{out}"
        );
        for name in ["Nat: Nat", "Zero: Zero", "Succ: Succ"] {
            assert!(
                out.contains(name),
                "record must project `{name}`, got:\n{out}"
            );
        }
    }
    // `export data_transfer := …` is an ordinary binding, not a decl.
    let out = handl_plugin_lib_handl::with_loader_disabled(|| {
        transform_export_to_object("export data_transfer := 5\n")
    });
    assert!(
        out.contains("data_transfer: data_transfer"),
        "identifier starting with `data` must stay a binding, got:\n{out}"
    );

    // The installed `.hnl` loader produces the same observable surface.
    install();
    for src in [
        "export data Nat := { Zero : Nat, Succ : (pred : Nat) ~> Nat }\n",
        "export data Nat := Zero | Succ(n : Nat)\n",
    ] {
        let out = transform_export_to_object(src);
        assert!(
            out.contains("data Nat"),
            "loader: decl must survive verbatim, got:\n{out}"
        );
        for name in ["Nat: Nat", "Zero: Zero", "Succ: Succ"] {
            assert!(
                out.contains(name),
                "loader: record must project `{name}`, got:\n{out}"
            );
        }
        // …and textually matches the bootstrap mirror.
        assert_eq!(
            out,
            bootstrap::transform_export_to_object(src),
            "loader/bootstrap parity for {src:?}"
        );
    }
}

/// End-to-end: a `.hnl` module whose exports share a line with an import
/// statement (or each other) binds and applies correctly through
/// `HandlModule` — the regression is a silently-`{}` module record.
#[test]
fn single_line_module_exports_evaluate() {
    ensure_lang();
    install();

    // `export` sharing a line with an import statement: the `from` tail
    // suppresses the split at `import`, then `export` opens a new
    // statement mid-line.
    let src = "from \"char\" import * as char export sign := (x) ~> if x > 0 { 1 } else { -1 }\n";
    let build = || {
        HandlModule::new("m", src, &["sign"])
            .with_dependency(CharModule)
            .function("sign")
            .expect("sign export bound")
            .call(&[serde_json::json!(-2)], &mut EvalContext::new())
            .expect("sign(-2) evaluates")
    };
    assert_eq!(build(), serde_json::json!(-1), "sign(-2) via loader path");
    // Same source through the bootstrap path (loader disabled).
    let via_bootstrap = handl_plugin_lib_handl::with_loader_disabled(|| {
        HandlModule::new("m2", src, &["sign"])
            .with_dependency(CharModule)
            .function("sign")
            .expect("sign export bound (bootstrap)")
            .call(&[serde_json::json!(-2)], &mut EvalContext::new())
            .expect("sign(-2) evaluates (bootstrap)")
    });
    assert_eq!(
        via_bootstrap,
        serde_json::json!(-1),
        "sign(-2) via bootstrap path"
    );

    // `;`-separated exports on one line, no deps.
    let src2 = "export inc := (x) ~> x + 1; export dec := (x) ~> x - 1\n";
    let m = HandlModule::new("two", src2, &["inc", "dec"]);
    let mut ctx = EvalContext::new();
    let inc = m
        .function("inc")
        .expect("inc bound")
        .call(&[serde_json::json!(4)], &mut ctx)
        .expect("inc(4)");
    let dec = m
        .function("dec")
        .expect("dec bound")
        .call(&[serde_json::json!(5)], &mut ctx)
        .expect("dec(5)");
    assert_eq!((inc, dec), (serde_json::json!(5), serde_json::json!(4)));

    // An indented export is a top-level declaration, not a continuation.
    let src3 = "  export double := (x) ~> x * 2\n";
    let m = HandlModule::new("ind", src3, &["double"]);
    let v = m
        .function("double")
        .expect("double bound")
        .call(&[serde_json::json!(3)], &mut ctx)
        .expect("double(3)");
    assert_eq!(v, serde_json::json!(6));
}

#[test]
fn loader_parity() {
    ensure_lang();
    install();
    for (i, src) in CORPUS.iter().enumerate() {
        assert_eq!(
            strip_comment_lines(src),
            bootstrap::strip_comment_lines(src),
            "strip_comments corpus[{i}]"
        );
        assert_eq!(
            strip_declaration_lines(src),
            bootstrap::strip_declaration_lines(src),
            "strip_decls corpus[{i}]"
        );
        assert_eq!(
            transform_export_to_object(src),
            bootstrap::transform_export_to_object(src),
            "transform_export_to_object corpus[{i}]"
        );
        assert_eq!(
            parse_cap_declarations(src),
            bootstrap::parse_cap_declarations(src),
            "parse_caps corpus[{i}]"
        );
        assert_eq!(
            parse_tag_declarations(src),
            bootstrap::parse_tag_declarations(src),
            "parse_tags corpus[{i}]"
        );
        assert_eq!(
            expand_tag_declarations(src),
            bootstrap::expand_tag_declarations(src),
            "expand_tags corpus[{i}]"
        );
    }
}
