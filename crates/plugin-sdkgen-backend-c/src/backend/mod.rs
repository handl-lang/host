//! The `CBackend` entry point: libclang translation-unit setup and the
//! `SdkgenBackend::parse_header` driver.
//!
//! Pipeline: [`clang_args`] → parse TU → [`diagnostics`] gate on
//! error/fatal → [`crate::collect`] declaration cursors →
//! [`crate::parse`] each into IR with config overrides →
//! [`macro_probe`] evaluates `#define` constants → [`prune`] drops
//! unreferenced types → `SdkgenModule`.

mod clang_args;
mod diagnostics;
mod macro_probe;
mod prune;

#[cfg(test)]
mod clang_args_test;
#[cfg(test)]
mod diagnostics_test;
#[cfg(test)]
mod macro_probe_test;
#[cfg(test)]
mod prune_test;

use clang_sys::*;
use glob::Pattern;
use handl_plugin_sdkgen::{DocSource, SdkgenBackend, SdkgenError, SdkgenModule, pick_doc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_int, c_void};
use std::path::{Path, PathBuf};

use crate::collect::{CursorData, collect_cursors, collect_enum};
use crate::config::SdkgenConfig;
use crate::cx::cursor_name;
use crate::parse::{ParseCtx, parse_function, parse_record, parse_typedef};

/// C header parser using libclang.
pub struct CBackend {
    target: Option<String>,
    config: SdkgenConfig,
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
    consts: Vec<Pattern>,
}

/// Compile a comma-separated glob list (`"a*,b?,c"`) into patterns.
fn globs(list: &str) -> Result<Vec<Pattern>, SdkgenError> {
    list.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| Pattern::new(s).map_err(|e| SdkgenError(format!("invalid glob `{s}`: {e}"))))
        .collect()
}

impl CBackend {
    pub fn new(
        target: Option<String>,
        config: SdkgenConfig,
        include: Option<String>,
        exclude: Option<String>,
    ) -> Result<Self, SdkgenError> {
        let include = include.map(|p| globs(&p)).transpose()?.unwrap_or_default();
        let exclude = exclude.map(|p| globs(&p)).transpose()?.unwrap_or_default();
        let consts = config
            .constants
            .iter()
            .map(|s| globs(s))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(Self {
            target,
            config,
            include,
            exclude,
            consts,
        })
    }

    /// Layer `.sdkgen.yaml` `docs:` overrides and an optional external
    /// [`DocSource`] (e.g. [`crate::ManPageDocSource`]) over comments
    /// captured from the header. Per slot: override > in-source comment
    /// > external lookup. Keys are C symbol names; record fields use
    /// `record.field`.
    ///
    /// When `external_docs` is not `false` (the default) and no `source`
    /// is supplied, a [`ManPageDocSource`] is auto-created and consulted.
    pub fn apply_docs(&self, module: &mut SdkgenModule, source: Option<&dyn DocSource>) {
        let docs = &self.config.docs;
        // Respect `external_docs: false` — skip external lookup entirely.
        // Otherwise auto-create a man-page source if the caller gave none.
        let fallback = if self.config.external_docs != Some(false) && source.is_none() {
            Some(ManPageDocSource::new())
        } else {
            None
        };
        let src: Option<&dyn DocSource> = if self.config.external_docs == Some(false) {
            None
        } else {
            source.or_else(|| fallback.as_ref().map(|s| s as &dyn DocSource))
        };
        for f in &mut module.functions {
            pick_doc(&mut f.doc, docs, &[&f.name], src, Some(&f.name));
        }
        for r in &mut module.records {
            pick_doc(&mut r.doc, docs, &[&r.name], src, Some(&r.name));
            for f in &mut r.fields {
                let key = format!("{}.{}", r.name, f.name);
                pick_doc(&mut f.doc, docs, &[&key], None, None);
            }
        }
        for t in &mut module.typedefs {
            pick_doc(&mut t.doc, docs, &[&t.name], src, Some(&t.name));
        }
        for c in &mut module.enum_constants {
            pick_doc(&mut c.doc, docs, &[&c.name], None, None);
        }
    }
}

/// Best-effort doc lookup via the `man` command. Tries sections 2 (system
/// calls) and 3 (library functions); the NAME-section description is
/// extracted and cached per symbol. Returns `None` when `man` is absent
/// or the symbol has no page.
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
            if let Some(text) = run_man(section, key) {
                if let Some(desc) = extract_name_section(&text) {
                    return Some(desc);
                }
            }
        }
        None
    }
}

impl Default for ManPageDocSource {
    fn default() -> Self {
        Self::new()
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

/// Extract the description from the NAME section of a man page.
/// The NAME section looks like:
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

impl SdkgenBackend for CBackend {
    fn parse_header(&self, path: &Path) -> Result<SdkgenModule, SdkgenError> {
        if !path.is_file() {
            return Err(SdkgenError(format!("header not found: {}", path.display())));
        }
        clang_sys::load().map_err(|e| SdkgenError(format!("libclang load failed: {e}")))?;

        let target = self
            .target
            .clone()
            .unwrap_or_else(|| target_lexicon::Triple::host().to_string());

        let index = unsafe { clang_createIndex(0, 0) };
        if index.is_null() {
            return Err(SdkgenError("libclang createIndex failed".into()));
        }

        // `_raw_args` owns the CStrings `args` points into — must outlive
        // every libclang call that takes `args` (incl. the macro probe).
        let (_raw_args, args) = clang_args::clang_args(path, &target, &self.config.defines);
        let c_path = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let tu = unsafe {
            clang_parseTranslationUnit(
                index,
                c_path.as_ptr(),
                args.as_ptr(),
                args.len() as c_int,
                std::ptr::null_mut(),
                0,
                // DetailedPreprocessingRecord: without it macro cursors
                // never appear in the translation unit's child list.
                CXTranslationUnit_DetailedPreprocessingRecord,
            )
        };
        if tu.is_null() {
            unsafe { clang_disposeIndex(index) };
            return Err(SdkgenError("libclang failed to parse header".into()));
        }
        if let Err(e) = diagnostics::check_tu(tu) {
            unsafe { clang_disposeTranslationUnit(tu) };
            unsafe { clang_disposeIndex(index) };
            return Err(e);
        }

        let pointer_width = unsafe {
            let ti = clang_getTranslationUnitTargetInfo(tu);
            let width = clang_TargetInfo_getPointerWidth(ti);
            clang_TargetInfo_dispose(ti);
            width as usize
        };
        let ctx = ParseCtx {
            pointer_width,
            records: RefCell::new(HashMap::new()),
        };

        // Config `headers:` per-file include/exclude patterns union with
        // `--include`/`--exclude`, so a narrowed header's recipe is
        // reproducible from the yaml.
        let mut include = self.include.clone();
        let mut exclude = self.exclude.clone();
        let basename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if let Some(hc) = self.config.headers.get(basename) {
            for pat in &hc.include {
                include.extend(globs(pat)?);
            }
            for pat in &hc.exclude {
                exclude.extend(globs(pat)?);
            }
        }

        let mut cursor_data = CursorData {
            include: include.clone(),
            exclude: exclude.clone(),
            consts: self.consts.clone(),
            ..Default::default()
        };
        let root = unsafe { clang_getTranslationUnitCursor(tu) };
        unsafe {
            clang_visitChildren(
                root,
                collect_cursors,
                &mut cursor_data as *mut _ as *mut c_void,
            )
        };

        let mut module = SdkgenModule::default();
        module.client_package = "ffi".into();
        module.backend_id = "c".into();
        module.target = target;
        if let Some(lib) = &self.config.library {
            module.library = lib.clone();
        }
        module.module = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string();

        for &cursor in &cursor_data.records {
            match parse_record(cursor, &ctx) {
                Ok(rec) => {
                    if !self.config.should_skip(&rec.name) {
                        ctx.records
                            .borrow_mut()
                            .insert(rec.name.clone(), rec.clone());
                        module.records.push(rec);
                    }
                }
                Err(e) => module.diagnostics.push(e),
            }
        }

        for &cursor in &cursor_data.typedefs {
            match parse_typedef(cursor, &ctx) {
                Ok(td) => {
                    if !self.config.should_skip(&td.name) {
                        module.typedefs.push(td);
                    }
                }
                Err(e) => module.diagnostics.push(e),
            }
        }

        for &cursor in &cursor_data.enums {
            collect_enum(cursor, &mut module, &include, &exclude, &self.consts);
        }

        // Macro constants are evaluated in a probe TU, which has no comment
        // of its own — the doc comes from the main TU's macro cursors or,
        // failing that, the source text around the `#define` itself.
        let mut macro_files: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut macro_docs: HashMap<String, Option<String>> = HashMap::new();
        let mut macro_names: Vec<String> = Vec::new();
        for &c in &cursor_data.macros {
            let name = cursor_name(c);
            if name.is_empty() {
                continue;
            }
            macro_docs.insert(name.clone(), crate::cx::macro_doc(c, &mut macro_files));
            macro_names.push(name);
        }
        for mut c in macro_probe::eval_macros(index, &args, &macro_names, path) {
            if !module.enum_constants.iter().any(|e| e.name == c.name) {
                c.doc = macro_docs.get(&c.name).cloned().flatten();
                module.enum_constants.push(c);
            }
        }

        for &cursor in &cursor_data.functions {
            match parse_function(cursor, &ctx) {
                Ok(mut f) => {
                    if !self.config.should_skip(&f.name) {
                        let name = f.name.clone();
                        self.config.apply_to_function(&name, &mut f);
                        module.functions.push(f);
                    }
                }
                Err(e) => {
                    // A full `args` override can rescue a symbol whose C
                    // signature we cannot parse (e.g. `char**` params).
                    let fname = cursor_name(cursor);
                    match self.config.rescue_function(&fname) {
                        Some(mut f) if !self.config.should_skip(&f.name) => {
                            // The signature came from config, but the
                            // declaration cursor still carries its own doc
                            // comment.
                            f.doc = crate::cx::doc_comment(cursor);
                            module.functions.push(f);
                        }
                        _ => module.diagnostics.push(e),
                    }
                }
            }
        }

        prune::prune_unreferenced(&mut module);

        // Collect types/c aliases referenced by the surviving port types,
        // grouped by submodule, so the emitter can render
        // `from "types/c/<header>" import {...}` — declared on the module
        // as data, keeping the emitter backend-agnostic.
        let mut by_module: std::collections::BTreeMap<&str, std::collections::BTreeSet<String>> =
            Default::default();
        let mut note = |port: &str| {
            if let Some(m) = crate::map::ctypes_module(port) {
                by_module.entry(m).or_default().insert(port.to_string());
            }
        };
        for f in &module.functions {
            for a in &f.port_args {
                note(&a.port);
            }
            note(&f.return_port);
        }
        for r in &module.records {
            for fl in &r.fields {
                note(&fl.port);
            }
        }
        for t in &module.typedefs {
            note(&t.port);
        }
        for (package, names) in by_module {
            module.imports.push(handl_plugin_sdkgen::ModuleImport {
                package: package.into(),
                names: names.into_iter().collect(),
            });
        }

        unsafe {
            clang_disposeTranslationUnit(tu);
            clang_disposeIndex(index);
        }

        Ok(module)
    }
}
