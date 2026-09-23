//! Man-page doc lookup for C symbols — local page files, the `man`
//! viewer, and the man7.org mirror.

use flate2::read::GzDecoder;
use handl_plugin_sdkgen::{http_get, DocSource};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Best-effort doc lookup for C symbols. Tries, in order: local man page
/// files under `/usr/share/man` (installed by `man-pages`/`manpages-dev`
/// and library packages — no `man` viewer needed), the `man` command
/// itself (resolves whatis aliases), then the man7.org mirror over HTTP
/// (works in CI/containers). Sections 2 (system calls) and 3 (library
/// functions) are searched; the NAME-section description is extracted
/// and cached per symbol.
pub struct ManPageDocSource {
    cache: RefCell<HashMap<String, Option<String>>>,
}

impl ManPageDocSource {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
        }
    }

    fn lookup_uncached(&self, key: &str) -> Option<String> {
        for section in ["2", "3"] {
            if let Some(doc) = page_file_doc(key, section) {
                return Some(doc);
            }
        }
        for section in ["2", "3"] {
            if let Some(text) = run_man(section, key) {
                if let Some(desc) = extract_name_section(&text) {
                    return Some(desc);
                }
            }
        }
        for section in ["2", "3"] {
            if let Some(doc) = self.man7_doc(key, section) {
                return Some(doc);
            }
        }
        None
    }

    /// Fetch `https://man7.org/linux/man-pages/manS/sym.S.html` and pull
    /// the NAME-section description out of the HTML.
    fn man7_doc(&self, key: &str, section: &str) -> Option<String> {
        let url = format!("https://man7.org/linux/man-pages/man{section}/{key}.{section}.html");
        let body = http_get(&url, 10_000).ok()?;
        extract_html_name(&body, key)
    }
}

impl Default for ManPageDocSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DocSource for ManPageDocSource {
    fn lookup(&self, key: &str) -> Option<String> {
        if let Some(cached) = self.cache.borrow().get(key) {
            return cached.clone();
        }
        let result = self.lookup_uncached(key);
        self.cache
            .borrow_mut()
            .insert(key.to_string(), result.clone());
        result
    }
}

/// Read the man page for `symbol` from the page files themselves —
/// `manS/sym.S[.gz]` under each root — without needing the `man`
/// viewer. Follows `.so` alias links one hop.
fn page_file_doc(symbol: &str, section: &str) -> Option<String> {
    page_file_doc_in(&["/usr/share/man", "/usr/local/share/man"], symbol, section)
}

pub(crate) fn page_file_doc_in(roots: &[&str], symbol: &str, section: &str) -> Option<String> {
    for root in roots {
        let root = Path::new(root);
        let dir = root.join(format!("man{section}"));
        for path in page_candidates(&dir, symbol, section) {
            let Some(text) = read_page(&path) else {
                continue;
            };
            if let Some(doc) = extract_groff_name(&text, symbol) {
                return Some(doc);
            }
            // `.so manS/other.S` alias page — resolve one hop into the
            // same root (e.g. `creat.3` → `man3/open.3`).
            if let Some(rel) = text.trim_start().strip_prefix(".so ") {
                let rel = rel.trim();
                for cand in [root.join(format!("{rel}.gz")), root.join(rel)] {
                    if let Some(t2) = read_page(&cand) {
                        if let Some(doc) = extract_groff_name(&t2, symbol) {
                            return Some(doc);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Candidate page paths for `symbol` in `dir`: `sym.S.gz`, `sym.S`, then
/// subsection pages (`sym.3p.gz`, `sym.3type.gz`, …) in sorted order.
fn page_candidates(dir: &Path, symbol: &str, section: &str) -> Vec<PathBuf> {
    let mut out = vec![
        dir.join(format!("{symbol}.{section}.gz")),
        dir.join(format!("{symbol}.{section}")),
    ];
    let prefix = format!("{symbol}.{section}");
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut subs: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    let base = n.strip_suffix(".gz").unwrap_or(n);
                    base.strip_prefix(&prefix)
                        .is_some_and(|rest| !rest.is_empty())
                })
            })
            .collect();
        subs.sort();
        out.extend(subs);
    }
    out
}

/// Read a man page — gunzipping `.gz` files, plain reads otherwise.
fn read_page(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if path.extension().is_some_and(|e| e == "gz") {
        let mut text = String::new();
        GzDecoder::new(bytes.as_slice())
            .read_to_string(&mut text)
            .ok()?;
        Some(text)
    } else {
        String::from_utf8(bytes).ok()
    }
}

fn run_man(section: &str, symbol: &str) -> Option<String> {
    let output = std::process::Command::new("man")
        .args(["-P", "cat", "-S", section, symbol])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Extract the description from the NAME section of rendered man output:
/// ```text
/// NAME
///        symbol - description text
/// ```
fn extract_name_section(text: &str) -> Option<String> {
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim() == "NAME" {
            if let Some(name_line) = lines.next() {
                let trimmed = name_line.trim();
                if let Some(pos) = trimmed.find(" - ") {
                    let desc = trimmed[pos + 3..].trim();
                    if !desc.is_empty() {
                        return Some(desc.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Extract the description from the NAME section of raw man page source
/// (groff): `.SH NAME` then `name[, …] \- description`. The symbol must
/// appear in the names list so `.so` chains and subsection pages cannot
/// emit a mismatched doc.
pub(crate) fn extract_groff_name(text: &str, symbol: &str) -> Option<String> {
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let l = line.trim();
        if l == ".SH NAME" || l == ".SH \"NAME\"" {
            let mut body = String::new();
            for l in lines.by_ref() {
                let t = l.trim();
                if t.starts_with(".SH") || t.starts_with(".SS") {
                    break;
                }
                body.push_str(t);
                body.push(' ');
            }
            return groff_desc(&body, symbol);
        }
    }
    None
}

/// `name[, name2, …] \- description` — the dash may be a groff `\-` or
/// a plain ` - `.
fn groff_desc(body: &str, symbol: &str) -> Option<String> {
    let (names, desc) = body.split_once("\\-").or_else(|| body.split_once(" - "))?;
    if !names.split(',').any(|n| strip_groff(n.trim()) == symbol) {
        return None;
    }
    let desc = strip_groff(desc.trim());
    (!desc.is_empty()).then_some(desc)
}

/// Remove common groff markup: font escapes (`\fB`), `\-`, `\e`,
/// two-char escapes (`\(xx`), and leading `.B`/`.I` macro words.
fn strip_groff(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('f') => {
                chars.next();
            }
            Some('-') => out.push('-'),
            Some('e') => out.push('\\'),
            Some('(') => {
                chars.next();
                chars.next();
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out.trim().to_string()
}

/// Extract the NAME-section description from a man7.org HTML page:
/// the `<pre>` block right after the `id="NAME"` heading holds
/// `symbol - description`.
pub(crate) fn extract_html_name(html: &str, symbol: &str) -> Option<String> {
    let name_idx = html.find("id=\"NAME\"")?;
    let pre_start = html[name_idx..].find("<pre>")? + name_idx + 5;
    let pre_end = html[pre_start..].find("</pre>")? + pre_start;
    let text = strip_html(&html[pre_start..pre_end]);
    for line in text.lines() {
        if let Some((names, desc)) = line.trim().split_once(" - ") {
            if names.split(',').any(|n| n.trim() == symbol) && !desc.trim().is_empty() {
                return Some(desc.trim().to_string());
            }
        }
    }
    None
}

/// Remove HTML tags and decode the handful of entities man pages
/// produce. `&amp;` decodes last so `&amp;lt;` stays `&lt;`.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        match rest.find('>') {
            Some(j) => rest = &rest[j + 1..],
            None => break,
        }
    }
    out.push_str(rest);
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

