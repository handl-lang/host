//! `.sdkgen.yaml` configuration for the C backend.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Per-library SDK generation configuration.
#[derive(Debug, Default, Deserialize)]
pub struct SdkgenConfig {
    /// Shared-object / library name used in adapter calls.
    pub library: Option<String>,
    /// Per-symbol overrides.
    #[serde(default)]
    pub symbols: HashMap<String, SymbolOverride>,
}

/// Overrides for a single C symbol.
#[derive(Debug, Default, Deserialize)]
pub struct SymbolOverride {
    /// Argument descriptors (e.g. `["cstr", "i32"]`).
    pub args: Option<Vec<String>>,
    /// Return port type (e.g. `Int`, `Handle`).
    pub ret: Option<String>,
    /// Return descriptor (e.g. `i32`, `handle`).
    pub descriptor: Option<String>,
    /// Whether the return pointer can be null.
    pub nullable: Option<bool>,
    /// Skip this symbol entirely.
    pub skip: Option<bool>,
}

impl SdkgenConfig {
    /// Load a `.sdkgen.yaml` file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {}: {}", path.display(), e))?;
        serde_yaml::from_str(&s).map_err(|e| format!("failed to parse config: {e}"))
    }

    /// Return `true` if the symbol is explicitly skipped.
    pub fn should_skip(&self, name: &str) -> bool {
        self.symbols.get(name).and_then(|o| o.skip).unwrap_or(false)
    }

    /// Apply overrides to a parsed C function.
    pub fn apply_to_function(&self, name: &str, f: &mut crate::CFunction) {
        let Some(ov) = self.symbols.get(name) else {
            return;
        };
        if let Some(args) = &ov.args {
            f.arg_descriptors = args.clone();
            f.port_args = args
                .iter()
                .enumerate()
                .map(|(i, d)| crate::CHandlArg {
                    name: f
                        .port_args
                        .get(i)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| format!("__arg_{}", i)),
                    port: descriptor_to_port(d),
                })
                .collect();
        }
        if let Some(ret) = &ov.ret {
            f.return_port = ret.clone();
            f.ret_descriptor = port_to_descriptor(ret);
        }
        if let Some(desc) = &ov.descriptor {
            f.ret_descriptor = desc.clone();
        }
    }
}

fn descriptor_to_port(d: &str) -> String {
    if d.starts_with("out_struct:") {
        return "Obj".to_string();
    }
    if d.starts_with("out_buf:") || d == "in_buf" {
        return "Bytes".to_string();
    }
    match d {
        "i32" | "i64" | "u32" | "u64" | "isize" | "usize" | "size_t" | "ssize_t" => {
            "Int".to_string()
        }
        "f64" | "f32" => "Float".to_string(),
        "cstr" => "String".to_string(),
        "cbytes" => "Bytes".to_string(),
        "ptr" => "Handle".to_string(),
        "handle" => "Handle".to_string(),
        "void" | "()" => "()".to_string(),
        _ if d.starts_with('i') || d.starts_with('u') => "Int".to_string(),
        _ => "Obj".to_string(),
    }
}

fn port_to_descriptor(p: &str) -> String {
    match p {
        "Int" => "i32".to_string(),
        "Nat" => "u32".to_string(),
        "Float" => "f64".to_string(),
        "String" => "cstr".to_string(),
        "Bytes" => "cbytes".to_string(),
        "Handle" => "handle".to_string(),
        "void" | "()" => "void".to_string(),
        _ => "ptr".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_sdkgen::{CFunction, CHandlArg};

    #[test]
    fn skip_and_override_symbols() {
        let mut cfg = SdkgenConfig::default();
        cfg.symbols.insert(
            "skip_me".to_string(),
            SymbolOverride {
                skip: Some(true),
                ..Default::default()
            },
        );
        cfg.symbols.insert(
            "open".to_string(),
            SymbolOverride {
                args: Some(vec!["cstr".to_string(), "i32".to_string()]),
                ret: Some("Handle".to_string()),
                ..Default::default()
            },
        );

        assert!(cfg.should_skip("skip_me"));
        assert!(!cfg.should_skip("open"));

        let mut f = CFunction {
            name: "open".to_string(),
            port_name: "Open".to_string(),
            port_args: vec![
                CHandlArg {
                    name: "path".to_string(),
                    port: "String".to_string(),
                },
                CHandlArg {
                    name: "flags".to_string(),
                    port: "Int".to_string(),
                },
            ],
            arg_descriptors: vec!["cstr".to_string(), "i32".to_string()],
            return_port: "Int".to_string(),
            ret_descriptor: "i32".to_string(),
            out_struct: None,
            variadic: false,
            c_decl: "int open(const char *p, int f);".to_string(),
            doc: None,
        };
        cfg.apply_to_function("open", &mut f);
        assert_eq!(f.return_port, "Handle");
        assert_eq!(f.ret_descriptor, "handle");
        assert_eq!(f.port_args.len(), 2);
        assert_eq!(f.port_args[1].port, "Int");
    }
}
