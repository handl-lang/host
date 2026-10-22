//! Parity test: the self-hosted handl loader (`handl-packages/loader/loader.hnl`)
//! must produce identical results to the Rust bootstrap transforms.
//!
//! The loader needs the `char`/`str` floors: `char` is native
//! (`plugin-lib-char-native`) and `str` is `str-handl`, itself prepared by
//! the Rust bootstrap path (the bootstrap paradox — ADR-0094).

use std::sync::Arc;

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_char_native::CharModule;
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
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
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
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
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
        let mut entries: Vec<(String, Vec<String>)> = Vec::new();
        let mut privates: Vec<(String, Vec<String>)> = Vec::new();
        let mut current: Option<(String, Vec<String>)> = None;
        let mut current_is_private: bool = false;
        // Whether the module's last top-level statement is a binding. When
        // it is not (a trailing `do { … }` or record literal) the body's
        // own value is the module record and no name projection is
        // appended — mirrors `loader.hnl`'s `tail` flag.
        let mut tail_is_binding = true;
        macro_rules! finalize {
            () => {
                if let Some(entry) = current.take() {
                    if current_is_private {
                        privates.push(entry);
                    } else {
                        entries.push(entry);
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
                let Some((sep_pos, sep_len)) = find_binding_sep(rest) else {
                    panic!(
                        "module prep: cannot transform `export` declaration \
                         (expected `export <name> <::|:=|=|: > …`): {trimmed}"
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
                current = Some((name, expr_lines));
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
                    current = Some((name, expr_lines));
                    current_is_private = true;
                    tail_is_binding = true;
                    continue;
                }
            }
            // A non-binding statement that does not continue an in-flight
            // binding means the module's last statement is a bare
            // expression — its value is the module record, so no
            // projection is appended. Otherwise the segment continues the
            // current binding's expression verbatim.
            if current.is_none() {
                tail_is_binding = false;
            } else if let Some((_, lines)) = &mut current {
                lines.push(seg.clone());
            }
        }
        finalize!();

        let imports = collect_import_lines(src);

        let mut prelude = String::new();
        for (name, expr_lines) in &privates {
            prelude.push_str(name);
            prelude.push_str(" := ");
            prelude.push_str(expr_lines.join("\n").trim());
            prelude.push_str(";\n");
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
                for (name, _) in &privates {
                    push_field(&mut fields, name);
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
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut depth: i32 = 0;
        let mut first_eq: Option<usize> = None;
        let mut first_colon: Option<usize> = None;
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b':' if depth == 0 => {
                    if i + 1 < bytes.len() && bytes[i + 1] == b':' {
                        return Some((i, 2));
                    }
                    if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                        return Some((i, 2));
                    }
                    if first_colon.is_none() {
                        first_colon = Some(i);
                    }
                }
                b'=' if depth == 0 => {
                    let prev = if i > 0 { bytes[i - 1] } else { 0 };
                    let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
                    if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>'
                    {
                        i += 1;
                        continue;
                    }
                    if first_eq.is_none() {
                        first_eq = Some(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if let Some(pos) = first_eq {
            return Some((pos, 1));
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
                .find("::")
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
                        let sep_idx = rest.find("::").or_else(|| rest.find('='));
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
                "  {name} :: _0 ~> hash_map.set({{value: _0}}, \"__tag__\", \"{name}\")\n"
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
fn install() {
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
    "cap \"fs.read\"\ncap 'fs.write'\nop \"fs.read\" : Path -> Bytes\n  guarantee: True\n  requires: x\nhandl \"fs.impl\" = y ~> y\n  termination: total\n{ f: x ~> x }",
    // import forms.
    "from \"char\" import * as char\nfrom 'str' import * as 'str'\nimport \"math\" as m\nimport \"a/b/c\"\nimport bad\n{ f: x ~> x }",
    // named imports (bare name and `as` alias) plus star-without-as.
    "from \"len\" import len\nfrom \"reverse\" import reverse as rev\nexport a :: x ~> len(x)",
    // export bindings (::, legacy =, multi-line bodies, trailing record
    // literal continues the last export's expression).
    "export a :: x ~> x\nexport b = [1, 2]\nexport c :: x ~> do {\n  y :: x + 1\n  y\n}\n{ f: 1 }",
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
    // tag declarations.
    "tag ok = Value\ntag err :: Error\n{ ok: ok, err: err }",
    // tag lines that are NOT declarations.
    "tag1 x = 1\ntags :: t ~> t\n{ f: 1 }",
    // exports mixed with comments and blanks.
    "-- comment\n\nexport dedent :: (text: String) ~> {\n  trim(text)\n}\n\n-- tail\nexport other = a >= b && a != b",
    // binding-separator edge cases.
    "export f :: x ~> if a == b { x } else { x }\nexport g = a => b\nexport h :: r ~> match r { []: 0, [h, ...t]: h }",
    // indented cap continuation variants and non-declaration keywords.
    "cap\nop\nhandl\nimport\nfrom\ncapx \"no\"\noperator :: x ~> x\n{ f: 1 }",
    // `from ... export ...` re-export shorthand: single name, multiple
    // names, single-quoted spec, a `from` line whose tail is NOT `export`,
    // and a `from`-export line interleaved with a plain export.
    "from \"./upper.hnl\" export upper\nfrom \"./pair.hnl\" export a, b\nfrom './q.hnl' export q\nfrom \"char\" import * as char\nexport z :: x ~> x\nfrom \"./w.hnl\" export w",
    // Private top-level bindings before, between, and after exports —
    // hoisted into the do-block so exported closures can see them.
    "helper :: x ~> x + 1\nconst := 3\nexport a :: x ~> helper(x)\nmiddle :: y ~> y * const\nexport b :: x ~> middle(x)",
    // Private bindings with no exports at all — wrapped in `do { … }` with
    // a trailing record projecting every bound name.
    "helper :: x ~> x + 1\nother := helper(1)",
    // Private multi-line arrow between exports.
    "export a :: x ~> helper(x)\nhelper :: x ~> do {\n  y :: x + 1\n  y\n}\nexport b :: x ~> helper(x)",
    // Graded-arrow signature exports erase to `null`; n-cell *value*
    // exports (`f ~[2]~> g`, string-law domains) stay. A grade-1 arrow
    // over a capitalized type head is still a signature.
    "export Sig := (Int) ~[1]~> Result<Int, String>\nexport BareSig := Int ~[1]~> Int\nexport f := (x) ~> x\nexport alpha := f ~[2]~> f\nexport law := \"a >>> b\" ~[2]~> \"id\"",
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
    let caught = std::panic::catch_unwind(|| {
        bootstrap::transform_export_to_object("export just_a_name\n")
    });
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
    let caught = std::panic::catch_unwind(|| {
        transform_export_to_object("export just_a_name\n")
    });
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
    let caught = std::panic::catch_unwind(|| {
        bootstrap::transform_export_to_object("export := 1\n")
    });
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
