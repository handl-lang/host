//! Tests for the arrow parser and loader (ADR-0069).
//!
//! These tests verify:
//! - Simple arrow-graph syntax parses correctly
//! - Rich `loop { ... } ( proc ... )` syntax parses correctly
//! - Actual `.hnl` stdlib files parse successfully
//! - Inference from parsed `.hnl` files matches hand-built Scheme results

use std::collections::HashMap;

use handl_language::Scheme;

use crate::arrow_loader::{infer_file, load_all_arrows, load_arrow_file, resolve_apps};
use crate::arrow_parser::{parse_arrow_expr, parse_arrow_file, ParsedFile};
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::Measure;

// Embed the actual .hnl stdlib files for testing.
const INSERT_HNL: &str =
    include_str!("../../../kit-stdlib/data/arrows/collection/insert.hnl");
const SORT_HNL: &str =
    include_str!("../../../kit-stdlib/data/arrows/collection/sort.hnl");
const MAP_HNL: &str = include_str!("../../../kit-stdlib/data/arrows/fn/map.hnl");
const FILTER_HNL: &str = include_str!("../../../kit-stdlib/data/arrows/fn/filter.hnl");
const FOLD_HNL: &str = include_str!("../../../kit-stdlib/data/arrows/fn/fold.hnl");

// ============================================================================
// Hand-built graphs (matching e2e_tests.rs) for comparison
// ============================================================================

fn insert_graph() -> Scheme {
    Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("prim.compare", vec![])),
            snd: Box::new(Scheme::Choice {
                l: Box::new(Scheme::arr_native("cons", vec![])),
                r: Box::new(Scheme::arr_native("cons", vec![])),
            }),
        }),
    }
}

fn sort_graph() -> Scheme {
    Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(insert_graph()),
            snd: Box::new(Scheme::First {
                inner: Box::new(Scheme::arr_native("id", vec![])),
            }),
        }),
    }
}

fn map_graph() -> Scheme {
    Scheme::Loop {
        body: Box::new(Scheme::First {
            inner: Box::new(Scheme::arr_native("fn.apply", vec![])),
        }),
    }
}

fn filter_graph() -> Scheme {
    Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("filter_step", vec![])),
            snd: Box::new(Scheme::arr_native("id", vec![])),
        }),
    }
}

fn fold_graph() -> Scheme {
    Scheme::Loop {
        body: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("fold_step", vec![])),
            snd: Box::new(Scheme::First {
                inner: Box::new(Scheme::arr_native("id", vec![])),
            }),
        }),
    }
}

// ============================================================================
// Simple syntax parsing tests
// ============================================================================

#[test]
fn test_parse_arr() {
    let scheme = parse_arrow_expr(r#"arr("sort")"#).expect("parse arr");
    match scheme {
        Scheme::Arr {
            op,
            source,
            args,
            lit,
        } => {
            assert_eq!(op, "sort");
            assert!(args.is_empty());
            assert!(lit.is_none());
            assert_eq!(
                source,
                handl_language::ArrSource::Native,
                "arr should be Native source"
            );
        }
        other => panic!("expected Arr, got {:?}", other),
    }
}

#[test]
fn test_parse_id() {
    let scheme = parse_arrow_expr("id").expect("parse id");
    match scheme {
        Scheme::Arr { op, .. } => {
            assert_eq!(op, "id");
        }
        other => panic!("expected Arr(id), got {:?}", other),
    }
}

#[test]
fn test_parse_comp() {
    let scheme =
        parse_arrow_expr(r#"comp(arr("a"), arr("b"))"#).expect("parse comp");
    match scheme {
        Scheme::Seq { fst, snd } => {
            assert!(matches!(*fst, Scheme::Arr { ref op, .. } if op == "a"));
            assert!(matches!(*snd, Scheme::Arr { ref op, .. } if op == "b"));
        }
        other => panic!("expected Seq, got {:?}", other),
    }
}

#[test]
fn test_parse_first() {
    let scheme = parse_arrow_expr("first(id)").expect("parse first");
    match scheme {
        Scheme::First { inner } => {
            assert!(matches!(*inner, Scheme::Arr { ref op, .. } if op == "id"));
        }
        other => panic!("expected First, got {:?}", other),
    }
}

#[test]
fn test_parse_choice() {
    let scheme =
        parse_arrow_expr(r#"choice(arr("a"), arr("b"))"#).expect("parse choice");
    match scheme {
        Scheme::Choice { l, r } => {
            assert!(matches!(*l, Scheme::Arr { ref op, .. } if op == "a"));
            assert!(matches!(*r, Scheme::Arr { ref op, .. } if op == "b"));
        }
        other => panic!("expected Choice, got {:?}", other),
    }
}

#[test]
fn test_parse_loop() {
    let scheme = parse_arrow_expr("loop(id)").expect("parse loop");
    match scheme {
        Scheme::Loop { body } => {
            assert!(matches!(*body, Scheme::Arr { ref op, .. } if op == "id"));
        }
        other => panic!("expected Loop, got {:?}", other),
    }
}

#[test]
fn test_parse_nested_loop_comp() {
    // loop(comp(insert, first(id)))
    let scheme =
        parse_arrow_expr("loop(comp(insert, first(id)))").expect("parse nested");
    match scheme {
        Scheme::Loop { body } => {
            match body.as_ref() {
                Scheme::Seq { fst, snd } => {
                    // insert is a bare identifier → App reference
                    assert!(
                        matches!(fst.as_ref(), Scheme::App { .. }),
                        "expected App for bare identifier 'insert', got {:?}",
                        fst
                    );
                    // first(id)
                    assert!(matches!(snd.as_ref(), Scheme::First { .. }));
                }
                other => panic!("expected Seq in loop body, got {:?}", other),
            }
        }
        other => panic!("expected Loop, got {:?}", other),
    }
}

#[test]
fn test_parse_app() {
    let scheme = parse_arrow_expr(r#"app("sort", arr("x"))"#).expect("parse app");
    match scheme {
        Scheme::App { callee, args } => {
            assert!(matches!(callee.as_ref(), Scheme::Arr { op, .. } if op == "sort"));
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected App, got {:?}", other),
    }
}

#[test]
fn test_parse_bare_identifier_is_app_reference() {
    // A bare identifier (not id, not a combinator) is a reference to a named arrow
    let scheme = parse_arrow_expr("insert").expect("parse bare ident");
    match scheme {
        Scheme::App { callee, args } => {
            assert!(matches!(callee.as_ref(), Scheme::Arr { op, .. } if op == "insert"));
            assert!(args.is_empty());
        }
        other => panic!("expected App for bare identifier, got {:?}", other),
    }
}

// ============================================================================
// Multi-name file parsing tests
// ============================================================================

#[test]
fn test_parse_multi_name_file() {
    let src = r#"
cons = arr("list.cons")
head = arr("list.head")
sort = loop(comp(insert, first(id)))
"#;
    let parsed = parse_arrow_file(src).expect("parse multi-name");
    match parsed {
        ParsedFile::Named(map) => {
            assert_eq!(map.len(), 3, "should have 3 named arrows");
            assert!(map.contains_key("cons"));
            assert!(map.contains_key("head"));
            assert!(map.contains_key("sort"));
            // Verify cons is an arr
            match map.get("cons").unwrap() {
                Scheme::Arr { op, .. } => assert_eq!(op, "list.cons"),
                other => panic!("expected Arr for cons, got {:?}", other),
            }
            // Verify sort is a Loop
            assert!(matches!(map.get("sort").unwrap(), Scheme::Loop { .. }));
        }
        other => panic!("expected Named, got {:?}", other),
    }
}

#[test]
fn test_parse_single_expr_file() {
    let src = "loop(comp(insert, first(id)))";
    let parsed = parse_arrow_file(src).expect("parse single expr");
    match parsed {
        ParsedFile::Single(scheme) => {
            assert!(matches!(scheme, Scheme::Loop { .. }));
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

// ============================================================================
// Rich syntax parsing tests (actual .hnl files)
// ============================================================================

#[test]
fn test_parse_insert_hnl() {
    let parsed = parse_arrow_file(INSERT_HNL).expect("parse insert.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            // Should be: loop(comp(prim.compare, choice(cons, cons)))
            assert!(matches!(scheme, Scheme::Loop { .. }));
            if let Scheme::Loop { body } = &scheme {
                assert!(matches!(body.as_ref(), Scheme::Seq { .. }));
                if let Scheme::Seq { fst, snd } = body.as_ref() {
                    assert!(
                        matches!(fst.as_ref(), Scheme::Arr { op, .. } if op == "prim.compare"),
                        "fst should be prim.compare"
                    );
                    assert!(
                        matches!(snd.as_ref(), Scheme::Choice { .. }),
                        "snd should be Choice"
                    );
                }
            }
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

#[test]
fn test_parse_sort_hnl() {
    let parsed = parse_arrow_file(SORT_HNL).expect("parse sort.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            // Should be: loop(comp(app(insert), first(id)))
            assert!(matches!(scheme, Scheme::Loop { .. }));
            if let Scheme::Loop { body } = &scheme {
                assert!(matches!(body.as_ref(), Scheme::Seq { .. }));
                if let Scheme::Seq { fst, snd } = body.as_ref() {
                    // fst should be App(insert) — a reference to the insert arrow
                    assert!(
                        matches!(fst.as_ref(), Scheme::App { .. }),
                        "fst should be App(insert)"
                    );
                    // snd should be first(id)
                    assert!(
                        matches!(snd.as_ref(), Scheme::First { .. }),
                        "snd should be First"
                    );
                }
            }
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

#[test]
fn test_parse_map_hnl() {
    let parsed = parse_arrow_file(MAP_HNL).expect("parse map.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            // Should be: loop(first(fn.apply))
            assert!(matches!(scheme, Scheme::Loop { .. }));
            if let Scheme::Loop { body } = &scheme {
                assert!(
                    matches!(body.as_ref(), Scheme::First { .. }),
                    "body should be First"
                );
                if let Scheme::First { inner } = body.as_ref() {
                    assert!(
                        matches!(inner.as_ref(), Scheme::Arr { op, .. } if op == "fn.apply"),
                        "inner should be fn.apply"
                    );
                }
            }
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

#[test]
fn test_parse_filter_hnl() {
    let parsed = parse_arrow_file(FILTER_HNL).expect("parse filter.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            // Should be: loop(comp(filter_step, id))
            assert!(matches!(scheme, Scheme::Loop { .. }));
            if let Scheme::Loop { body } = &scheme {
                assert!(matches!(body.as_ref(), Scheme::Seq { .. }));
                if let Scheme::Seq { fst, snd } = body.as_ref() {
                    assert!(
                        matches!(fst.as_ref(), Scheme::Arr { op, .. } if op == "filter_step"),
                        "fst should be filter_step"
                    );
                    assert!(
                        matches!(snd.as_ref(), Scheme::Arr { op, .. } if op == "id"),
                        "snd should be id"
                    );
                }
            }
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

#[test]
fn test_parse_fold_hnl() {
    let parsed = parse_arrow_file(FOLD_HNL).expect("parse fold.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            // Should be: loop(comp(fold_step, first(id)))
            assert!(matches!(scheme, Scheme::Loop { .. }));
            if let Scheme::Loop { body } = &scheme {
                assert!(matches!(body.as_ref(), Scheme::Seq { .. }));
                if let Scheme::Seq { fst, snd } = body.as_ref() {
                    assert!(
                        matches!(fst.as_ref(), Scheme::Arr { op, .. } if op == "fold_step"),
                        "fst should be fold_step"
                    );
                    assert!(
                        matches!(snd.as_ref(), Scheme::First { .. }),
                        "snd should be First"
                    );
                }
            }
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

// ============================================================================
// Parsed .hnl matches hand-built Scheme (after resolution)
// ============================================================================

#[test]
fn test_parsed_insert_matches_handbuilt() {
    let parsed = parse_arrow_file(INSERT_HNL).expect("parse insert");
    let handbuilt = insert_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed insert.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_parsed_map_matches_handbuilt() {
    let parsed = parse_arrow_file(MAP_HNL).expect("parse map");
    let handbuilt = map_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed map.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_parsed_filter_matches_handbuilt() {
    let parsed = parse_arrow_file(FILTER_HNL).expect("parse filter");
    let handbuilt = filter_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed filter.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_parsed_fold_matches_handbuilt() {
    let parsed = parse_arrow_file(FOLD_HNL).expect("parse fold");
    let handbuilt = fold_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed fold.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_parsed_sort_resolved_matches_handbuilt() {
    // sort.hnl references insert via App — after resolution, it should match
    let parsed = parse_arrow_file(SORT_HNL).expect("parse sort");
    let handbuilt = sort_graph();

    // Build a graph map with insert's parsed graph
    let insert_parsed = parse_arrow_file(INSERT_HNL).expect("parse insert");
    let mut graph_map = HashMap::new();
    if let ParsedFile::Single(insert_scheme) = insert_parsed {
        graph_map.insert("insert".to_string(), insert_scheme);
    }

    if let ParsedFile::Single(sort_scheme) = parsed {
        let resolved = resolve_apps(&sort_scheme, &graph_map);
        assert_eq!(
            resolved, handbuilt,
            "resolved sort.hnl should match hand-built graph (insert inlined)"
        );
    } else {
        panic!("expected Single");
    }
}

// ============================================================================
// End-to-end: infer from parsed .hnl → same results as hand-built Scheme
// ============================================================================

#[test]
fn test_infer_insert_from_hnl() {
    let registry = ContractRegistry::minimal();
    let parsed = parse_arrow_file(INSERT_HNL).expect("parse insert");
    let handbuilt = insert_graph();

    if let ParsedFile::Single(scheme) = parsed {
        let parsed_result = infer_refinement(&scheme, &registry).expect("infer parsed");
        let handbuilt_result =
            infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        // Same guarantee
        assert_eq!(
            parsed_result.guarantee, handbuilt_result.guarantee,
            "parsed and hand-built insert should have the same guarantee"
        );

        // Guarantee should contain Sorted
        let pretty = parsed_result.guarantee.pretty();
        assert!(
            pretty.contains("Sorted"),
            "insert guarantee should contain Sorted: {}",
            pretty
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_infer_sort_from_hnl() {
    let registry = ContractRegistry::minimal();

    // Build graph map for resolution
    let insert_parsed = parse_arrow_file(INSERT_HNL).expect("parse insert");
    let mut graph_map = HashMap::new();
    if let ParsedFile::Single(insert_scheme) = insert_parsed {
        graph_map.insert("insert".to_string(), insert_scheme);
    }

    let sort_parsed = parse_arrow_file(SORT_HNL).expect("parse sort");
    let handbuilt = sort_graph();

    if let ParsedFile::Single(sort_scheme) = sort_parsed {
        let resolved = resolve_apps(&sort_scheme, &graph_map);

        let parsed_result = infer_refinement(&resolved, &registry).expect("infer parsed");
        let handbuilt_result =
            infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        // Same guarantee
        assert_eq!(
            parsed_result.guarantee, handbuilt_result.guarantee,
            "parsed and hand-built sort should have the same guarantee"
        );

        // Guarantee should contain Sorted (propagated from insert)
        let pretty = parsed_result.guarantee.pretty();
        assert!(
            pretty.contains("Sorted"),
            "sort guarantee should contain Sorted: {}",
            pretty
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_infer_map_from_hnl() {
    let registry = ContractRegistry::minimal();
    let parsed = parse_arrow_file(MAP_HNL).expect("parse map");
    let handbuilt = map_graph();

    if let ParsedFile::Single(scheme) = parsed {
        let parsed_result = infer_refinement(&scheme, &registry).expect("infer parsed");
        let handbuilt_result =
            infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        assert_eq!(
            parsed_result.guarantee, handbuilt_result.guarantee,
            "parsed and hand-built map should have the same guarantee"
        );

        // map guarantee should contain LengthEq(input.1, output.1)
        let pretty = parsed_result.guarantee.pretty();
        assert!(
            pretty.contains("input.1") && pretty.contains("output.1"),
            "map guarantee should contain LengthEq(input.1, output.1): {}",
            pretty
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_infer_filter_from_hnl() {
    let registry = ContractRegistry::minimal();
    let parsed = parse_arrow_file(FILTER_HNL).expect("parse filter");
    let handbuilt = filter_graph();

    if let ParsedFile::Single(scheme) = parsed {
        let parsed_result = infer_refinement(&scheme, &registry).expect("infer parsed");
        let handbuilt_result =
            infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        assert_eq!(
            parsed_result.guarantee, handbuilt_result.guarantee,
            "parsed and hand-built filter should have the same guarantee"
        );

        // filter guarantee should contain AllSatisfy
        let pretty = parsed_result.guarantee.pretty();
        assert!(
            pretty.contains("AllSatisfy"),
            "filter guarantee should contain AllSatisfy: {}",
            pretty
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_infer_fold_from_hnl() {
    let registry = ContractRegistry::minimal();
    let parsed = parse_arrow_file(FOLD_HNL).expect("parse fold");
    let handbuilt = fold_graph();

    if let ParsedFile::Single(scheme) = parsed {
        let parsed_result = infer_refinement(&scheme, &registry).expect("infer parsed");
        let handbuilt_result =
            infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        // Same termination measure
        assert_eq!(
            parsed_result.termination, handbuilt_result.termination,
            "parsed and hand-built fold should have the same termination"
        );

        // fold termination should be ListLength, not Constant
        let term = parsed_result.termination.as_ref().expect("termination");
        assert!(
            matches!(term, Measure::ListLength(_)),
            "fold termination should be ListLength, got {:?}",
            term
        );
        assert_ne!(
            *term,
            Measure::Constant,
            "fold termination must NOT be Constant"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_infer_all_five_from_hnl() {
    let registry = ContractRegistry::minimal();

    // Build graph map for resolution (insert needed by sort)
    let insert_parsed = parse_arrow_file(INSERT_HNL).expect("parse insert");
    let mut graph_map = HashMap::new();
    if let ParsedFile::Single(insert_scheme) = insert_parsed {
        graph_map.insert("insert".to_string(), insert_scheme);
    }

    let files = [
        ("insert", INSERT_HNL, insert_graph()),
        ("sort", SORT_HNL, sort_graph()),
        ("map", MAP_HNL, map_graph()),
        ("filter", FILTER_HNL, filter_graph()),
        ("fold", FOLD_HNL, fold_graph()),
    ];

    for (name, hnl_src, handbuilt) in files {
        let parsed = parse_arrow_file(hnl_src).expect(&format!("parse {}", name));
        if let ParsedFile::Single(scheme) = parsed {
            let resolved = resolve_apps(&scheme, &graph_map);
            let parsed_result =
                infer_refinement(&resolved, &registry).expect(&format!("infer {}", name));
            let handbuilt_result =
                infer_refinement(&handbuilt, &registry).expect(&format!("infer handbuilt {}", name));

            assert_eq!(
                parsed_result.guarantee, handbuilt_result.guarantee,
                "{}: parsed and hand-built guarantees should match",
                name
            );

            println!(
                "{}: guarantee={}",
                name,
                parsed_result.guarantee.pretty()
            );
        } else {
            panic!("{}: expected Single", name);
        }
    }
}

// ============================================================================
// File loader tests (using actual file paths)
// ============================================================================

#[test]
fn test_load_arrow_file_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../kit-stdlib/data/arrows", manifest_dir);

    // Load insert.hnl from disk
    let insert_path = format!("{}/collection/insert.hnl", arrows_dir);
    let scheme = load_arrow_file(&insert_path).expect("load insert from disk");
    assert!(matches!(scheme, Scheme::Loop { .. }));

    // Load map.hnl from disk
    let map_path = format!("{}/fn/map.hnl", arrows_dir);
    let scheme = load_arrow_file(&map_path).expect("load map from disk");
    assert!(matches!(scheme, Scheme::Loop { .. }));
}

#[test]
fn test_load_all_arrows_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../kit-stdlib/data/arrows", manifest_dir);

    let map = load_all_arrows(&arrows_dir).expect("load all arrows");
    assert!(
        map.contains_key("insert"),
        "should have insert arrow: keys = {:?}",
        map.keys().collect::<Vec<_>>()
    );
    assert!(map.contains_key("sort"));
    assert!(map.contains_key("map"));
    assert!(map.contains_key("filter"));
    assert!(map.contains_key("fold"));
}

#[test]
fn test_infer_file_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../kit-stdlib/data/arrows", manifest_dir);
    let registry = ContractRegistry::minimal();

    // Infer insert from disk
    let insert_path = format!("{}/collection/insert.hnl", arrows_dir);
    let result = infer_file(&insert_path, &registry).expect("infer insert from disk");
    let pretty = result.guarantee.pretty();
    assert!(
        pretty.contains("Sorted"),
        "insert from disk should have Sorted: {}",
        pretty
    );

    // Infer sort from disk (should resolve insert reference)
    let sort_path = format!("{}/collection/sort.hnl", arrows_dir);
    let result = infer_file(&sort_path, &registry).expect("infer sort from disk");
    let pretty = result.guarantee.pretty();
    assert!(
        pretty.contains("Sorted"),
        "sort from disk should have Sorted: {}",
        pretty
    );
}
