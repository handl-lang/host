//! `.sdkgen.yaml` configuration for the C backend.

use handl_plugin_sdkgen::{Arg, Function};
use heck::ToUpperCamelCase;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Per-library SDK generation configuration.
#[derive(Debug, Default, Deserialize)]
pub struct SdkgenConfig {
    /// Shared-object / library name used in adapter calls.
    pub library: Option<String>,
    /// Extra preprocessor defines passed to libclang (`-D`). Use for
    /// feature-test-gated declarations — e.g. `_GNU_SOURCE` exposes
    /// `accept4`, `pipe2`, `ppoll` on glibc.
    #[serde(default)]
    pub defines: Vec<String>,
    /// Glob patterns narrowing which constants (enum constants and
    /// object-like `#define`s) to emit — independent of `include`, which
    /// gates functions/types. Empty emits every integer constant reachable
    /// through the header's include closure.
    #[serde(default)]
    pub constants: Vec<String>,
    /// Per-header narrowing, keyed by header basename (`unistd.h`). Makes
    /// the regeneration recipe reproducible from config alone —
    /// `--include`/`--exclude` on the CLI union with these.
    #[serde(default)]
    pub headers: HashMap<String, HeaderConfig>,
    /// Per-symbol overrides.
    #[serde(default)]
    pub symbols: HashMap<String, SymbolOverride>,
    /// Hand-written doc comments, keyed by symbol name (`clock_gettime`),
    /// record.field (`sockaddr.sa_family`), or constant (`EPOLLIN`).
    /// Overrides beat both header comments and man-page lookup.
    #[serde(default)]
    pub docs: HashMap<String, String>,
    /// Set `false` to disable external doc resolution (man pages).
    /// Absent = enabled.
    pub external_docs: Option<bool>,
}

/// Per-header options.
#[derive(Debug, Default, Deserialize)]
pub struct HeaderConfig {
    /// Glob patterns narrowing which functions/types to emit — same
    /// semantics as the `--include` CLI flag.
    #[serde(default)]
    pub include: Vec<String>,
    /// Glob patterns vetoing emission — same semantics as `--exclude`.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Overrides for a single C symbol.
#[derive(Debug, Default, Deserialize)]
pub struct SymbolOverride {
    /// Argument descriptors (e.g. `["cstr", "i32"]`).
    pub args: Option<Vec<String>>,
    /// Names for the *consuming* port args (in order). Useful for rescued
    /// functions whose C parameter names were lost with the failed parse.
    pub names: Option<Vec<String>>,
    /// Return port type (e.g. `Int`, `Handle`).
    pub ret: Option<String>,
    /// Return descriptor (e.g. `i32`, `handle`).
    pub descriptor: Option<String>,
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
    pub fn apply_to_function(&self, name: &str, f: &mut Function) {
        let Some(ov) = self.symbols.get(name) else {
            return;
        };
        if let Some(args) = &ov.args {
            f.arg_descriptors = args.clone();
            // The override replaces the descriptor list wholesale — an
            // auto-mapped `out_struct` (e.g. recvmsg's `msghdr*`) no
            // longer applies; an inline `out_struct:`/`io_struct:`
            // descriptor's buf is the `{Port}Msg` read-back shape.
            f.out_struct = None;
            // Only descriptors that consume a handl argument produce a port
            // arg — `out_buf`/`out_struct`/`void` expand to out-params and
            // consume nothing. Names are recovered by C-argument index:
            // buffer descriptors expand to two C args, so the descriptor
            // position is not the C position.
            let mut carg_idx = 0usize;
            f.port_args = args
                .iter()
                .filter_map(|d| {
                    let idx = carg_idx;
                    carg_idx += descriptor_c_arg_count(d);
                    if descriptor_consumes_handl_arg(d) {
                        Some(Arg {
                            name: f
                                .port_args
                                .get(idx)
                                .map(|a| a.name.clone())
                                .unwrap_or_else(|| format!("__arg_{}", idx)),
                            port: if d.starts_with("in_struct:") {
                                format!("{}Msg", f.port_name)
                            } else {
                                descriptor_to_port(d)
                            },
                        })
                    } else {
                        None
                    }
                })
                .collect();
        }
        if let Some(names) = &ov.names {
            for (arg, name) in f.port_args.iter_mut().zip(names.iter()) {
                arg.name = name.clone();
            }
        }
        if let Some(ret) = &ov.ret {
            f.return_port = ret.clone();
            f.ret_descriptor = port_to_descriptor(ret);
        }
        if let Some(desc) = &ov.descriptor {
            f.ret_descriptor = desc.clone();
        }
        let has_struct_out = f.out_struct.is_some()
            || f.arg_descriptors
                .iter()
                .any(|d| d.starts_with("io_struct:") || d.starts_with("out_struct:"));
        if has_struct_out
            || f.arg_descriptors
                .iter()
                .any(|d| d.starts_with("io_scalar:"))
        {
            // Struct outs and scalar value-results: `ret` is independent
            // of the payload — the emitter returns the full `{ret, buf,
            // outs, vals}` record, declared in the port as `<Port>Result`.
            f.return_port = format!("{}Result", f.port_name);
        } else {
            let slots = f.arg_descriptors.iter().filter(|d| is_out_slot(d)).count();
            if slots > 1
                || f.arg_descriptors
                    .iter()
                    .any(|d| d.starts_with("out_ptr:") || d.starts_with("io_buf:"))
            {
                // `out_ptr`/`io_buf`/multi-out: `ret` is independent of
                // the written bytes (e.g. `accept`'s new fd) — the emitter
                // returns the whole `{ ret, buf, outs }` record, declared
                // in the port as `<Port>Result`.
                f.return_port = format!("{}Result", f.port_name);
            } else if f.arg_descriptors.iter().any(|d| d.starts_with("out_buf:")) {
                // `out_buf` alone: `ret` doubles as the byte count, so the
                // emitter unwraps to `r.buf` — the port says Bytes.
                f.return_port = "Bytes".into();
            }
        }
    }

    /// Build a [`Function`] from a full override for a symbol that failed
    /// to parse (e.g. `char**` params the backend cannot map). Returns
    /// `None` unless `args` is given — without it the C arity is unknown.
    /// `ret`/`descriptor` default to `Int`/`i32` when absent.
    pub fn rescue_function(&self, name: &str) -> Option<Function> {
        self.symbols.get(name)?.args.as_ref()?;
        let mut f = Function {
            name: name.to_string(),
            port_name: name.to_upper_camel_case(),
            port_args: Vec::new(),
            arg_descriptors: Vec::new(),
            return_port: "Int".into(),
            ret_descriptor: "i32".into(),
            out_struct: None,
            variadic: false,
            c_decl: String::new(),
            doc: None,
        };
        self.apply_to_function(name, &mut f);
        Some(f)
    }
}

/// Whether a descriptor allocates a callee-filled out slot
/// (`out_buf`/`out_ptr`/`io_buf`/`out_struct`).
pub(crate) fn is_out_slot(d: &str) -> bool {
    d.starts_with("out_buf:")
        || d.starts_with("out_ptr:")
        || d.starts_with("io_buf:")
        || d.starts_with("out_struct:")
        || d.starts_with("io_struct:")
}

/// Whether a marshalling descriptor consumes a Handl argument.
/// `out_buf`/`out_ptr`/`io_buf`/`out_struct`/`io_struct`/`void` expand to
/// callee-allocated out-params and consume none. Mirrors the descriptor
/// vocabulary defined by `plugin-ffi`'s marshaller.
fn descriptor_consumes_handl_arg(desc: &str) -> bool {
    desc != "void" && !is_out_slot(desc)
}

/// How many C arguments a descriptor occupies at the call site.
fn descriptor_c_arg_count(d: &str) -> usize {
    if d == "in_buf" || d == "cbytes" || d.starts_with("out_buf:") || d.starts_with("io_buf:") {
        2
    } else {
        1
    }
}

fn descriptor_to_port(d: &str) -> String {
    if d.starts_with("out_struct:") || d.starts_with("io_struct:") {
        return "Obj".to_string();
    }
    if d.starts_with("io_scalar:") {
        return "Int".to_string();
    }
    if d.starts_with("out_buf:")
        || d.starts_with("out_ptr:")
        || d.starts_with("io_buf:")
        || d == "in_buf"
    {
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
