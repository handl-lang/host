//! `hnl sdkgen` CLI extension (ADR-0107).
//!
//! This crate is a lookup over the [`SdkgenRegistry`]: each linked target
//! module (`c`, `webidl`, `python`, …) registers its own backend, emitter,
//! doc pass, and spec input — there is no per-target `match` here. Hosts
//! opt targets in through this crate's Cargo features (`c`, `webidl`,
//! `python`; all on by default), each feature enabling one repo module's
//! dependency (ADR-0128 §"Registration: link-time first").

use handl_language::Language;
use handl_plugin_sdkgen::{SdkgenRegistry, SdkgenRequest, SdkgenTargetError, http_get};
use std::path::{Path, PathBuf};

/// The registry populated with every target this build linked in.
///
/// Modules register by name — the `python` target record composes
/// backend `c` + emitter `python` without `python/` crate-depending on
/// `c/`'s module. A host may extend the returned registry with
/// additional modules before driving it itself.
pub fn linked_targets() -> SdkgenRegistry {
    // `mut` is exercised only when at least one target feature is on.
    #[allow(unused_mut)]
    let mut registry = SdkgenRegistry::new();
    #[cfg(any(feature = "c", feature = "webidl"))]
    handl_plugin_sdkgen_emitter_file::register(&mut registry);
    #[cfg(feature = "c")]
    handl_plugin_sdkgen_backend_c::register(&mut registry);
    #[cfg(feature = "webidl")]
    handl_plugin_sdkgen_backend_webidl::register(&mut registry);
    #[cfg(feature = "python")]
    handl_plugin_sdkgen_emitter_python::register(&mut registry);
    #[cfg(feature = "cli")]
    handl_plugin_sdkgen_cli::register(&mut registry);
    registry
}

/// Run the `hnl sdkgen` subcommand and return the process exit code.
///
/// `lang` is the language the emitted `.hnl` port/adapter sources are
/// canonicalized against — injected by the host (ADR-0128 §"The 0.8 seam")
/// because this crate must not link `handl-kit-data-prelude`.
pub fn sdkgen_command(lang: &Language, args: &[String]) -> i32 {
    let registry = linked_targets();
    let usage = format!(
        "usage: hnl sdkgen <{}> [<header|spec> | --from-url <url>] --output <dir> [--target <triple>] [--include <glob>] [--exclude <glob>] [--config <yaml>]",
        registry.target_names().join("|")
    );
    if args.is_empty() {
        eprintln!("{usage}");
        return 2;
    }

    let kind = args[0].as_str();
    if registry.target(kind).is_none() {
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

    let request = SdkgenRequest {
        spec: &header,
        source_url: source_url.as_deref(),
        output: output_path,
        target: target.as_deref(),
        include: include.as_deref(),
        exclude: exclude.as_deref(),
        config: config_path.as_deref().map(Path::new),
        lang,
    };
    match registry.run(kind, &request) {
        Ok(()) => 0,
        Err(SdkgenTargetError::Usage(e)) => {
            eprintln!("sdkgen: {e}");
            2
        }
        Err(SdkgenTargetError::Failed(e)) => {
            eprintln!("sdkgen: {e}");
            1
        }
    }
}

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
