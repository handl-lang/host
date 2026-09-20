//! Parity test: the self-hosted handl loader (`handl-packages/loader/loader.hnl`)
//! must produce identical results to the Rust bootstrap transforms.
//!
//! The loader needs the `char`/`str` floors: `char` is native
//! (`plugin-lib-char-native`) and `str` is `str-handl`, itself prepared by
//! the Rust bootstrap path (the bootstrap paradox — ADR-0094).

use std::sync::Arc;

use handl_platform::Module;
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{
    HandlLoader, ModuleResolver, build_str_handl_module, expand_tag_declarations, install_loader,
    parse_cap_declarations, parse_tag_declarations, strip_comment_lines, strip_declaration_lines,
    transform_export_to_object,
};

/// Bootstrap implementations, re-exposed for parity checking.
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

    fn is_top_import_line(line: &str) -> bool {
        !line.starts_with(' ') && !line.starts_with('\t') && is_import_line(line.trim_start())
    }

    fn collect_import_lines(src: &str) -> Vec<String> {
        src.lines()
            .filter(|line| is_top_import_line(line))
            .map(|line| {
                from_export_to_import(line)
                    .map(|(import_line, _)| import_line)
                    .unwrap_or_else(|| line.to_string())
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
        src.lines()
            .filter(|line| !is_top_import_line(line))
            .collect::<Vec<_>>()
            .join("\n")
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

    pub fn transform_export_to_object(src: &str) -> String {
        let mut entries: Vec<(String, Vec<String>)> = Vec::new();
        let mut current: Option<(String, Vec<String>)> = None;
        for line in src.lines() {
            let trimmed = line.trim_start();
            if is_top_import_line(line) {
                if let Some((_, names)) = from_export_to_import(line) {
                    if let Some(entry) = current.take() {
                        entries.push(entry);
                    }
                    for (name, alias) in names {
                        entries.push((name, vec![alias]));
                    }
                }
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.starts_with("export ") {
                if let Some(entry) = current.take() {
                    entries.push(entry);
                }
                let rest = &trimmed["export ".len()..];
                if let Some((sep_pos, sep_len)) = find_binding_sep(rest) {
                    let name = rest[..sep_pos].trim().to_string();
                    let expr_start = rest[sep_pos + sep_len..].trim();
                    let mut expr_lines = Vec::new();
                    if !expr_start.is_empty() {
                        expr_lines.push(expr_start.to_string());
                    }
                    current = Some((name, expr_lines));
                }
            } else if let Some((_, ref mut lines)) = current {
                if !line.trim().is_empty() {
                    lines.push(line.to_string());
                }
            }
        }
        if let Some(entry) = current.take() {
            entries.push(entry);
        }

        let imports = collect_import_lines(src);

        let body = if entries.is_empty() {
            if imports.is_empty() {
                return src.to_string();
            }
            drop_import_lines(src)
        } else {
            let mut out = String::from("{ ");
            for (i, (name, expr_lines)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(name);
                out.push_str(": ");
                out.push_str(expr_lines.join("\n").trim());
            }
            out.push_str(" }");
            out
        };

        if imports.is_empty() {
            body
        } else {
            format!("do {{\n{}\n{}\n}}", imports.join("\n"), body)
        }
    }

    fn find_binding_sep(s: &str) -> Option<(usize, usize)> {
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut depth: i32 = 0;
        let mut first_eq: Option<usize> = None;
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b':' if depth == 0 => {
                    if i + 1 < bytes.len() && bytes[i + 1] == b':' {
                        return Some((i, 2));
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
        first_eq.map(|pos| (pos, 1))
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
                "  {name} :: _0 ~> tag.tag(\"{name}\", {{value: _0}})\n"
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
    // export bindings (::, legacy =, multi-line bodies, skipped bad export).
    "export a :: x ~> x\nexport b = [1, 2]\nexport c :: x ~> do {\n  y :: x + 1\n  y\n}\nexport d\n{ f: 1 }",
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
    // NOTE: whole-package sources (e.g. str-handl.hnl) are excluded: the
    // interpreted transform over ~25k chars is far too slow for a test —
    // see AGENTS.md "loader self-hosting" for the performance note.
];

#[test]
fn loader_parity() {
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
