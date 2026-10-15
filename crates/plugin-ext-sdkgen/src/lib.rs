//! `hnl sdkgen` CLI extension (ADR-0107).

use handl_language::Language;
use handl_plugin_sdkgen::{Emitter, SdkgenBackend, http_get};
use handl_plugin_sdkgen_backend_c::{CBackend, SdkgenConfig};
use handl_plugin_sdkgen_backend_webidl::{MdnDocSource, WebIdlBackend, WebIdlConfig};
use handl_plugin_sdkgen_emitter_file::FileEmitter;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Run the `hnl sdkgen` subcommand and return the process exit code.
///
/// `lang` is the language the emitted `.hnl` port/adapter sources are
/// canonicalized against — injected by the host (ADR-0128 §"The 0.8 seam")
/// because this crate must not link `handl-kit-data-canonical`.
pub fn sdkgen_command(lang: &Language, args: &[String]) -> i32 {
    let usage = "usage: hnl sdkgen <c|webidl|python> [<header|spec> | --from-url <url>] --output <dir> [--target <triple>] [--include <glob>] [--exclude <glob>] [--config <yaml>]";
    if args.is_empty() {
        eprintln!("{usage}");
        return 2;
    }

    let kind = &args[0];
    if kind != "c" && kind != "webidl" && kind != "python" {
        eprintln!("{usage}");
        return 2;
    }

    let mut header: Option<String> = None;
    let mut output: Option<String> = None;
    let mut target: Option<String> = None;
    let mut include: Option<String> = None;
    let mut exclude: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut from_url: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" => {
                i += 1;
                output = args.get(i).map(String::to_string);
            }
            "--target" => {
                i += 1;
                target = args.get(i).map(String::to_string);
            }
            "--include" => {
                i += 1;
                include = args.get(i).map(String::to_string);
            }
            "--exclude" => {
                i += 1;
                exclude = args.get(i).map(String::to_string);
            }
            "--config" => {
                i += 1;
                config_path = args.get(i).map(String::to_string);
            }
            "--from-url" => {
                i += 1;
                from_url = args.get(i).map(String::to_string);
            }
            s if s.starts_with("--") => {
                eprintln!("sdkgen: unknown flag `{s}`");
                return 2;
            }
            s => {
                if header.is_none() {
                    header = Some(s.to_string());
                } else {
                    eprintln!("sdkgen: unexpected argument `{s}`");
                    return 2;
                }
            }
        }
        i += 1;
    }

    let Some(output) = output else {
        eprintln!("sdkgen: missing --output");
        return 2;
    };
    if header.is_some() && from_url.is_some() {
        eprintln!("sdkgen: <header|spec> and --from-url are mutually exclusive");
        return 2;
    }

    let output_path = Path::new(&output);

    // Resolve the input spec: either a local path or a downloaded URL. The
    // fetched file lands next to the output dir (the `generated/` spec
    // convention) so the port's `module.library` still derives from the
    // upstream basename. `source_url` is kept to stamp the module's
    // provenance (emitted as `--- source:` header lines).
    let source_url = from_url.clone();
    let header: PathBuf = match (header, from_url) {
        (Some(h), None) => PathBuf::from(h),
        (None, Some(url)) => {
            let spec_dir = output_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            match fetch_spec(&url, spec_dir) {
                Ok(p) => {
                    eprintln!("sdkgen: fetched {url} -> {}", p.display());
                    p
                }
                Err(e) => {
                    eprintln!("sdkgen: {e}");
                    return 1;
                }
            }
        }
        (None, None) => {
            eprintln!("sdkgen: missing <header|spec>");
            return 2;
        }
        (Some(_), Some(_)) => unreachable!(),
    };

    match kind.as_str() {
        "c" => {
            let config = match config_path {
                Some(p) => match SdkgenConfig::load(Path::new(&p)) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("sdkgen: {e}");
                        return 1;
                    }
                },
                None => SdkgenConfig::default(),
            };
            let backend = match CBackend::new(target, config, include, exclude) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("sdkgen: {e}");
                    return 1;
                }
            };
            let mut module = match backend.parse_header(&header) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("sdkgen: {e}");
                    return 1;
                }
            };
            module.source = source_url.clone();
            backend.apply_docs(&mut module, None);
            for d in &module.diagnostics {
                eprintln!("sdkgen: diagnostic: {d}");
            }
            let emitter = FileEmitter::new(std::sync::Arc::new(lang.clone()));
            if let Err(e) = emitter.emit(&module, output_path) {
                eprintln!("sdkgen: {e}");
                return 1;
            }
        }
        "webidl" => {
            if include.is_some() || exclude.is_some() {
                eprintln!("sdkgen: --include/--exclude are only supported for `sdkgen c`");
                return 2;
            }
            let config = match config_path.as_deref() {
                Some(p) => match WebIdlConfig::load(Path::new(p)) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("sdkgen: {e}");
                        return 1;
                    }
                },
                None => WebIdlConfig::default(),
            };
            let mut backend = WebIdlBackend::new(target);
            // `includes:` paths are relative to the config file's dir.
            if let Some(cfg_dir) = config_path.as_deref().map(Path::new).and_then(Path::parent) {
                for inc in &config.includes {
                    backend.add_include_spec(cfg_dir.join(inc));
                }
            }
            let mut module = match backend.parse_header(&header) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("sdkgen: {e}");
                    return 1;
                }
            };
            module.source = source_url.clone();
            // Persistent MDN cache beside the generated output — repeat
            // regens skip the network for known keys (misses cached too).
            let mdn = MdnDocSource::with_disk_cache(
                output_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(".mdn-cache.json"),
            );
            backend.apply_docs(&mut module, &config, Some(&mdn));
            for d in &module.diagnostics {
                eprintln!("sdkgen: diagnostic: {d}");
            }
            let emitter = FileEmitter::new(std::sync::Arc::new(lang.clone()));
            if let Err(e) = emitter.emit(&module, output_path) {
                eprintln!("sdkgen: {e}");
                return 1;
            }
        }
        "python" => {
            // A C header goes through the C backend and the Python
            // emitter — a faithful ctypes projection of the same C ABI
            // the Handl port/adapter pair exposes. A `.yaml`/`.json`
            // spec takes the older spec-driven path (an embedded
            // CPython interpreter over an env-var side channel).
            let is_header = !matches!(
                header.extension().and_then(|e| e.to_str()),
                Some("yaml") | Some("yml") | Some("json")
            );
            if is_header {
                let config = match config_path {
                    Some(p) => match SdkgenConfig::load(Path::new(&p)) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("sdkgen: {e}");
                            return 1;
                        }
                    },
                    None => SdkgenConfig::default(),
                };
                let backend = match CBackend::new(target, config, include, exclude) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("sdkgen: {e}");
                        return 1;
                    }
                };
                let mut module = match backend.parse_header(&header) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("sdkgen: {e}");
                        return 1;
                    }
                };
                module.source = source_url.clone();
                for d in &module.diagnostics {
                    eprintln!("sdkgen: diagnostic: {d}");
                }
                let emitter = handl_plugin_sdkgen_emitter_python::PythonEmitter::new();
                if let Err(e) = emitter.emit(&module, output_path) {
                    eprintln!("sdkgen: {e}");
                    return 1;
                }
            } else if let Err(e) = generate_python(&header, output_path) {
                eprintln!("sdkgen: {e}");
                return 1;
            }
        }
        _ => unreachable!(),
    }

    0
}

// ---------------------------------------------------------------------------
// `--from-url` spec fetching
// ---------------------------------------------------------------------------

/// Download `url` into `dir` under the URL's basename and return the local
/// path. The basename is preserved because backends derive the module/library
/// name from the input file stem.
fn fetch_spec(url: &str, dir: &Path) -> Result<PathBuf, String> {
    let basename = url
        .split(['?', '#'])
        .next()
        .and_then(|u| u.rsplit('/').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("spec")
        .to_string();

    let body = http_get(url, 30_000).map_err(|e| format!("failed to fetch {url}: {e}"))?;

    std::fs::create_dir_all(dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    let path = dir.join(basename);
    std::fs::write(&path, &body).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Python / spec-driven backend
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PythonSpec {
    package: String,
    doc: String,
    library: String,
    python_init: String,
    python_run: String,
    env_get: String,
    env_get_lib: String,
    env_var: String,
    functions: Vec<PythonFunction>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PythonFunction {
    name: String,
    doc: String,
    args: Vec<PythonArg>,
    script: String,
    env: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PythonArg {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

impl PythonSpec {
    fn new() -> Self {
        Self {
            package: String::new(),
            doc: String::new(),
            library: "libpython3.14.so".to_string(),
            python_init: "Py_Initialize".to_string(),
            python_run: "PyRun_SimpleString".to_string(),
            env_get: "getenv".to_string(),
            env_get_lib: "libc.so.6".to_string(),
            env_var: "HANDL_PY_OUT".to_string(),
            functions: Vec::new(),
        }
    }
}

fn load_python_spec(path: &Path) -> Result<PythonSpec, String> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read spec {}: {e}", path.display()))?;
    let mut spec: PythonSpec = if path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_yaml::from_str(&s).map_err(|e| format!("failed to parse JSON spec: {e}"))?
    } else {
        serde_yaml::from_str(&s).map_err(|e| format!("failed to parse YAML spec: {e}"))?
    };
    if spec.package.is_empty() {
        return Err("spec is missing `package` name".to_string());
    }
    if spec.functions.is_empty() {
        return Err("spec has no `functions`".to_string());
    }
    // Merge any defaults that were not explicitly supplied.
    let defaults = PythonSpec::new();
    if spec.library.is_empty() {
        spec.library = defaults.library;
    }
    if spec.python_init.is_empty() {
        spec.python_init = defaults.python_init;
    }
    if spec.python_run.is_empty() {
        spec.python_run = defaults.python_run;
    }
    if spec.env_get.is_empty() {
        spec.env_get = defaults.env_get;
    }
    if spec.env_get_lib.is_empty() {
        spec.env_get_lib = defaults.env_get_lib;
    }
    if spec.env_var.is_empty() {
        spec.env_var = defaults.env_var;
    }
    Ok(spec)
}

fn generate_python(spec_path: &Path, output: &Path) -> Result<(), String> {
    let spec = load_python_spec(spec_path)?;
    let pkg_dir = output.join(&spec.package);
    std::fs::create_dir_all(&pkg_dir)
        .map_err(|e| format!("failed to create output dir {}: {e}", pkg_dir.display()))?;

    let hnl_path = pkg_dir.join(format!("{}.hnl", spec.package));
    let hnl = build_python_hnl(&spec);
    std::fs::write(&hnl_path, hnl)
        .map_err(|e| format!("failed to write {}: {e}", hnl_path.display()))?;

    Ok(())
}

fn build_python_hnl(spec: &PythonSpec) -> String {
    let mut out = String::new();

    if !spec.doc.is_empty() {
        let lines: Vec<&str> = spec.doc.lines().collect();
        let mut first = true;
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !first {
                    out.push_str("---\n");
                }
                continue;
            }
            if first {
                out.push_str(&format!("--- {} -- {}\n", spec.package, trimmed));
                first = false;
            } else {
                out.push_str(&format!("--- {}\n", trimmed));
            }
        }
    }
    out.push('\n');

    out.push_str("from \"ffi\" import { call }\n\n");

    out.push_str(&format!(
        "py_init := () ~> call(\"{}\", \"{}\", [], [], \"void\", false)\n",
        spec.library, spec.python_init
    ));
    out.push_str(&format!(
        "py_run := (code) ~> call(\"{}\", \"{}\", [code], [\"cstr\"], \"i32\", false)\n",
        spec.library, spec.python_run
    ));
    out.push_str(&format!(
        "get_env := (name) ~> call(\"{}\", \"{}\", [name], [\"cstr\"], \"cstr\", false)\n",
        spec.env_get_lib, spec.env_get
    ));
    out.push('\n');

    for f in &spec.functions {
        if !f.doc.is_empty() {
            for line in f.doc.lines() {
                out.push_str(&format!("-- {}\n", line.trim()));
            }
        }

        let args = f
            .args
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ");

        let env_var = f.env.as_ref().unwrap_or(&spec.env_var);

        out.push_str(&format!("{} := ({}) ~> do {{\n", f.name, args));
        out.push_str("  _ = py_init();\n");
        out.push_str(&build_code_assignment(&f.script, &f.args));
        out.push_str("  _ = py_run(code);\n");
        out.push_str(&format!("  get_env(\"{}\")\n", env_var));
        out.push_str("}\n\n");
    }

    for f in &spec.functions {
        out.push_str(&format!("export {}\n", f.name));
    }

    out
}

fn build_code_assignment(script: &str, args: &[PythonArg]) -> String {
    if args.is_empty() {
        return format!("  code := \"{}\";\n", escape_hnl_string(script));
    }

    let sep = '\u{001D}'; // group separator, unlikely in Python code
    let mut work = script.to_string();
    for arg in args {
        let placeholder = format!("$${}$$", arg.name);
        work = work.replace(&placeholder, &format!("{sep}{}{sep}", arg.name));
    }

    let mut out = String::new();
    out.push_str("  code := \"");
    for (i, piece) in work.split(sep).enumerate() {
        if i % 2 == 0 {
            out.push_str(&escape_hnl_string(piece));
            if i != work.split(sep).count() - 1 {
                out.push_str("\" + ");
            }
        } else {
            out.push_str(piece);
            out.push_str(" + \"");
        }
    }
    out.push_str("\";\n");
    out
}

fn escape_hnl_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\"', "\\\"")
}
