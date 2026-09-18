//! `hnlc` -- the handl arrow-graph compiler CLI.
//!
//! Loads a `.hnl` file (with imports), runs the full module pipeline
//! (parse_arrow_module -> ModuleLoader -> Strategy A or B -> inference),
//! and prints the result.
//!
//! # Usage
//!
//! ```text
//! hnlc <path-to-hnl-file> [--strategy <a|b>] [--root <dir>]
//!                        [--infer|--no-infer] [--print-scheme] [--verbose|-v]
//! ```
//!
//! The pipeline mirrors ADR-0069:
//! 1. `FileResolver { root }` resolves `<root>/<path>.hnl`.
//! 2. `ModuleLoader::new(resolver)` loads and caches modules.
//! 3. `normalize_compile_time` (Strategy A) or `flatten_program` (Strategy B)
//!    eliminates module refs, producing a self-contained `Term`.
//! 4. `infer_refinement(scheme, &ContractRegistry::standard())` infers
//!    contracts on the flattened graph.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use handl_language::Term;
use handl_plugin_ext_refinement::{
    ContractRegistry, FileResolver, ModuleLoader, RefinementResult, contains_module_ref,
    contains_project, flatten_program, has_unresolved_apps, infer_refinement,
    normalize_compile_time,
};

/// CLI configuration parsed from argv.
struct Config {
    file: PathBuf,
    strategy: Strategy,
    root: Option<PathBuf>,
    infer: bool,
    print_scheme: bool,
    verbose: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Strategy {
    A,
    B,
}

impl std::str::FromStr for Strategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "a" => Ok(Strategy::A),
            "b" => Ok(Strategy::B),
            other => Err(format!(
                "unknown strategy '{}' (expected 'a' or 'b')",
                other
            )),
        }
    }
}

/// Print the usage/help text to stdout.
fn print_usage() {
    println!("hnlc -- handl arrow-graph compiler");
    println!();
    println!("USAGE:");
    println!("    hnlc <path-to-hnl-file> [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --strategy <a|b>   Normalization strategy (default: b)");
    println!("                       a = normalize_compile_time (compile-time desugar)");
    println!("                       b = flatten_program (whole-program flattening)");
    println!("    --root <dir>       Resolver root directory (default: entry file's parent)");
    println!("    --infer            Run refinement inference (default)");
    println!("    --no-infer         Skip refinement inference");
    println!("    --print-scheme     Pretty-print the flattened Term graph");
    println!("    -v, --verbose      Show loading steps and cache hits");
    println!("    -h, --help         Print this help text");
}

/// Parse argv into a [`Config`], or print an error/usage and exit.
fn parse_args() -> Config {
    let mut args = std::env::args().skip(1);
    let mut file: Option<PathBuf> = None;
    let mut strategy = Strategy::B;
    let mut root: Option<PathBuf> = None;
    let mut infer = true;
    let mut print_scheme = false;
    let mut verbose = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--strategy" => {
                let val = args.next().unwrap_or_else(|| missing_value("--strategy"));
                strategy = val.parse().unwrap_or_else(|e| {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                });
            }
            "--root" => {
                let val = args.next().unwrap_or_else(|| missing_value("--root"));
                root = Some(PathBuf::from(val));
            }
            "--infer" => infer = true,
            "--no-infer" => infer = false,
            "--print-scheme" => print_scheme = true,
            "-v" | "--verbose" => verbose = true,
            other if other.starts_with("--") => {
                eprintln!("error: unknown option '{}'", other);
                eprintln!();
                print_usage();
                std::process::exit(1);
            }
            other => {
                if file.is_none() {
                    file = Some(PathBuf::from(other));
                } else {
                    eprintln!("error: unexpected extra argument '{}'", other);
                    std::process::exit(1);
                }
            }
        }
    }

    let file = match file {
        Some(f) => f,
        None => {
            eprintln!("error: missing file path argument");
            eprintln!();
            print_usage();
            std::process::exit(1);
        }
    };

    Config {
        file,
        strategy,
        root,
        infer,
        print_scheme,
        verbose,
    }
}

fn missing_value(flag: &str) -> ! {
    eprintln!("error: {} requires a value", flag);
    std::process::exit(1);
}

/// Compute the resolver root: explicit `--root`, else the entry file's parent.
fn resolver_root(cfg: &Config) -> PathBuf {
    cfg.root
        .clone()
        .unwrap_or_else(|| cfg.file.parent().map(Path::to_path_buf).unwrap_or_default())
}

/// Compute the module path (relative to the root, without `.hnl`).
fn module_path(root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let s = rel.to_string_lossy().to_string();
    s.strip_suffix(".hnl").unwrap_or(&s).to_string()
}

/// Pretty-print a `Term` graph with indentation for nested combinators.
fn pretty_scheme(scheme: &Term) -> String {
    pretty_scheme_inner(scheme, 0)
}

fn pretty_scheme_inner(scheme: &Term, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    match scheme {
        Term::Op {
            name: op,
            args,
            source,
            lit,
            ..
        } => {
            let lit_str = lit
                .as_ref()
                .map(|v| format!(" lit={}", v))
                .unwrap_or_default();
            if args.is_empty() {
                format!("{}arr({}{}){}", indent, op, lit_str, source_tag(source))
            } else {
                let mut out = format!(
                    "{}arr({}{}){} {{\n",
                    indent,
                    op,
                    lit_str,
                    source_tag(source)
                );
                for a in args {
                    out.push_str(&pretty_scheme_inner(a, depth + 1));
                    out.push('\n');
                }
                out.push_str(&indent);
                out.push('}');
                out
            }
        }
        Term::Seq { axis, fst, snd } => {
            // `>>>` is `>>>[0]`; show `seq[k]` for non-default axes.
            let head = match axis.nat_literal() {
                Some(0) => "seq".to_string(),
                Some(k) => format!("seq[{k}]"),
                None => "seq[?]".to_string(),
            };
            let mut out = format!("{}{} {{\n", indent, head);
            if !axis.is_default_axis() {
                out.push_str(&pretty_scheme_inner(axis, depth + 1));
                out.push('\n');
            }
            out.push_str(&pretty_scheme_inner(fst, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(snd, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        Term::First { inner } => {
            let mut out = format!("{}first {{\n", indent);
            out.push_str(&pretty_scheme_inner(inner, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        Term::Choice { l, r } => {
            let mut out = format!("{}choice {{\n", indent);
            out.push_str(&pretty_scheme_inner(l, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(r, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        Term::Loop { body } => {
            let mut out = format!("{}loop {{\n", indent);
            out.push_str(&pretty_scheme_inner(body, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        Term::App { callee, args } => {
            let mut out = format!("{}app {{\n", indent);
            out.push_str(&pretty_scheme_inner(callee, depth + 1));
            out.push('\n');
            for a in args {
                out.push_str(&pretty_scheme_inner(a, depth + 1));
                out.push('\n');
            }
            out.push_str(&indent);
            out.push('}');
            out
        }
        Term::Handle {
            body,
            target,
            impl_,
        } => {
            let mut out = format!("{}handle({}) {{\n", indent, target);
            out.push_str(&pretty_scheme_inner(body, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(impl_, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }

        Term::Arr { value, .. } => format!("{}const({})", indent, value),
        // `Const` literals appear in the `Seq` axis slot (`>>>[k]`,
        // ADR-0120 Phase 2) — print them like `Arr` constants.
        Term::Const { value } => format!("{}const({})", indent, value),
        Term::Child { role } => format!("{}child({})", indent, role),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => {
            let mut out = format!("{}iter {{\n", indent);
            out.push_str(&pretty_scheme_inner(fuel, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(predicate, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(step, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(initial, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        Term::Rewrite { source, target, .. } => {
            let mut out = format!("{}rewrite {{\n", indent);
            out.push_str(&pretty_scheme_inner(source, depth + 1));
            out.push('\n');
            out.push_str(&pretty_scheme_inner(target, depth + 1));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        _ => format!("{}<unsupported>", indent),
    }
}

fn source_tag(source: &handl_language::ArrSource) -> String {
    match source {
        handl_language::ArrSource::Native => String::new(),
        handl_language::ArrSource::Foreign(name) => format!(" <{}>", name),
        handl_language::ArrSource::Judgment => " <judgment>".to_string(),
        handl_language::ArrSource::DynamicForeign => " <dyn-foreign>".to_string(),
    }
}

/// Print a refinement result in a readable contract format.
fn print_refinement_result(name: &str, result: &RefinementResult) {
    println!("arrow \"{}\":", name);
    println!("  input:       {}", result.input.pretty());
    println!("  output:      {}", result.output.pretty());
    println!("  guarantee:   {}", result.guarantee.pretty());
    if let Some(term) = &result.termination {
        println!("  termination: {}", term.pretty());
    }
    if !result.loop_invariants.is_empty() {
        println!("  loop invariants:");
        for inv in &result.loop_invariants {
            println!(
                "    {} ({}; smt-verified: {})",
                inv.invariant.pretty(),
                inv.how_discovered,
                inv.verified_by_smt
            );
        }
    }
    if !result.law_annotations.is_empty() {
        println!("  laws:");
        for law in &result.law_annotations {
            println!("    {}: {}", law.law, law.strengthening);
        }
    }
}

/// Run the CLI. Returns the process exit code.
fn run() -> Result<(), String> {
    let cfg = parse_args();
    let root = resolver_root(&cfg);
    let entry = module_path(&root, &cfg.file);

    if cfg.verbose {
        println!("entry file : {}", cfg.file.display());
        println!("resolver   : <root = {}>", root.display());
        println!("entry path : {}", entry);
        println!("strategy   : {}", strategy_name(cfg.strategy));
        println!();
    }

    // 1. Build the resolver + loader.
    let resolver = FileResolver::new(root.clone());
    let loader = ModuleLoader::new(Box::new(resolver));

    // 2. Run the chosen strategy.
    match cfg.strategy {
        Strategy::A => {
            let arrows = normalize_compile_time(&entry, &loader)
                .map_err(|e| format!("Strategy A failed: {}", e))?;
            print_loading_summary(&loader, &entry, cfg.verbose);

            let names: Vec<String> = arrows.keys().cloned().collect();
            if cfg.print_scheme {
                for name in &names {
                    let scheme = &arrows[name];
                    println!("\n--- scheme: {} ---", name);
                    println!("{}", pretty_scheme(scheme));
                }
            }

            if cfg.infer {
                let registry = ContractRegistry::standard();
                for name in &names {
                    let scheme = &arrows[name];
                    match infer_refinement(scheme, &registry) {
                        Ok(result) => {
                            println!();
                            print_refinement_result(name, &result);
                        }
                        Err(e) => {
                            eprintln!("warning: inference failed for '{}': {}", name, e);
                        }
                    }
                }
            }
        }
        Strategy::B => {
            let scheme = flatten_program(&entry, &loader)
                .map_err(|e| format!("Strategy B failed: {}", e))?;
            print_loading_summary(&loader, &entry, cfg.verbose);

            // Report any residual unresolved nodes.
            let known: HashSet<String> = HashSet::new();
            let has_modref = contains_module_ref(&scheme);
            let has_proj = contains_project(&scheme);
            let has_apps = has_unresolved_apps(&scheme, &known);
            if has_modref || has_proj || has_apps {
                println!();
                if has_modref {
                    println!("note: residual module_ref nodes remain in the flattened graph");
                }
                if has_proj {
                    println!("note: residual project nodes remain in the flattened graph");
                }
                if has_apps {
                    println!("note: residual unresolved App nodes remain in the flattened graph");
                }
            }

            if cfg.print_scheme {
                println!("\n--- flattened scheme ---");
                println!("{}", pretty_scheme(&scheme));
            }

            if cfg.infer {
                let registry = ContractRegistry::standard();
                match infer_refinement(&scheme, &registry) {
                    Ok(result) => {
                        println!();
                        print_refinement_result("main", &result);
                    }
                    Err(e) => {
                        eprintln!("warning: inference failed: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

fn strategy_name(s: Strategy) -> &'static str {
    match s {
        Strategy::A => "A (normalize_compile_time)",
        Strategy::B => "B (flatten_program)",
    }
}

/// Print a summary of what the loader cached.
fn print_loading_summary(loader: &ModuleLoader, entry: &str, verbose: bool) {
    let graph_cache = loader.graph_cache.borrow();
    let contract_cache = loader.contract_cache.borrow();
    let loaded = graph_cache.len();
    let with_contracts = contract_cache.len();
    println!();
    println!("modules loaded : {}", loaded);
    println!("entry module   : {}", entry);
    if verbose {
        let mut paths: Vec<&String> = graph_cache.keys().collect();
        paths.sort();
        for p in &paths {
            let arrows = graph_cache.get(*p);
            let n = arrows.map(|m| m.len()).unwrap_or(0);
            println!("  - {} ({} arrow{})", p, n, if n == 1 { "" } else { "s" });
        }
    }
    let _ = with_contracts;
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("hnlc: {}", e);
            ExitCode::FAILURE
        }
    }
}
