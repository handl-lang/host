//! Text preparation for canonical `.hnl` modules loaded by the runtime.
//!
//! This is a lightweight, line-based mirror of the export-to-object rewrite
//! used by `handl_plugin_lib_handl` so that `ModuleLoader` can evaluate
//! multi-export package modules without depending on the higher-level
//! `handl_plugin_lib_handl` crate (which itself depends on `handl_runtime`).

/// Prepare a module source for evaluation: strip comments and declarations,
/// then rewrite top-level `export` declarations into a single object literal.
///
/// `is_file_spec` reports whether an import specifier resolves to a file —
/// those are the executable import op (ADR-0096) and must survive prep.
/// Specifiers naming host modules (`from "str" import …`, `import <name>`)
/// are decorative declarations and are dropped.
pub fn prepare_module(src: &str, is_file_spec: &dyn Fn(&str) -> bool) -> String {
    let src = strip_comment_lines(src);
    let src = strip_declaration_lines(&src);
    transform_export_to_object(&src, is_file_spec)
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
fn transform_export_to_object(src: &str, is_file_spec: &dyn Fn(&str) -> bool) -> String {
    // Each top-level binding is collected as `(name, expr_lines)`.
    // Non-private entries become object fields (`{ name: expr }`); private
    // entries become `do`-block assignments (`name := expr;`) emitted *before*
    // the object so exported closures can reference private helpers.
    let mut entries: Vec<(String, Vec<String>)> = Vec::new();
    let mut privates: Vec<(String, Vec<String>)> = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    let mut current_is_private: bool = false;

    // Finalize `current` into the right collection.
    macro_rules! finalize {
        () => {
            if let Some(entry) = current.take() {
                if current_is_private {
                    privates.push(entry);
                } else {
                    entries.push(entry);
                }
                current_is_private = false;
            }
        };
    }

    for line in src.lines() {
        let trimmed = line.trim_start();
        if is_top_import_line(line) {
            finalize!();
            // Only file-resolving specs can re-export; module-name specs
            // name host modules and are decorative (ADR-0096).
            let file_spec =
                import_line_spec(line).map_or(false, |s| is_file_spec(&s));
            if file_spec {
                if let Some((_, names)) = from_export_to_import(line) {
                    for (name, alias) in names {
                        entries.push((name, vec![alias]));
                    }
                }
            }
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.starts_with("export ") {
            finalize!();
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
                current_is_private = false;
            }
            continue;
        } else if !line.starts_with(' ') && !line.starts_with('\t') {
            // Column-0 line that is not an import or `export` declaration.
            // Detect a *private* (non-exported) top-level binding so that
            // helper functions are preserved instead of dropped.
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
                    continue;
                }
            }
        }
        if let Some((_, lines)) = &mut current {
            if !line.trim().is_empty() {
                lines.push(line.to_string());
            }
        }
    }
    finalize!();

    let imports = collect_executable_import_lines(src, is_file_spec);

    // If there are no exports at all, preserve the original script/test
    // behavior: private bindings stay verbatim, decorative import
    // declarations are dropped — but executable path imports
    // (`import "f.hnl"`, `from "./f" import …`) are kept so nested imports
    // still run (ADR-0096).
    if entries.is_empty() {
        return drop_non_file_import_lines(src, is_file_spec);
    }

    // Build the body: private bindings as do-block assignments, then the
    // export object literal.
    let mut body = String::new();
    for (name, expr_lines) in &privates {
        let expr = expr_lines.join("\n");
        let expr = expr.trim();
        body.push_str(name);
        body.push_str(" := ");
        body.push_str(expr);
        body.push_str(";\n");
    }
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
        body
    } else {
        format!("do {{\n{}\n{}\n}}", imports.join("\n"), body)
    }
}

/// True when `s` is a plausible Handl identifier: starts with an
/// alphabetic character or underscore, followed by zero or more
/// alphanumeric characters or underscores.
fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => chars.all(|c| c.is_alphanumeric() || c == '_'),
        _ => false,
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

/// Top-level import lines whose specifier resolves to a file — the
/// executable import op (ADR-0096). Decorative declarations
/// (`import <name>`, module-name specs like `from "str" import …`) are
/// excluded: the host module chain supplies those bindings.
fn collect_executable_import_lines(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    src.lines()
        .filter(|line| is_top_import_line(line))
        .filter(|line| import_line_spec(line).map_or(false, |s| is_file_spec(&s)))
        .map(|line| {
            from_export_to_import(line)
                .map(|(import_line, _)| import_line)
                .unwrap_or_else(|| line.to_string())
        })
        .collect()
}

/// Drop top-level import declarations that do not resolve to a file.
/// Executable path imports are kept so they still run at eval time.
fn drop_non_file_import_lines(src: &str, is_file_spec: &dyn Fn(&str) -> bool) -> String {
    src.lines()
        .filter(|line| {
            !is_top_import_line(line)
                || import_line_spec(line).map_or(false, |s| is_file_spec(&s))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract the quoted specifier from `import "<spec>"` or
/// `from "<spec>" …`. Non-quoted forms (`import <name>`, bare `import`)
/// return `None`.
fn import_line_spec(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("import")
        .or_else(|| trimmed.strip_prefix("from"))?;
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let after = &rest[quote.len_utf8()..];
    let end = after.find(quote)?;
    Some(after[..end].to_string())
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
