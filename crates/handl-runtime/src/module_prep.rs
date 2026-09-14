//! Text preparation for canonical `.hnl` modules loaded by the runtime.
//!
//! This is a lightweight, line-based mirror of the export-to-object rewrite
//! used by `handl_plugin_lib_handl` so that `ModuleLoader` can evaluate
//! multi-export package modules without depending on the higher-level
//! `handl_plugin_lib_handl` crate (which itself depends on `handl_runtime`).

/// Prepare a module source for evaluation: strip comments and declarations,
/// then rewrite top-level `export` declarations into a single object literal.
pub(crate) fn prepare_module(src: &str) -> String {
    let src = strip_comment_lines(src);
    let src = strip_declaration_lines(&src);
    transform_export_to_object(&src)
}

/// Strip `--` comment lines from a `.hnl` source.
fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip `cap`/`op`/`handl` declaration lines and their indented
/// continuation lines from a `.hnl` source.
fn strip_declaration_lines(src: &str) -> String {
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
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Transform top-level `export name <::|:=|=> expr` declarations into an
/// object literal.  Type-only `export Name: Type` declarations are given the
/// value `null` so that type expressions are not evaluated at runtime.
fn transform_export_to_object(src: &str) -> String {
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
                let name = strip_export_name(&rest[..sep_pos]);
                let is_type_only = sep_len == 1 && rest.as_bytes().get(sep_pos) == Some(&b':');
                let expr_start = if is_type_only {
                    "null"
                } else {
                    rest[sep_pos + sep_len..].trim()
                };
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

fn is_top_import_line(line: &str) -> bool {
    !line.starts_with(' ') && !line.starts_with('\t') && is_import_line(line.trim_start())
}

/// If a `from` line uses the re-export form `from "..." export x, y`, rewrite
/// it to a plain import `from "..." import x as __x, y as __y` and return the
/// (name, local-alias) pairs that should appear in the module record.
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

/// Extract the first quoted path in a `from` line and the text that follows it.
fn extract_quoted_path(line: &str) -> Option<(String, &str)> {
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
                if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>' {
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
