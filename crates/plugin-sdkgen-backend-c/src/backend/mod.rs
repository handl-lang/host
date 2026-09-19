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
use handl_plugin_sdkgen::{SdkgenBackend, SdkgenError, SdkgenModule};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_int, c_void};
use std::path::Path;

use crate::collect::{CursorData, collect_cursors, collect_enum};
use crate::config::SdkgenConfig;
use crate::cx::cursor_name;
use crate::parse::{ParseCtx, parse_function, parse_record, parse_typedef};

/// C header parser using libclang.
pub struct CBackend {
    target: Option<String>,
    config: SdkgenConfig,
    only: Vec<Pattern>,
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
        only: Option<String>,
    ) -> Result<Self, SdkgenError> {
        let only = only.map(|p| globs(&p)).transpose()?.unwrap_or_default();
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
            only,
            consts,
        })
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

        let mut cursor_data = CursorData {
            only: self.only.clone(),
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
            collect_enum(cursor, &mut module, &self.only, &self.consts);
        }

        let macro_names: Vec<String> = cursor_data
            .macros
            .iter()
            .map(|&c| cursor_name(c))
            .filter(|n| !n.is_empty())
            .collect();
        for c in macro_probe::eval_macros(index, &args, &macro_names, path) {
            if !module.enum_constants.iter().any(|e| e.name == c.name) {
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
                        Some(f) if !self.config.should_skip(&f.name) => {
                            module.functions.push(f);
                        }
                        _ => module.diagnostics.push(e),
                    }
                }
            }
        }

        prune::prune_unreferenced(&mut module);

        unsafe {
            clang_disposeTranslationUnit(tu);
            clang_disposeIndex(index);
        }

        Ok(module)
    }
}
