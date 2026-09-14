//! `hnl sdkgen` CLI extension (ADR-0107).

use handl_plugin_sdkgen::{Emitter, SdkgenBackend};
use handl_plugin_sdkgen_backend_c::{CBackend, SdkgenConfig};
use handl_plugin_sdkgen_backend_webidl::WebIdlBackend;
use handl_plugin_sdkgen_emitter_file::FileEmitter;
use std::path::Path;

/// Run the `hnl sdkgen` subcommand and return the process exit code.
pub fn sdkgen_command(args: &[String]) -> i32 {
    if args.is_empty() || (args[0] != "c" && args[0] != "webidl") {
        eprintln!("usage: hnl sdkgen <c|webidl> <header> --output <dir> [--target <triple>] [--only <glob>] [--config <yaml>]");
        return 2;
    }

    let mut header: Option<String> = None;
    let mut output: Option<String> = None;
    let mut target: Option<String> = None;
    let mut only: Option<String> = None;
    let mut config_path: Option<String> = None;

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
            "--only" => {
                i += 1;
                only = args.get(i).map(String::to_string);
            }
            "--config" => {
                i += 1;
                config_path = args.get(i).map(String::to_string);
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

    let Some(header) = header else {
        eprintln!("sdkgen: missing <header>");
        return 2;
    };
    let Some(output) = output else {
        eprintln!("sdkgen: missing --output");
        return 2;
    };

    let module = match args[0].as_str() {
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
            match CBackend::new(target, config, only) {
                Ok(b) => b.parse_header(Path::new(&header)),
                Err(e) => Err(e),
            }
        }
        "webidl" => {
            let _ = only;
            let _ = config_path;
            let backend = WebIdlBackend::new(target);
            backend.parse_header(Path::new(&header))
        }
        _ => unreachable!(),
    };
    let module = match module {
        Ok(m) => m,
        Err(e) => {
            eprintln!("sdkgen: {e}");
            return 1;
        }
    };

    let emitter = FileEmitter::new();
    if let Err(e) = emitter.emit(&module, Path::new(&output)) {
        eprintln!("sdkgen: {e}");
        return 1;
    }

    0
}
