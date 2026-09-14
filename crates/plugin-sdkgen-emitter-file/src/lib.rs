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
    grade: &'static str,
    error_type: &'static str,
    error_arrow: &'static str,
}

fn client_info(client_package: &str) -> ClientInfo {
    if client_package == "js" {
        ClientInfo {
            client_pkg: "js",
            grade: "JS",
            error_type: "JsError",
            error_arrow: "error",
        }
    } else {
        ClientInfo {
            client_pkg: "ffi",
            grade: "FFI",
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
        "from \"{}\" import {{ {}, {} }}\n",
        client.client_pkg, client.grade, client.error_type
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
        out.push_str(&format!("--- {}: {}\n", client.client_pkg, f.c_decl));
        out.push_str(&format!(
            "export {}: ({}) ~[({})]~> Result<{}, {}>\n\n",
            f.port_name, arg_list, client.grade, f.return_port, client.error_type
        ));
    }

    out
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
        } else {
            (format!("{lib}.so.6"), f.name.clone())
        };

        out.push_str(&format!(
            "export {}: {} := ({}) ~[{}]~> do {{\n",
            f.name, f.port_name, arg_names, client.grade
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
        } else if f.out_struct.is_some() {
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
