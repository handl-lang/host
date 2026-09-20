//! Demo: load actual `.hnl` arrow files and infer their contracts.
//!
//! This demonstrates ADR-0069: a user writes `.hnl` source files, and the
//! inference engine infers contracts from them — no hand-built `Term`
//! values, no user annotations. The path is:
//!
//!   `.hnl` file → `arrow_parser` → `Term` IR → `infer_refinement`

use handl_plugin_ext_refinement::{
    ContractRegistry, infer_file, infer_refinement, load_all_arrows, load_arrow_file, resolve_apps,
};

fn main() {
    let registry = ContractRegistry::from_axioms();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // The former `handl-packages/{fn,collection,state,contracts,parse-pratt}/`
    // arrows — superseded as stdlib packages, kept only as
    // refinement-checker fixtures.
    let fixtures_dir = format!("{}/tests/fixtures", manifest_dir);

    println!("=== ADR-0069: .hnl arrow files → Term → inferred contracts ===");
    println!("Atom contracts: {}", registry.len());
    println!("Arrows directory: {}", fixtures_dir);
    println!();

    // Load all arrow files for cross-file reference resolution
    let graph_map = load_all_arrows(&fixtures_dir).expect("failed to load fixture arrows");
    println!("Loaded {} arrow files from stdlib", graph_map.len());
    println!();

    let files = [
        ("insert", &fixtures_dir, "collection/insert.hnl"),
        ("sort", &fixtures_dir, "collection/sort.hnl"),
        ("map", &fixtures_dir, "fn/map.hnl"),
        ("filter", &fixtures_dir, "fn/filter.hnl"),
        ("fold", &fixtures_dir, "fn/fold.hnl"),
        ("pratt", &fixtures_dir, "parse-pratt/pratt.hnl"),
        ("counter", &fixtures_dir, "state/counter.hnl"),
        ("buffer", &fixtures_dir, "state/buffer.hnl"),
        ("protocol", &fixtures_dir, "state/protocol.hnl"),
    ];

    for (name, base_dir, rel_path) in files {
        let path = format!("{}/{}", base_dir, rel_path);

        // Load the .hnl file → Term IR
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
                println!("  invariants:    {} discovered", r.loop_invariants.len());
                for inv in &r.loop_invariants {
                    println!(
                        "    - {} (via {})",
                        inv.invariant.pretty(),
                        inv.how_discovered
                    );
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
    let sort_path = format!("{}/collection/sort.hnl", fixtures_dir);
    match infer_file(&sort_path, &registry) {
        Ok(r) => {
            println!(
                "sort (via infer_file): guarantee = {}",
                r.guarantee.pretty()
            );
        }
        Err(e) => println!("sort (via infer_file): ERROR: {}", e),
    }
    println!();

    println!("=== Loaded from actual .hnl files. Zero user annotations. ===");
    println!("=== Contracts only on Rust atoms. Inference is compositional. ===");
}
