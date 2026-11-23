//! `hnl-ext-mlopt` — the mlopt product commands as a dynamic `hnl`
//! extension (ADR-0149), the kit-prelude successor to the same-named
//! crate that lived in the mlopt repo.
//!
//! The optimization engine is Python (`mlopt` package — graphopt /
//! eqsat / kernelopt git deps). This crate is pure *packaging*: it
//! exports the ADR-0149 ABI (`handl_ext_manifest` / `handl_ext_dispatch`
//! / `handl_ext_free`) and forwards each declared command to
//! `python -m mlopt <cmd> <args>` through [`handl_kit_mlopt::run_mlopt`],
//! mapping the child's exit code and streams onto `ExtResponse`
//! verbatim — dissolution plan Step B
//! (`host/project/reports/mlopt_dissolution_plan.md`).
//!
//! Interpreter resolution order (inside `run_mlopt`):
//!   1. `MLOPT_PYTHON` env var (explicit override)
//!   2. `python3` on PATH (expects the mlopt project's venv activated,
//!      or `PYTHONPATH` pointing at an installed `mlopt`)
//!
//! In a checkout, run `uv sync` once, then either activate `.venv` or
//! set `MLOPT_PYTHON=$PWD/.venv/bin/python`.

use handl_ext::{ExtRequest, ExtResponse, ExtensionManifest};
use std::path::Path;

handl_ext::export!(
    ExtensionManifest::new("mlopt")
        .version(env!("CARGO_PKG_VERSION"))
        // Single namespaced command — hnl's command table is global, so
        // bare `doctor`/`export`/`models` would collide with other
        // extensions. `hnl mlopt optimize|export|doctor|models …`.
        .command(
            "mlopt",
            "mlopt <optimize|export|doctor|models> <args...> — e.g. `hnl mlopt optimize whisper:openai/whisper-turbo`",
        ),
    dispatch
);

/// ABI dispatch: `mlopt` forwards argv verbatim to `python -m mlopt`.
fn dispatch(cmd: &str, req: &ExtRequest) -> ExtResponse {
    match cmd {
        "mlopt" => {
            let r = handl_kit_mlopt::run_mlopt(&req.args, Path::new(&req.cwd), &req.env);
            ExtResponse {
                code: r.code,
                stdout: r.stdout,
                stderr: r.stderr,
            }
        }
        other => ExtResponse::error(2, format!("mlopt: unknown command `{other}`\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn req(args: &[&str]) -> ExtRequest {
        ExtRequest {
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: ".".into(),
            // /bin/echo stands in for the interpreter: it prints the
            // forwarded argv verbatim, so no Python is needed.
            env: BTreeMap::from([("MLOPT_PYTHON".to_string(), "/bin/echo".to_string())]),
        }
    }

    #[test]
    fn mlopt_forwards_argv_to_python_m_mlopt() {
        let r = dispatch("mlopt", &req(&["models", "--json"]));
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);
        // echo prints `-m mlopt models --json` — the verbatim forward.
        assert!(
            r.stdout.contains("-m mlopt models --json"),
            "stdout: {}",
            r.stdout
        );
    }

    #[test]
    fn mlopt_covers_the_full_cli_surface() {
        for sub in ["models", "optimize", "export", "doctor"] {
            let r = dispatch("mlopt", &req(&[sub]));
            assert_eq!(r.code, 0, "{sub}: stderr {}", r.stderr);
            assert!(
                r.stdout.contains(&format!("-m mlopt {sub}")),
                "{sub}: stdout {}",
                r.stdout
            );
        }
    }

    #[test]
    fn unknown_command_declines() {
        let r = dispatch("bogus", &req(&[]));
        assert_eq!(r.code, 2);
        assert!(r.stderr.contains("unknown command"));
    }

    #[test]
    fn missing_interpreter_is_loud_error() {
        let mut r = req(&["models"]);
        r.env.insert(
            "MLOPT_PYTHON".to_string(),
            "/nonexistent/python".to_string(),
        );
        let resp = dispatch("mlopt", &r);
        assert_eq!(resp.code, 2);
        assert!(resp.stderr.contains("failed to launch"));
    }
}
