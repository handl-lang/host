//! File emitter for the Handl SDK generator (ADR-0107).

use handl_plugin_sdkgen::{Emitter, SdkgenError, SdkgenModule};
use std::path::Path;

/// Emitter that writes port and adapter `.hnl` files to disk.
pub struct FileEmitter;

impl FileEmitter {
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for FileEmitter {
    fn emit(&self, module: &SdkgenModule, output: &Path) -> Result<(), SdkgenError> {
        std::fs::create_dir_all(output)
            .map_err(|e| SdkgenError(format!("failed to create output dir: {e}")))?;

        let lib = output
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("lib")
            .to_string();
        let module_name = &module.module;
        let target_slug = target_slug(&module.target);
        let adapter_pkg = format!("{lib}-{}-{}", module.backend_id, target_slug);

        let adapter_dir = output.parent().unwrap_or(output).join(&adapter_pkg);
        std::fs::create_dir_all(&adapter_dir)
            .map_err(|e| SdkgenError(format!("failed to create adapter dir: {e}")))?;

        let port = build_port(module, &lib, module_name);
        let adapter = build_adapter(module, &lib, module_name, &adapter_pkg);

        let port_path = output.join(format!("{module_name}.hnl"));
        let adapter_path = adapter_dir.join(format!("{module_name}.hnl"));

        std::fs::write(&port_path, port)
            .map_err(|e| SdkgenError(format!("failed to write {port_path:?}: {e}")))?;
        std::fs::write(&adapter_path, adapter)
            .map_err(|e| SdkgenError(format!("failed to write {adapter_path:?}: {e}")))?;

        Ok(())
    }
}

fn target_slug(target: &str) -> String {
    let parts: Vec<&str> = target.split('-').collect();
    let arch = parts.first().copied().unwrap_or("unknown");
    let os = parts
        .get(2)
        .copied()
        .or(parts.get(1).copied())
        .unwrap_or("unknown");
    format!("{os}-{arch}")
}

struct ClientInfo {
    client_pkg: &'static str,
    error_type: &'static str,
    error_arrow: &'static str,
}

fn client_info(client_package: &str) -> ClientInfo {
    if client_package == "js" {
        ClientInfo {
            client_pkg: "js",
            error_type: "JsError",
            error_arrow: "error",
        }
    } else {
        ClientInfo {
            client_pkg: "ffi",
            error_type: "Errno",
            error_arrow: "errno",
        }
    }
}

fn build_port(module: &SdkgenModule, lib: &str, name: &str) -> String {
    let client = client_info(&module.client_package);
    let mut out = String::new();
    out.push_str(&format!("--- Generated port for {lib}.{name}\n"));
    out.push_str(&format!(
        "--- {} target: {}\n",
        client.client_pkg, module.target
    ));
    out.push_str("\n");
    out.push_str(&format!(
        "from \"{}\" import {{ {} }}\n",
        client.client_pkg, client.error_type
    ));
    out.push_str("from \"result\" import { Result }\n");
    out.push_str("\n");

    for rec in &module.records {
        if let Some(d) = &rec.doc {
            for line in d.lines() {
                out.push_str(&format!("--- {}\n", line.trim()));
            }
        }
        out.push_str(&format!("export {} := {{\n", rec.name));
        for (i, f) in rec.fields.iter().enumerate() {
            let comma = if i == rec.fields.len() - 1 { "" } else { "," };
            out.push_str(&format!("  {}: {}{}\n", f.name, f.port, comma));
        }
        out.push_str("}\n\n");
    }

    for td in &module.typedefs {
        if let Some(d) = &td.doc {
            for line in d.lines() {
                out.push_str(&format!("--- {}\n", line.trim()));
            }
        }
        out.push_str(&format!("export {} := {}\n\n", td.name, td.port));
    }

    for c in &module.enum_constants {
        out.push_str(&format!("export {}: Int\n", c.name));
    }
    if !module.enum_constants.is_empty() {
        out.push('\n');
    }

    for f in &module.functions {
        let arg_list = f
            .port_args
            .iter()
            .map(|a| a.port.clone())
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(d) = &f.doc {
            for line in d.lines() {
                out.push_str(&format!("--- {}\n", line.trim()));
            }
        }
        if returns_outs_record(f) {
            // Multi-out / value-result functions return the full
            // `{ ret, buf, outs }` marshal record.
            out.push_str(&format!(
                "export {}Result := {{ ret: Int, buf: Bytes, outs: List<Bytes> }}\n\n",
                f.port_name
            ));
        }
        if let Some(fields) = msg_record_fields(f) {
            // in_struct/io_struct functions pass or return a C struct —
            // declare its Handl record shape as `<Port>Msg`.
            out.push_str(&format!("export {}Msg := {{\n", f.port_name));
            for (i, (fname, port)) in fields.iter().enumerate() {
                let comma = if i == fields.len() - 1 { "" } else { "," };
                out.push_str(&format!("  {fname}: {port}{comma}\n"));
            }
            out.push_str("}\n\n");
        }
        out.push_str(&format!("--- {}: {}\n", client.client_pkg, f.c_decl));
        out.push_str(&format!(
            "export {}: ({}) ~[1]~> Result<{}, {}>\n\n",
            f.port_name, arg_list, f.return_port, client.error_type
        ));
    }

    out
}

/// Whether the function's return is the full `{ ret, buf, outs }` record
/// rather than `r.buf` or `r` alone: any `out_ptr`/`io_buf` slot (where
/// `ret` carries its own meaning, e.g. `accept`'s fd) or more than one
/// byte out-slot. `out_struct` functions keep the `r.buf` unwrap.
fn returns_outs_record(f: &handl_plugin_sdkgen::Function) -> bool {
    if f.out_struct.is_some() {
        return false;
    }
    let buf_slots = f
        .arg_descriptors
        .iter()
        .filter(|d| {
            d.starts_with("out_buf:") || d.starts_with("out_ptr:") || d.starts_with("io_buf:")
        })
        .count();
    buf_slots > 1
        || f.arg_descriptors
            .iter()
            .any(|d| d.starts_with("out_ptr:") || d.starts_with("io_buf:"))
}

/// Parse the field list of an `in_struct:{...}`/`io_struct:{...}`
/// descriptor into `(name, port-type)` pairs for the port's record decl.
fn msg_record_fields(f: &handl_plugin_sdkgen::Function) -> Option<Vec<(String, String)>> {
    let desc = f
        .arg_descriptors
        .iter()
        .find(|d| d.starts_with("in_struct:") || d.starts_with("io_struct:"))?;
    let inner = desc
        .split_once(':')
        .map(|(_, r)| r)?
        .strip_prefix('{')?
        .strip_suffix('}')?;
    let fields = inner
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (name, spec) = p.split_once(':')?;
            let spec = spec.trim();
            let port = if spec == "bytes" || spec.starts_with("buf:") {
                "Bytes"
            } else if spec == "iov" || spec.starts_with("iovbuf:") {
                "List<Bytes>"
            } else if spec.starts_with('u') || spec.starts_with("isize") || spec == "usize" {
                "Nat"
            } else {
                "Int"
            };
            Some((name.trim().to_string(), port.to_string()))
        })
        .collect();
    Some(fields)
}

fn build_adapter(module: &SdkgenModule, lib: &str, name: &str, adapter_pkg: &str) -> String {
    let client = client_info(&module.client_package);
    let mut out = String::new();
    out.push_str(&format!("--- Generated adapter for {adapter_pkg}/{name}\n"));
    out.push_str("\n");
    out.push_str(&format!(
        "from \"{}\" import {{ call, {} }}\n",
        client.client_pkg, client.error_arrow
    ));
    out.push_str("from \"result\" import { ok, err }\n");

    let mut imports = Vec::new();
    for rec in &module.records {
        imports.push(rec.name.clone());
    }
    for f in &module.functions {
        imports.push(f.port_name.clone());
    }
    if !imports.is_empty() {
        out.push_str(&format!(
            "from \"../{lib}/{name}\" import {{ {} }}\n",
            imports.join(", ")
        ));
    }
    out.push_str("\n");

    // Constants are pure values — no ffi.call involved. The port file
    // declares the type; the adapter binds the (target-specific) value.
    for c in &module.enum_constants {
        out.push_str(&format!("export {}: Int := {}\n", c.name, c.value));
    }
    if !module.enum_constants.is_empty() {
        out.push('\n');
    }

    for f in &module.functions {
        let arg_names = f
            .port_args
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ");

        let descriptors: Vec<String> = f
            .arg_descriptors
            .iter()
            .map(|d| {
                let mapped = if client.client_pkg == "js" {
                    js_descriptor(d)
                } else {
                    d.clone()
                };
                format!("\"{mapped}\"")
            })
            .collect();
        let descs = descriptors.join(", ");

        let ret_descriptor = if client.client_pkg == "js" {
            js_descriptor(&f.ret_descriptor)
        } else {
            f.ret_descriptor.clone()
        };

        let (call_lib, call_sym) = if client.client_pkg == "js" {
            let method = f.c_decl.rsplit_once('.').map(|(_, m)| m).unwrap_or(&f.name);
            (module.library.clone(), method.to_string())
        } else if module.library.is_empty() {
            (format!("{lib}.so.6"), f.name.clone())
        } else {
            (module.library.clone(), f.name.clone())
        };

        out.push_str(&format!(
            "export {}: {} := ({}) ~[1]~> do {{\n",
            f.name, f.port_name, arg_names
        ));
        out.push_str(&format!(
            "  r = call(\"{call_lib}\", \"{call_sym}\", [{arg_names}], [{descs}], \"{ret_descriptor}\", false)\n"
        ));

        if client.client_pkg == "js" {
            out.push_str("  e = error()\n");
            out.push_str("  if e == null {\n");
            out.push_str("    ok(r)\n");
            out.push_str("  } else {\n");
            out.push_str("    err(e)\n");
            out.push_str("  }\n");
        } else if returns_outs_record(f) {
            out.push_str("  if r.ret < 0 {\n");
            out.push_str("    err(errno())\n");
            out.push_str("  } else {\n");
            out.push_str("    ok(r)\n");
            out.push_str("  }\n");
        } else if f.out_struct.is_some()
            || f.arg_descriptors.iter().any(|d| {
                d.starts_with("out_buf:")
                    || d.starts_with("out_struct:")
                    || d.starts_with("io_struct:")
            })
        {
            out.push_str("  if r.ret < 0 {\n");
            out.push_str("    err(errno())\n");
            out.push_str("  } else {\n");
            out.push_str("    ok(r.buf)\n");
            out.push_str("  }\n");
        } else if f.ret_descriptor == "void" {
            out.push_str("  ok(null)\n");
        } else if is_numeric_descriptor(&f.ret_descriptor) {
            out.push_str("  if r < 0 {\n");
            out.push_str("    err(errno())\n");
            out.push_str("  } else {\n");
            out.push_str("    ok(r)\n");
            out.push_str("  }\n");
        } else {
            out.push_str("  ok(r)\n");
        }

        out.push_str("}\n\n");
    }

    out
}

fn js_descriptor(d: &str) -> String {
    match d {
        "js_string" | "js_number" | "js_object" | "js_boolean" | "js_promise" | "js_void" => {
            d.to_string()
        }
        "void" => "js_void".to_string(),
        "bool" => "js_boolean".to_string(),
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => {
            "js_number".to_string()
        }
        s if s.starts_with("promise:") || s == "promise" => "js_promise".to_string(),
        _ => d.to_string(),
    }
}

fn is_numeric_descriptor(d: &str) -> bool {
    d.starts_with('i') || d.starts_with('u') || d == "f64"
}
