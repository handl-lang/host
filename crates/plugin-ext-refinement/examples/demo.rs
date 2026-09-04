//! Demo: load actual `.hnl` arrow files and infer their contracts.
//!
//! This demonstrates ADR-0069: a user writes `.hnl` source files, and the
//! inference engine infers contracts from them — no hand-built `Scheme`
//! values, no user annotations. The path is:
//!
//!   `.hnl` file → `arrow_parser` → `Scheme` IR → `infer_refinement`

use handl_plugin_ext_refinement::{
    infer_file, load_all_arrows, load_arrow_file, resolve_apps, ContractRegistry,
    infer_refinement,
};

fn main() {
    let registry = ContractRegistry::from_axioms();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../handl-packages/kit-stdlib", manifest_dir);

    println!("=== ADR-0069: .hnl arrow files → Scheme → inferred contracts ===");
    println!("Atom contracts: {}", registry.len());
    println!("Arrows directory: {}", arrows_dir);
    println!();

    // Load all arrow files for cross-file reference resolution
    let graph_map = load_all_arrows(&arrows_dir).expect("failed to load arrow files");
    println!("Loaded {} arrow files from stdlib", graph_map.len());
    println!();

    let files = [
        ("insert", "collection/insert.hnl"),
        ("sort", "collection/sort.hnl"),
        ("map", "fn/map.hnl"),
        ("filter", "fn/filter.hnl"),
        ("fold", "fn/fold.hnl"),
        ("pratt", "parse-pratt/pratt.hnl"),
        ("counter", "state/counter.hnl"),
        ("buffer", "state/buffer.hnl"),
        ("protocol", "state/protocol.hnl"),
    ];

    for (name, rel_path) in files {
        let path = format!("{}/{}", arrows_dir, rel_path);

        // Load the .hnl file → Scheme IR
        let scheme = match load_arrow_file(&path) {
            Ok(s) => s,
            Err(e) => {
                println!("{}: FAILED to load: {}\n", name, e);
                continue;
            }
        };

        // Resolve cross-file references (e.g. sort → insert)
        let resolved = resolve_apps(&scheme, &graph_map);

        // Infer the contract
        match infer_refinement(&resolved, &registry) {
            Ok(r) => {
                println!("{} (from {}):", name, rel_path);
                println!("  guarantee:     {}", r.guarantee.pretty());
                println!(
                    "  invariants:    {} discovered",
                    r.loop_invariants.len()
                );
                for inv in &r.loop_invariants {
                    println!("    - {} (via {})", inv.invariant.pretty(), inv.how_discovered);
                }
                println!(
                    "  termination:   {}",
                    r.termination
                        .as_ref()
                        .map(|t| format!("{:?}", t))
                        .unwrap_or("unknown".to_string())
                );
                println!("  annotations:   0");
                println!();
            }
            Err(e) => println!("{}: ERROR: {}\n", name, e),
        }
    }

    // Also demonstrate the one-call API: infer_file does load + resolve + infer
    println!("--- One-call API: infer_file(path, registry) ---");
    let sort_path = format!("{}/collection/sort.hnl", arrows_dir);
    match infer_file(&sort_path, &registry) {
        Ok(r) => {
            println!("sort (via infer_file): guarantee = {}", r.guarantee.pretty());
        }
        Err(e) => println!("sort (via infer_file): ERROR: {}", e),
    }
    println!();

    println!("=== Loaded from actual .hnl files. Zero user annotations. ===");
    println!("=== Contracts only on Rust atoms. Inference is compositional. ===");
}
