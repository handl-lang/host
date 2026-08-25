use anyhow::{Context, Result};
use regex::Regex;
use rustdoc_types::{Crate, Id, Item, ItemEnum};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct Catalog {
    reference: Vec<Category>,
}

#[derive(Debug, Deserialize)]
struct Category {
    title: String,
    slug: String,
    #[serde(default)]
    description: Option<String>,
    include: Vec<Include>,
}

#[derive(Debug, Deserialize)]
struct Include {
    #[serde(rename = "crate")]
    krate: String,
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    reexports: bool,
    #[serde(default)]
    reexports_from: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FunctionDocs {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    params: Vec<ParamDoc>,
    #[serde(default)]
    return_value: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    examples: Vec<String>,
    #[serde(default)]
    see: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ParamDoc {
    name: String,
    #[serde(rename = "type")]
    param_type: String,
    #[serde(default)]
    description: Option<String>,
}

type FunctionDocsMap = BTreeMap<String, BTreeMap<String, FunctionDocs>>;

fn main() -> Result<()> {
    let catalog_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/reference.yaml".to_string());
    let out_dir = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "docs/reference".to_string());

    fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output directory {}", out_dir))?;

    let catalog_yaml = fs::read_to_string(&catalog_path)
        .with_context(|| format!("reading catalog {}", catalog_path))?;
    let catalog: Catalog = serde_yaml::from_str(&catalog_yaml)
        .with_context(|| format!("parsing catalog {}", catalog_path))?;

    let function_docs = load_function_docs()
        .with_context(|| "loading function documentation catalog")?;

    let mut index = String::new();
    index.push_str("# handl API Reference\n\n");
    if let Some(first) = catalog.reference.first() {
        if let Some(desc) = &first.description {
            index.push_str(&format!("{}\n\n", desc));
        } else {
            index.push_str("User-facing reference by domain.\n\n");
        }
    }

    for cat in &catalog.reference {
        let cat_md = generate_category(cat, &function_docs)
            .with_context(|| format!("generating category {}", cat.slug))?;
        let path = std::path::Path::new(&out_dir).join(format!("{}.md", cat.slug));
        fs::write(&path, cat_md).with_context(|| format!("writing {}", path.display()))?;

        let desc = cat.description.as_deref().unwrap_or("");
        index.push_str(&format!(
            "- [{}](./{}){}\n",
            cat.title,
            cat.slug,
            if desc.is_empty() {
                String::new()
            } else {
                format!(" — {}", desc)
            }
        ));
    }

    fs::write(std::path::Path::new(&out_dir).join("index.md"), index)
        .with_context(|| format!("writing {}/index.md", out_dir))?;

    Ok(())
}

fn generate_category(cat: &Category, function_docs: &FunctionDocsMap) -> Result<String> {
    let mut md = String::new();

    md.push_str(&format!("# {}\n\n", cat.title));
    if let Some(desc) = &cat.description {
        md.push_str(&format!("{}\n\n", desc));
    }

    for inc in &cat.include {
        let krate = &inc.krate;
        let crate_underscore = krate.replace('-', "_");
        let json_path = format!("target/doc/{}.json", crate_underscore);

        let krate_json: Crate = serde_json::from_reader(
            fs::File::open(&json_path)
                .with_context(|| format!("opening rustdoc JSON {}", json_path))?,
        )
        .with_context(|| format!("parsing rustdoc JSON {}", json_path))?;

        if inc.reexports || inc.reexports_from.is_some() {
            let modules = collect_modules(&krate_json, inc.reexports_from.as_deref())?;
            let mut found_any = false;

            for (rust_module, handl_module, span_filename, module_summary) in modules {
                found_any = true;

                let module_id = if rust_module.is_empty() {
                    krate_json.root.clone()
                } else if let Some(id) = module_id_from_path(&krate_json, &crate_underscore, &rust_module) {
                    id
                } else {
                    // Fall back to the crate root and let function-docs.yaml drive the listing.
                    krate_json.root.clone()
                };

                let names = read_module_function_names(&span_filename);
                let functions = if names.is_empty() {
                    Vec::new()
                } else {
                    collect_module_functions(&krate_json, &module_id, &names)
                };

                // Use hand-curated docs when rustdoc/span extraction fails.
                let module_docs = function_docs.get(&handl_module);

                if functions.is_empty() && names.is_empty() && module_docs.is_none() {
                    continue;
                }

                md.push_str(&format!("## `{}`\n\n", handl_module));
                if let Some(summary) = module_summary {
                    if !summary.is_empty() {
                        md.push_str(&format!("{}\n\n", escape_html(&capitalize_first(&summary))));
                    }
                }

                if !functions.is_empty() {
                    for (ac_fn, summary, _id) in functions {
                        if ac_fn == handl_module {
                            continue;
                        }
                        md.push_str(&format!("### `{}.{}`\n\n", handl_module, ac_fn));

                        let docs = module_docs.and_then(|m| m.get(&ac_fn));

                        let description = docs
                            .and_then(|d| d.description.as_deref())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or(summary);

                        if !description.is_empty() {
                            md.push_str(&format!("{}\n\n", escape_html(&description)));
                        }

                        write_function_doc(&mut md, docs, &handl_module, &ac_fn);
                    }
                } else if !names.is_empty() {
                    // The module maps to fields/inner items rather than top-level
                    // function structs, so list each mapped handl name.
                    for (_rust_name, ac_fn) in &names {
                        if ac_fn == &handl_module {
                            continue;
                        }
                        md.push_str(&format!("### `{}.{}`\n\n", handl_module, ac_fn));

                        let docs = module_docs.and_then(|m| m.get(ac_fn));

                        write_function_doc(&mut md, docs, &handl_module, ac_fn);
                    }
                } else if let Some(docs) = module_docs {
                    for (ac_fn, doc) in docs {
                        if ac_fn == &handl_module {
                            continue;
                        }
                        md.push_str(&format!("### `{}.{}`\n\n", handl_module, ac_fn));

                        if let Some(description) =
                            doc.description.as_deref().filter(|s| !s.is_empty())
                        {
                            md.push_str(&format!("{}\n\n", escape_html(description)));
                        }

                        write_function_doc(&mut md, Some(doc), &handl_module, ac_fn);
                    }
                }

                md.push('\n');
            }

            if inc.reexports && found_any {
                continue;
            }

            // Fallback: if no re-exported handl Module was found, read the
            // crate's hand-curated function-docs.yaml (if any) and emit its
            // modules.
            if !found_any {
                let crate_dir = krate
                    .strip_prefix("handl-")
                    .unwrap_or(krate)
                    .to_string();
                let crate_docs_path = std::path::Path::new("crates")
                    .join(crate_dir)
                    .join("function-docs.yaml");
                if let Ok(yaml) = fs::read_to_string(&crate_docs_path) {
                    if let Ok(crate_docs) = serde_yaml::from_str::<FunctionDocsMap>(&yaml) {
                        for (handl_module, fns) in crate_docs {
                            md.push_str(&format!("## `{}`\n\n", handl_module));
                            for (ac_fn, docs) in fns {
                                if ac_fn == handl_module {
                                    continue;
                                }
                                md.push_str(&format!(
                                    "### `{}.{}`\n\n",
                                    handl_module, ac_fn
                                ));
                                write_function_doc(&mut md, Some(&docs), &handl_module, &ac_fn);
                            }
                            md.push('\n');
                        }
                        continue;
                    }
                }
            }
        }

        let module = inc.module.as_deref().unwrap_or("");
        let module_id = if module.is_empty() {
            krate_json.root.clone()
        } else {
            find_module_id(&krate_json, module)
                .with_context(|| format!("module '{}' not found in {}", module, krate))?
        };

        let module_item = krate_json
            .index
            .get(&module_id)
            .with_context(|| format!("module '{}' missing in index for {}", module, krate))?;

        let mod_display = if module.is_empty() {
            krate.clone()
        } else {
            module.to_string()
        };
        md.push_str(&format!("## `{}`", mod_display));
        if let Some(link) = find_page_link(&crate_underscore, module) {
            md.push_str(&format!(" ([docs](/api/{}))", link));
        }
        md.push_str("\n\n");

        if let Some(docs) = &module_item.docs {
            md.push_str(&format!("{}\n\n", first_para(docs)));
        }

        if let ItemEnum::Module(mod_data) = &module_item.inner {
            let mut items: Vec<(&Item, &str)> = mod_data
                .items
                .iter()
                .filter_map(|id| {
                    krate_json
                        .index
                        .get(id)
                        .map(|it| (it, kind_name(&it.inner)))
                })
                .collect();

            let keep = HashSet::from([
                "fn", "struct", "enum", "trait", "type", "union", "const", "static", "macro", "mod",
            ]);
            items.retain(|(_, k)| keep.contains(k));
            items.sort_by(|a, b| {
                let by_kind = a.1.cmp(b.1);
                if by_kind == std::cmp::Ordering::Equal {
                    a.0.name
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.0.name.as_deref().unwrap_or(""))
                } else {
                    by_kind
                }
            });

            for (item, kind) in items {
                let name = item.name.as_deref().unwrap_or("<anon>");
                let summary = item.docs.as_deref().map(first_para).unwrap_or_default();
                md.push_str(&format!("- **{}** `{}`", kind, name));
                if !summary.is_empty() {
                    md.push_str(&format!(" — {}", summary));
                }
                md.push('\n');
            }
        }

        md.push('\n');
    }

    Ok(md)
}

fn collect_modules(
    krate: &Crate,
    prefix: Option<&str>,
) -> Result<Vec<(String, String, std::path::PathBuf, Option<String>)>> {
    let root = krate
        .index
        .get(&krate.root)
        .context("crate root not in index")?;

    let root_mod = match &root.inner {
        ItemEnum::Module(m) => m,
        _ => anyhow::bail!("crate root is not a module"),
    };

    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for id in &root_mod.items {
        let item = match krate.index.get(id) {
            Some(i) => i,
            None => continue,
        };

        // `pub use path::...Module;` re-exports
        if let ItemEnum::Use(u) = &item.inner {
            if u.is_glob || !u.name.ends_with("Module") {
                continue;
            }

            let rust_module = u.source.split("::").next().unwrap_or(&u.name).to_string();

            if let Some(p) = prefix {
                if rust_module != p {
                    continue;
                }
            }

            if let Some(target_id) = &u.id {
                if !seen.insert(target_id.clone()) {
                    continue;
                }
                if let Some(target) = krate.index.get(target_id) {
                    let span = target
                        .span
                        .as_ref()
                        .map(|s| s.filename.clone())
                        .unwrap_or_default();
                    let handl_module = read_module_name(&span)
                        .unwrap_or_else(|| to_snake_case(u.name.trim_end_matches("Module")));
                    let summary = target.docs.as_deref().map(first_para);
                    out.push((rust_module, handl_module, span, summary));
                }
            }
        }

        // `pub struct ...Module;` defined at the crate root
        if let ItemEnum::Struct(_) = &item.inner {
            if let Some(name) = &item.name {
                if name.ends_with("Module") && seen.insert(id.clone()) {
                    let span = item
                        .span
                        .as_ref()
                        .map(|s| s.filename.clone())
                        .unwrap_or_default();
                    let handl_module = read_module_name(&span)
                        .unwrap_or_else(|| to_snake_case(name.trim_end_matches("Module")));
                    let summary = item.docs.as_deref().map(first_para);
                    out.push((String::new(), handl_module, span, summary));
                }
            }
        }
    }

    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

fn module_id_from_path(krate: &Crate, _crate_name: &str, module_name: &str) -> Option<Id> {
    let root_crate_id = krate.index.get(&krate.root)?.crate_id;
    for (id, item) in &krate.index {
        if item.crate_id != root_crate_id {
            continue;
        }
        if matches!(&item.inner, ItemEnum::Module(_))
            && item.name.as_deref() == Some(module_name)
        {
            return Some(id.clone());
        }
    }
    None
}

fn collect_module_functions(
    krate: &Crate,
    module_id: &Id,
    names: &BTreeMap<String, String>,
) -> Vec<(String, String, Id)> {
    let mut seen = HashSet::new();
    let mut out: Vec<(String, String, Id, bool)> = Vec::new();
    collect_module_functions_rec(krate, module_id, &mut seen, &mut out, names);

    out.retain(|(name, _, _, in_names)| {
        let lower = name.to_lowercase();
        !lower.ends_with("module")
            && !lower.ends_with("tests")
            && !lower.ends_with("test")
            && (names.is_empty() || *in_names)
    });

    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter()
        .map(|(a, b, c, _)| (a, b, c))
        .collect()
}

fn collect_module_functions_rec(
    krate: &Crate,
    id: &Id,
    seen: &mut HashSet<Id>,
    out: &mut Vec<(String, String, Id, bool)>,
    names: &BTreeMap<String, String>,
) {
    if !seen.insert(id.clone()) {
        return;
    }

    let item = match krate.index.get(id) {
        Some(i) => i,
        None => return,
    };

    if let ItemEnum::Module(mod_data) = &item.inner {
        for child_id in &mod_data.items {
            let child = match krate.index.get(child_id) {
                Some(c) => c,
                None => continue,
            };

            match &child.inner {
                ItemEnum::Module(_) => {
                    let name = child.name.as_deref().unwrap_or("");
                    let lower = name.to_lowercase();
                    if !lower.ends_with("tests") && !lower.ends_with("test") {
                        collect_module_functions_rec(krate, child_id, seen, out, names);
                    }
                }
                _ => {
                    let kind = kind_name(&child.inner);
                    if ![
                        "fn", "struct", "enum", "trait", "type", "union", "const", "static",
                    ]
                    .contains(&kind)
                    {
                        continue;
                    }
                    if let Some(name) = child.name.as_deref() {
                        let summary = child.docs.as_deref().map(first_para).unwrap_or_default();
                        let in_names = names.contains_key(name);
                        let ac_name = if in_names {
                            names.get(name).unwrap().clone()
                        } else {
                            to_snake_case(name)
                        };
                        out.push((ac_name, summary, child_id.clone(), in_names));
                    }
                }
            }
        }
    }
}

fn find_module_id(krate: &Crate, name: &str) -> Option<Id> {
    let root = krate.index.get(&krate.root)?;
    if let ItemEnum::Module(root_mod) = &root.inner {
        for id in &root_mod.items {
            let item = krate.index.get(id)?;
            if item.name.as_deref() == Some(name) {
                if let ItemEnum::Module(_) = item.inner {
                    return Some(id.clone());
                }
            }
        }
    }
    None
}

// Module doc links are derived from `find_page_link`.

fn find_page_link(crate_underscore: &str, module: &str) -> Option<String> {
    if module.is_empty() {
        Some(format!("{}/", crate_underscore))
    } else {
        let dir = PathBuf::from(format!("docs/api/{}/{}", crate_underscore, module));
        if !dir.is_dir() {
            let file = PathBuf::from(format!("docs/api/{}/{}.md", crate_underscore, module));
            if file.is_file() {
                return Some(format!("{}/{}", crate_underscore, module));
            }
            return None;
        }

        let md_files: Vec<String> = fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                let name = e.file_name();
                let name = name.to_str()?;
                if name.ends_with(".md") {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .collect();

        if md_files.contains(&"module.md".to_string()) {
            Some(format!("{}/{}/module", crate_underscore, module))
        } else if md_files.contains(&format!("{}.md", module)) {
            Some(format!("{}/{}/{}", crate_underscore, module, module))
        } else {
            md_files
                .first()
                .map(|f| f.trim_end_matches(".md").to_string())
                .map(|stem| format!("{}/{}/{}", crate_underscore, module, stem))
        }
    }
}

fn kind_name(inner: &ItemEnum) -> &'static str {
    match inner {
        ItemEnum::Function(_) => "fn",
        ItemEnum::Struct(_) => "struct",
        ItemEnum::Enum(_) => "enum",
        ItemEnum::Trait(_) => "trait",
        ItemEnum::TypeAlias(_) => "type",
        ItemEnum::Union(_) => "union",
        ItemEnum::Constant { .. } => "const",
        ItemEnum::Static(_) => "static",
        ItemEnum::Macro(_) => "macro",
        ItemEnum::Module(_) => "mod",
        _ => "item",
    }
}

fn to_snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + name.len() / 2);
    let mut prev: Option<char> = None;

    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                if let Some(p) = prev {
                    if p.is_lowercase() || p.is_ascii_digit() {
                        out.push('_');
                    } else if i > 0 && p.is_uppercase() {
                        // potential end of acronym: look at next
                        if let Some(next) = name.chars().nth(i + 1) {
                            if next.is_lowercase() {
                                out.push('_');
                            }
                        }
                    }
                }
            }
            out.push(c.to_lowercase().next().unwrap_or(c));
        } else if c.is_ascii_digit() {
            if i > 0 {
                if let Some(p) = prev {
                    if p.is_alphabetic() {
                        out.push('_');
                    }
                }
            }
            out.push(c);
        } else {
            out.push(c);
        }
        prev = Some(c);
    }

    out
}

fn first_para(text: &str) -> String {
    text.split("\n\n")
        .next()
        .unwrap_or(text)
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn escape_html(text: &str) -> String {
    text.replace('<', "&lt;").replace('>', "&gt;")
}

fn write_function_doc(md: &mut String, docs: Option<&FunctionDocs>, ac_name: &str, ac_fn: &str) {
    if let Some(docs) = docs {
        if let Some(sig) = docs.signature.as_deref().filter(|s| !s.is_empty()) {
            md.push_str(&format!("**Signature:** `{}`\n\n", escape_html(sig)));
        }

        if let Some(details) = docs.details.as_deref().filter(|s| !s.is_empty()) {
            md.push_str(&format!("{}\n\n", escape_html(details)));
        }

        if !docs.params.is_empty() {
            md.push_str("**Parameters:**\n\n");
            for p in &docs.params {
                md.push_str(&format!("- `{}` (`{}`)", p.name, escape_html(&p.param_type)));
                if let Some(desc) = p.description.as_deref().filter(|s| !s.is_empty()) {
                    md.push_str(&format!(" — {}", escape_html(desc)));
                }
                md.push('\n');
            }
            md.push('\n');
        }

        if let Some(ret) = docs.return_value.as_deref().filter(|s| !s.is_empty()) {
            md.push_str(&format!("**Returns:** `{}`\n\n", escape_html(ret)));
        }

        if !docs.examples.is_empty() {
            md.push_str("**Examples:**\n\n");
            for ex in &docs.examples {
                md.push_str("```handl\n");
                md.push_str(ex);
                md.push_str("\n```\n\n");
            }
        }

        if !docs.see.is_empty() {
            md.push_str(&format!(
                "**See also:** {}\n\n",
                docs.see
                    .iter()
                    .map(|s| format!("`{}`", escape_html(s)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    } else {
        // If no docs entry, emit a minimal signature placeholder.
        md.push_str(&format!("**Signature:** `{}.{}(...)`\n\n", ac_name, ac_fn));
    }
}

fn load_function_docs() -> Result<FunctionDocsMap> {
    let mut combined = FunctionDocsMap::new();
    let crates_dir = std::path::Path::new("crates");
    if !crates_dir.is_dir() {
        return Ok(combined);
    }
    for entry in fs::read_dir(crates_dir)? {
        let entry = entry?;
        let crate_path = entry.path();
        if !crate_path.is_dir() {
            continue;
        }
        let name = crate_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !name.starts_with("lib-") {
            continue;
        }
        let docs_path = crate_path.join("function-docs.yaml");
        if !docs_path.exists() {
            continue;
        }
        let yaml = fs::read_to_string(&docs_path)
            .with_context(|| format!("reading function docs {}", docs_path.display()))?;
        let docs: FunctionDocsMap = serde_yaml::from_str(&yaml)
            .with_context(|| format!("parsing function docs {}", docs_path.display()))?;
        for (module, functions) in docs {
            if let Some(existing) = combined.get_mut(&module) {
                existing.extend(functions);
            } else {
                combined.insert(module, functions);
            }
        }
    }
    Ok(combined)
}

fn read_module_function_names(filename: &std::path::Path) -> BTreeMap<String, String> {
    let content = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(_) => return BTreeMap::new(),
    };
    let re =
Regex::new(r#""([^"]+)"\s*=>\s*Some\(&(?:(?:[a-z][a-z_0-9]*(?:::[a-z][a-z_0-9]*)*|self)(?:::|\.))?([A-Za-z][A-Za-z0-9_]*)\),"#)
            .expect("valid regex");
    re.captures_iter(&content)
        .map(|cap| (cap[2].to_string(), cap[1].to_string()))
        .collect()
}

fn read_module_name(filename: &std::path::Path) -> Option<String> {
    let content = fs::read_to_string(filename).ok()?;
    let re = Regex::new(r#"fn name\(&self\)\s*->\s*&str\s*\{\s*"([^"]+)"\s*\}"#).ok()?;
    re.captures(&content).map(|cap| cap[1].to_string())
}
