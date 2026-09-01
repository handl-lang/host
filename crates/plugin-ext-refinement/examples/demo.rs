use handl_language::Scheme;
use handl_plugin_ext_refinement::{ContractRegistry, infer_refinement};

fn main() {
    let registry = ContractRegistry::minimal();
    println!("=== Minimal Proof: 3 Rust atoms → handl arrows → inferred contracts ===");
    println!("Atom contracts: {}", registry.len());
    println!();

    let insert = Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("prim.compare", vec![])),
            snd: Box::new(Scheme::Choice {
                l: Box::new(Scheme::arr_native("cons", vec![])),
                r: Box::new(Scheme::arr_native("cons", vec![])),
            }),
        }),
    };

    let sort = Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(insert.clone()),
            snd: Box::new(Scheme::First {
                inner: Box::new(Scheme::arr_native("id", vec![])),
            }),
        }),
    };

    let map = Scheme::Loop {
        body: Box::new(Scheme::First {
            inner: Box::new(Scheme::arr_native("fn.apply", vec![])),
        }),
    };

    let filter = Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("filter_step", vec![])),
            snd: Box::new(Scheme::arr_native("id", vec![])),
        }),
    };

    let fold = Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("fold_step", vec![])),
            snd: Box::new(Scheme::First {
                inner: Box::new(Scheme::arr_native("id", vec![])),
            }),
        }),
    };

    for (name, graph) in [
        ("insert", &insert),
        ("sort", &sort),
        ("map", &map),
        ("filter", &filter),
        ("fold", &fold),
    ] {
        match infer_refinement(graph, &registry) {
            Ok(r) => {
                println!("{}:", name);
                println!("  guarantee:     {}", r.guarantee.pretty());
                println!("  invariants:    {} discovered", r.loop_invariants.len());
                for inv in &r.loop_invariants {
                    println!("    - {} (via {})", inv.invariant.pretty(), inv.how_discovered);
                }
                println!("  termination:   {:?}", r.termination.map(|t| format!("{:?}", t)).unwrap_or("unknown".to_string()));
                println!("  annotations:   0");
                println!();
            }
            Err(e) => println!("{}: ERROR: {}\n", name, e),
        }
    }

    println!("=== Zero user annotations. Contracts only on Rust atoms. ===");
}
